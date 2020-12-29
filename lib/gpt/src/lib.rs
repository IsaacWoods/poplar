#![feature(array_value_iter)]

pub mod mbr;

use std::io::{Read, Result, Seek, Write};

pub struct GptDisk<T: Read + Write + Seek> {
    inner: T,
}

impl<T> GptDisk<T>
where
    T: Read + Write + Seek,
{
    /// Creates a new `GptDisk`. If you want to interact with an existing GPT image, use [`GptDisk::from_existing`]
    /// instead.
    pub fn new(image: T) -> Result<GptDisk<T>> {
        todo!()
    }

    pub fn from_existing(image: T) -> Result<GptDisk<T>> {
        todo!()
    }
}
