/*
 * Copyright (C) 2018, Pebble Developers.
 * See LICENCE.md
 */

pub mod ramdisk;

use core::any::Any;
use core::str::Split;
use alloc::{String,Vec,boxed::Box,rc::Rc,BTreeMap};
use arch::MemoryAddress;
use libpebble::fs::FileHandle;

#[derive(Debug)]
pub enum FileError
{
    DoesNotExist,
    IsReadOnly,
    MalformedPath,
}

pub struct File
{
    pub name        : String,
    pub filesystem  : Rc<Filesystem>,
    pub data        : Box<Any>,
}

impl File
{
    pub fn read(&self) -> Result<Vec<u8>, FileError>
    {
        self.filesystem.read(self)
    }

    pub fn write(&mut self, stuff : &[u8]) -> Result<(), FileError>
    {
        self.filesystem.write(self, stuff)
    }

    pub fn close(self)
    {
        self.filesystem.close(&self);
    }
}

/// A filesystem is something that contains files (or can be treated as if it abstractly contained
/// files). It provides the implementations for managing the file on it.
/// These do not take the correct types, because if they did we couldn't borrow the filesystem as
/// well. These therefore should not be called manually, only internally. Use the methods on
/// `File` instead.
pub trait Filesystem
{
    fn open(&self, filesystem : Rc<Filesystem>, path : &str) -> Result<File, FileError>;
    fn close(&self, file : &File);
    fn read(&self, file : &File) -> Result<Vec<u8>, FileError>;
    fn write(&self, file : &File, stuff : &[u8]) -> Result<(), FileError>;
    fn get_physical_mapping(&self, file : &File) -> Option<(MemoryAddress, MemoryAddress)>;
}

fn parse_path<'a>(path : &'a str) -> Result<Split<'a, char>, FileError>
{
    // TODO: canonicalise the path
    //  * If not absolute, prepend with current working directory
    //  * Expand '.' and '..'
    //  * Remove assert below
    
    assert!(path.starts_with('/'), "Path isn't absolute");
    Ok(path[1..].split('/'))
}

/// This manages a set of filesystems and presents them as one virtual filesystem.
pub struct FileManager
{
    opened_files    : BTreeMap<FileHandle, File>,
    filesystems     : BTreeMap<String, Rc<Filesystem>>,
}

impl FileManager
{
    pub fn new() -> FileManager
    {
        FileManager
        {
            opened_files    : BTreeMap::new(),
            filesystems     : BTreeMap::new(),
        }
    }

    /// Mount a filesystem at the specified path
    pub fn mount(&mut self, mount_point : &str, filesystem : Rc<Filesystem>)
    {
        assert!(mount_point.starts_with('/'), "Filesystem mount points must be absolute paths!");
        self.filesystems.insert(String::from(&mount_point[1..]), filesystem);
    }

    pub fn open(&mut self, path : &str) -> Result<FileHandle, FileError>
    {
        let mut path_parts = parse_path(path)?;

        match self.filesystems.get(path_parts.next().unwrap())
        {
            Some(filesystem) =>
            {
                let file = filesystem.open(filesystem.clone(), &(path_parts.collect() : String))?;
                let handle = FileHandle(self.opened_files.len());
                self.opened_files.insert(handle.clone(), file);
                Ok(handle)
            },

            None =>
            {
                Err(FileError::DoesNotExist)
            },
        }
    }

    pub fn read(&self, handle : &FileHandle) -> Result<Vec<u8>, FileError>
    {
        assert!(self.opened_files.contains_key(&handle), "Tried to read from file handle that isn't open");
        self.opened_files.get(handle).unwrap().read()
    }

    pub fn write(&mut self, handle : &FileHandle, stuff : &[u8]) -> Result<(), FileError>
    {
        assert!(self.opened_files.contains_key(&handle), "Tried to write to file handle that isn't open");
        self.opened_files.get_mut(handle).unwrap().write(stuff)
    }

    pub fn close(&mut self, handle : FileHandle)
    {
        assert!(self.opened_files.contains_key(&handle), "Tried to close file handle that isn't open");
        self.opened_files.remove(&handle).unwrap().close();
    }

    /// Some filesystems may be backed by loaded physical memory (or just physically mapped memory
    /// for open files). This provides that physical mapping, if it exists.
    pub unsafe fn get_physical_mapping(&self, handle : &FileHandle) -> Option<(MemoryAddress,
                                                                               MemoryAddress)>
    {
        let file : &File = self.opened_files.get(handle).unwrap();
        file.filesystem.get_physical_mapping(file)
    }
}
