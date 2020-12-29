use std::{
    convert::TryFrom,
    io::{Result, Write},
};

#[derive(Clone, Debug)]
pub struct MasterBootRecord {
    boot_code: [u8; 440],
    unique_disk_signature: u32,
    unknown: u16,
    partition_record: [PartitionRecord; 4],
    signature: u16,
}

impl MasterBootRecord {
    /// Constructs a protective MBR. Takes the size of the disk in logical blocks.
    pub fn protective(disk_size: u64) -> MasterBootRecord {
        MasterBootRecord {
            boot_code: [0; 440],
            unique_disk_signature: 0,
            unknown: 0,
            partition_record: [
                PartitionRecord::protective(disk_size),
                PartitionRecord::zero(),
                PartitionRecord::zero(),
                PartitionRecord::zero(),
            ],
            signature: 0xaa55,
        }
    }

    pub fn write<W: Write>(self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.boot_code)?;
        writer.write_all(&self.unique_disk_signature.to_le_bytes())?;
        writer.write_all(&self.unknown.to_le_bytes())?;
        for partition in std::array::IntoIter::new(self.partition_record) {
            partition.write(writer)?;
        }
        writer.write_all(&self.signature.to_le_bytes())?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct PartitionRecord {
    boot_indicator: u8,
    starting_chs: [u8; 3],
    os_type: u8,
    ending_chs: [u8; 3],
    starting_lba: u32,
    ending_lba: u32,
}

impl PartitionRecord {
    pub fn protective(size_in_logical_blocks: u64) -> PartitionRecord {
        PartitionRecord {
            boot_indicator: 0x00,
            starting_chs: [0x00, 0x02, 0x00],
            os_type: 0xee,
            // TODO: we should technically translate the size into CHS if it'll fit
            ending_chs: [0xff, 0xff, 0xff],
            starting_lba: 1,
            ending_lba: u32::try_from(size_in_logical_blocks - 1).unwrap_or(0xffffffff),
        }
    }

    pub fn zero() -> PartitionRecord {
        PartitionRecord {
            boot_indicator: 0x00,
            starting_chs: [0; 3],
            os_type: 0x00,
            ending_chs: [0; 3],
            starting_lba: 0,
            ending_lba: 0,
        }
    }

    pub fn write<W: Write>(self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.boot_indicator.to_le_bytes())?;
        writer.write_all(&self.starting_chs)?;
        writer.write_all(&self.os_type.to_le_bytes())?;
        writer.write_all(&self.ending_chs)?;
        writer.write_all(&self.starting_lba.to_le_bytes())?;
        writer.write_all(&self.ending_lba.to_le_bytes())?;
        Ok(())
    }
}
