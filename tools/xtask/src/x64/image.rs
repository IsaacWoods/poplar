use eyre::{eyre, Result, WrapErr};
use std::{collections::BTreeMap, fs::File, path::PathBuf, process::Command};

pub struct MakeGptImage {
    pub image_path: PathBuf,
    /// Size of the image to make, in bytes. Must be a multiple of the LBA size (512 currently).
    pub image_size: u64,
    /// Size of the FAT partition for EFI to make, in bytes.
    pub efi_partition_size: u64,
    /// A list of files to create on the EFI system partition. The first element is the path on the FAT to put it
    /// at, and the second is the file to read out of on the host filesystem.
    pub efi_part_files: Vec<(String, PathBuf)>,
}

impl MakeGptImage {
    pub fn new(path: PathBuf, size: u64, efi_partition_size: u64) -> MakeGptImage {
        MakeGptImage { image_path: path, image_size: size, efi_partition_size, efi_part_files: vec![] }
    }

    pub fn add_efi_file<S: Into<String>>(mut self, efi_path: S, host_path: PathBuf) -> MakeGptImage {
        self.efi_part_files.push((efi_path.into(), host_path));
        self
    }

    pub fn build(self) -> Result<()> {
        use gpt::{disk::LogicalBlockSize, mbr::ProtectiveMBR, GptConfig};
        use std::convert::TryFrom;

        // TODO: Blocks of 512 bytes are hardcoded in a few places for now. We probably want to allow both LBA
        // sizes in the future.
        const LBA_SIZE: LogicalBlockSize = LogicalBlockSize::Lb512;

        Command::new("dd")
            .arg("if=/dev/zero")
            .arg(format!("of={}", self.image_path.to_str().unwrap()))
            .arg("bs=512")
            .arg(format!("count={}", self.image_size / u64::from(LBA_SIZE)))
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
        let efi_partition_id =
            disk.add_partition("EFI", self.efi_partition_size, gpt::partition_types::EFI, 0, None)?;

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

        /*
         * Put the requested files into the EFI system partition.
         */
        {
            let root_dir = fat.root_dir();
            root_dir.create_dir("efi").unwrap();
            root_dir.create_dir("efi/boot").unwrap();

            for (fat_path, host_path) in self.efi_part_files {
                let mut host_file = File::open(host_path.clone()).wrap_err_with(|| {
                    format!("Failed to open host file to put on EFI system partition: {:?}", host_path)
                })?;
                let mut fat_file = root_dir
                    .create_file(&fat_path)
                    .wrap_err_with(|| format!("Failed to create file on EFI system partition at: {}", fat_path))?;
                std::io::copy(&mut host_file, &mut fat_file).wrap_err_with(|| {
                    format!("Failed to copy host file onto FAT partition: {:?} -> {}", host_path, fat_path)
                })?;
            }
        }

        println!("FAT statistics: {:#?}", fat.stats().wrap_err("Failed to get stats from FAT")?);
        fat.unmount().wrap_err("Failed to unmount FAT filesystem")?;
        Ok(())
    }
}
