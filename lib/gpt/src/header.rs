use std::io::{Result, Write};
use uuid::Uuid;

pub struct GptHeader {
    signature: [u8; 8],
    revision: u32,
    header_size: u32,
    header_crc32: u32,
    reserved: u32,
    my_lba: u64,
    alternate_lba: u64,
    first_usable_lba: u64,
    last_usable_lba: u64,
    disk_guid: Uuid,
    partition_entry_lba: u64,
    /// The number of partition entries in the array of this header. Note that this is the number of **entries**,
    /// not the number of **partitions**, and must have a minimum value of `128`.
    number_of_partition_entries: u32,
    size_of_partition_entry: u32,
    partition_entry_array_crc32: u32,
}

impl GptHeader {
    pub fn new_primary(last_lba: u64) -> GptHeader {
        GptHeader {
            signature: *b"EFI PART",
            revision: 0x0001_0000,
            header_size: 92,
            header_crc32: 0,
            reserved: 0,
            my_lba: 1,
            alternate_lba: last_lba,
            // This is the first possible LBA we can use for a block size of 512 (1 block for the MBR, 1 for the
            // primary header, and then 32 for the partition entry array).
            first_usable_lba: 34,
            // The last block that can be used for data. This takes the last LBA (which contains the secondary
            // header), and reserves 32 blocks for the secondary partition entry array, and then takes the next one
            // (the first free one).
            last_usable_lba: last_lba - 32 - 1,
            // TODO: we should probably be using v1, but it's harder to call so we're not for now.
            disk_guid: Uuid::new_v4(),
            partition_entry_lba: 2,
            number_of_partition_entries: 128,
            size_of_partition_entry: 128,
            partition_entry_array_crc32: 0,
        }
    }

    pub fn new_backup(last_lba: u64) -> GptHeader {
        GptHeader {
            signature: *b"EFI PART",
            revision: 0x0001_0000,
            header_size: 92,
            header_crc32: 0,
            reserved: 0,
            my_lba: last_lba,
            alternate_lba: 1,
            // This is the first possible LBA we can use for a block size of 512 (1 block for the MBR, 1 for the
            // primary header, and then 32 for the partition entry array).
            first_usable_lba: 34,
            // The last block that can be used for data. This takes the last LBA (which contains the secondary
            // header), and reserves 32 blocks for the secondary partition entry array, and then takes the next one
            // (the first free one).
            last_usable_lba: last_lba - 32 - 1,
            // TODO: we should probably be using v1, but it's harder to call so we're not for now.
            disk_guid: Uuid::new_v4(),
            partition_entry_lba: last_lba - 32,
            number_of_partition_entries: 128,
            size_of_partition_entry: 128,
            partition_entry_array_crc32: 0,
        }
    }

    pub fn write<W: Write>(self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.signature)?;
        writer.write_all(&self.revision.to_le_bytes())?;
        writer.write_all(&self.header_size.to_le_bytes())?;
        writer.write_all(&self.header_crc32.to_le_bytes())?;
        writer.write_all(&self.reserved.to_le_bytes())?;
        writer.write_all(&self.my_lba.to_le_bytes())?;
        writer.write_all(&self.alternate_lba.to_le_bytes())?;
        writer.write_all(&self.first_usable_lba.to_le_bytes())?;
        writer.write_all(&self.last_usable_lba.to_le_bytes())?;
        writer.write_all(self.disk_guid.as_bytes())?;
        writer.write_all(&self.partition_entry_lba.to_le_bytes())?;
        writer.write_all(&self.number_of_partition_entries.to_le_bytes())?;
        writer.write_all(&self.size_of_partition_entry.to_le_bytes())?;
        writer.write_all(&self.partition_entry_array_crc32.to_le_bytes())?;
        Ok(())
    }
}

pub struct GptPartitionEntry {
    type_guid: Uuid,
    unique_partition_guid: Uuid,
    starting_lba: u64,
    ending_lba: u64,
    attributes: u64,
    partition_name: [u8; 72],
}

impl GptPartitionEntry {
    pub fn new(
        type_guid: Uuid,
        starting_lba: u64,
        ending_lba: u64,
        // TODO: use a wrapper type here to make this nicer (bitflags?)
        attributes: u64,
        name: &str,
    ) -> GptPartitionEntry {
        // XXX: name must be null terminated, so can't actually be the full 72 long
        assert!(name.len() < 72);

        GptPartitionEntry {
            type_guid,
            unique_partition_guid: Uuid::new_v4(),
            starting_lba,
            ending_lba,
            attributes,
            partition_name: {
                let mut bytes = [0; 72];
                (bytes[0..name.len()]).copy_from_slice(name.as_bytes());
                bytes
            },
        }
    }

    pub fn write<W: Write>(self, writer: &mut W) -> Result<()> {
        writer.write_all(self.type_guid.as_bytes())?;
        writer.write_all(self.unique_partition_guid.as_bytes())?;
        writer.write_all(&self.starting_lba.to_le_bytes())?;
        writer.write_all(&self.ending_lba.to_le_bytes())?;
        writer.write_all(&self.attributes.to_le_bytes())?;
        writer.write_all(&self.partition_name)?;
        Ok(())
    }
}
