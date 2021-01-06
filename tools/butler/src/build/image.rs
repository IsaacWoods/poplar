use super::BuildStep;
use eyre::{eyre, Result, WrapErr};
use std::{collections::BTreeMap, fs::File, path::PathBuf, process::Command};

pub struct MakeGptImage {
    pub image_path: PathBuf,
    /// Size of the image to make, in bytes. Must be a multiple of the LBA size (512 currently).
    pub image_size: u64,
    /// Size of the FAT partition for EFI to make, in bytes.
    pub efi_partition_size: u64,
}

impl BuildStep for MakeGptImage {
    fn build(self) -> Result<()> {
        use gpt::{disk::LogicalBlockSize, mbr::ProtectiveMBR, GptConfig, GptDisk};
        use std::convert::TryFrom;

        // TODO: Blocks of 512 bytes are hardcoded in a few places for now. We probably want to allow both LBA
        // sizes in the future.
        const LBA_SIZE: LogicalBlockSize = LogicalBlockSize::Lb512;

        Command::new("dd")
            .arg("if=/dev/zero")
            .arg(format!("of={}", self.image_path.to_str().unwrap()))
            .arg("bs=512")
            .arg(format!("count={}", self.image_size / (LBA_SIZE.into(): u64)))
            .status()
            .wrap_err("Failed to invoke dd")?
            .success()
            .then_some(())
            .ok_or(eyre!("Failed to make zeroed file for GPT image"))?;

        let mut image = Box::new(
            std::fs::OpenOptions::new()
                .write(true)
                .read(true)
                .open(self.image_path)
                .wrap_err("Failed to open zeroed image")?,
        );

        /*
         * Create a protective MBR in LBA 0.
         */
        let mbr = ProtectiveMBR::with_lb_size(u32::try_from((self.image_size / 512) - 1).unwrap_or(0xffffffff));
        mbr.overwrite_lba0(&mut image).wrap_err("Failed to write protective MBR to GPT image")?;

        let mut disk = GptConfig::default()
            .initialized(false)
            .writable(true)
            .logical_block_size(LogicalBlockSize::Lb512)
            .create_from_device(Box::new(image), None)
            .wrap_err("Failed to create GPT disk from zeroed image")?;

        /*
         * Update the partition table with an empty set of partitions to initialize the headers, and then add an
         * EFI System Partition.
         */
        disk.update_partitions(BTreeMap::new())?;
        let efi_partition_id = disk.add_partition("EFI", self.efi_partition_size, gpt::partition_types::EFI, 0)?;

        /*
         * Next, populate the blocks of that partition with a FAT32 filesystem.
         */
        let (efi_part_start, efi_part_end) = {
            let partition = disk.partitions().get(&efi_partition_id).unwrap();
            (
                partition.bytes_start(LBA_SIZE).unwrap(),
                partition.bytes_start(LBA_SIZE).unwrap() + partition.bytes_len(LBA_SIZE).unwrap(),
            )
        };
        let disk_file = disk.write().wrap_err("Failed to write GPT image to file/disk")?;
        let mut fat_partition = fscommon::StreamSlice::new(disk_file, efi_part_start, efi_part_end)
            .wrap_err("Failed to construct StreamSlice of FAT partition")?;
        fatfs::format_volume(
            &mut fat_partition,
            fatfs::FormatVolumeOptions::new().fat_type(fatfs::FatType::Fat32),
        )
        .wrap_err("Failed to format FAT partition with a FAT32 filesystem")?;
        let fat = fatfs::FileSystem::new(fat_partition, fatfs::FsOptions::new())
            .wrap_err("Failed to construct FAT filesystem from formatted partition")?;

        {
            let root_dir = fat.root_dir();
            root_dir.create_dir("efi").unwrap();
            root_dir.create_dir("efi/boot").unwrap();
            let mut efi_loader = root_dir.create_file("efi/boot/boot_x64.efi").unwrap();
            use std::io::Write;
            write!(efi_loader, "Test").unwrap();
        }

        println!("FAT statistics: {:#?}", fat.stats().wrap_err("Failed to get stats from FAT")?);
        fat.unmount().wrap_err("Failed to unmount FAT filesystem")?;
        Ok(())
    }
}
