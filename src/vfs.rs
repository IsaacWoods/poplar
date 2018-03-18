/*
 * Copyright (C) 2018, Pebble Developers.
 * See LICENCE.md
 */

use core::any::Any;
use alloc::{String,Vec,boxed::Box,BTreeMap};

#[derive(Debug)]
pub enum FileError
{
    DoesNotExist(String),
    MalformedPath(String),
    IsReadOnly(String),
}

pub type Result<T> = ::core::result::Result<T,FileError>;

pub struct File<'a>
{
    pub name        : String,
    pub file_system : &'a Filesystem,
    pub data        : Box<Any>,
}

impl<'a> File<'a>
{
    pub fn read(&self) -> Result<Vec<u8>>
    {
        self.file_system.read(self)
    }

    pub fn write(&mut self, stuff : &[u8]) -> Result<()>
    {
        self.file_system.write(self, stuff)
    }

    pub fn close(self)
    {
        self.file_system.close(self);
    }
}

/*
 * A filesystem is something that contains files (or can be treated as if it abstractly contained
 * files). It provides the implementations for managing the file on it.
 */
pub trait Filesystem
{
    fn open(&self, path : &str) -> Result<File>;
    fn close(&self, file : File);
    fn read(&self, file : &File) -> Result<Vec<u8>>;
    fn write(&self, file : &mut File, stuff : &[u8]) -> Result<()>;
}

/*
 * This manages a set of filesystems and presents them as one virtual filesystem.
 */
pub struct FileManager
{
    filesystems : BTreeMap<String, Box<Filesystem>>,
}

impl FileManager
{
    pub fn new() -> FileManager
    {
        FileManager
        {
            filesystems : BTreeMap::new(),
        }
    }

    pub fn add_filesystem(&mut self, mount_point : &str, filesystem : Box<Filesystem>)
    {
        assert!(mount_point.starts_with('/'), "Filesystem mount points must be absolute paths!");
        self.filesystems.insert(String::from(&mount_point[1..]), filesystem);
    }

    pub fn open(&self, path : &str) -> Result<File>
    {
        if !path.starts_with('/')
        {
            return Err(FileError::MalformedPath(String::from(path)));
        }

        /*
         * We are searching from the root of the filesystem. Therefore, the first part of the
         * path will be the mount point of the filesystem.
         */
        let mut path_parts = path[1..].split('/');

        match self.filesystems.get(path_parts.next().unwrap())
        {
            Some(filesystem) =>
            {
                filesystem.open(&(path_parts.collect() : String))
            },

            None =>
            {
                Err(FileError::DoesNotExist(String::from(path)))
            },
        }
    }
}
