use std::convert::TryFrom;

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
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
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
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
}
