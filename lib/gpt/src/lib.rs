#![feature(array_value_iter)]

pub mod header;
pub mod mbr;

use header::{GptHeader, GptPartitionEntry};
use mbr::MasterBootRecord;
use std::io::{Read, Result, Seek, SeekFrom, Write};

// We currently only support blocks of size 512.
const LBA_SIZE: usize = 512;

pub struct GptDisk<T: Read + Write + Seek> {
    image: T,
    mbr: MasterBootRecord,
    partitions: Vec<GptPartitionEntry>,
}

impl<T> GptDisk<T>
where
    T: Read + Write + Seek,
{
    /// Creates a new `GptDisk`. If you want to interact with an existing GPT image, use [`GptDisk::from_existing`]
    /// instead.
    pub fn new(image: T) -> Result<GptDisk<T>> {
        Ok(GptDisk { image, mbr: MasterBootRecord::protective(), partitions: Vec::new() })
    }

    pub fn from_existing(image: T) -> Result<GptDisk<T>> {
        todo!()
    }

    pub fn add_partition(&mut self, partition: GptPartitionEntry) -> Result<()> {
        // TODO: make sure the new partition is disjoint from the existing ones
        // TODO: make sure its past first_usable_lba and ends before last_usable_lba
        // TODO: make sure we're not adding more partitions than can fit in our 32-block array
        self.partitions.push(partition);
        Ok(())
    }

    pub fn write(mut self) -> Result<T> {
        /*
         * Find the length of the image, and make sure we're at the beginning, with only two seeks.
         */
        let image_length = self.image.seek(SeekFrom::End(0))?;
        assert!(image_length % LBA_SIZE == 0);
        let last_lba = (image_length / LBA_SIZE) - 1;
        self.image.seek(SeekFrom::Start(0))?;

        /*
         * Write the Master Boot Record.
         */
        self.mbr.write(&mut self.image)?;

        /*
         * Write the primary partition array. We reserve the minimum amount of space, 16384 bytes (32 blocks with a
         * size of 512).
         */
        self.image.seek(SeekFrom::Start(2 * LBA_SIZE))?;
        let mut entries_written = 0;
        for partition in self.partitions {
            partition.write(&mut self.image)?;
            entries_written += 1;
        }
        for i in entries_written..128 {
            self.image.write_all(&[0; 128])?;
        }

        // TODO: calculate the CRC32 of the partition array and put it in both headers

        /*
         * Write the primary header.
         * TODO: calculate the CRC32 of the header and put it in
         */
        self.image.seek(SeekFrom::Start(1 * LBA_SIZE))?;
        GptHeader::new_primary(last_lba).write(&mut self.image)?;

        /*
         * Write the backup partition array.
         */
        self.image.seek(SeekFrom::End(-(33 * LBA_SIZE)))?;
        let mut entries_written = 0;
        for partition in self.partitions {
            partition.write(&mut self.image)?;
            entries_written += 1;
        }
        for i in entries_written..128 {
            self.image.write_all(&[0; 128])?;
        }

        /*
         * Write the backup header.
         * TODO: calculate the CRC32 of the header and put it in
         */
        self.image.seek(SeekFrom::End(-(1 * LBA_SIZE)))?;
        GptHeader::new_backup(last_lba).write(&mut self.image)?;

        Ok(self.image)
    }
}
