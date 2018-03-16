/*
 * Copyright (C) 2018, Pebble Developers.
 * See LICENCE.md
 */

use alloc::{String,boxed::Box,BTreeMap};

#[derive(Debug)]
pub enum FileError
{
    DoesNotExist(String),
    MalformedPath(String),
}

pub type Result<T> = ::core::result::Result<T,FileError>;

pub struct File<'a>
{
    name        : String,
    file_system : &'a Filesystem,
}

impl<'a> File<'a>
{
    // pub fn read(&self) -> stuff
    // {
    //     self.file_system.read(&self)
    // }

    // pub fn write(&mut self, stuff : &stuff)
    // {
    //     self.file_system.write(&mut self, stuff);
    // }

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
    fn open(&self, path : &str) -> File;
    fn close(&self, file : File);
    // fn read(&self, file : &File) -> stuff;
    // fn write(&mut self, file : &mut File, stuff : &stuff);
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

    pub fn add_filesystem(&mut self, mount_point : String, filesystem : Box<Filesystem>)
    {
        self.filesystems.insert(mount_point, filesystem);
    }

    pub fn open(&self, path : &str) -> Result<File>
    {
        let filesystem : &Box<Filesystem>;

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
                Ok(filesystem.open(&(path_parts.collect() : String)))
            },

            None =>
            {
                return Err(FileError::DoesNotExist(String::from(path)));
            },
        }
    }
}
