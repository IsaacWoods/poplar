pub mod ramdisk;

use alloc::{boxed::Box, rc::Rc, collections::BTreeMap, string::String};
use core::any::Any;
use core::str::Split;

#[derive(Debug)]
pub enum FileError {
    DoesNotExist,
    IsReadOnly,
    MalformedPath,
}

pub struct File {
    pub name: String,
    pub filesystem: Rc<Filesystem>,

    /// This can contain any data the filesystem wants to store per-file, and is accessed
    /// internally
    pub data: Box<Any>,
}

impl File {
    pub fn read<'a>(&'a self) -> Result<&'a [u8], FileError> {
        self.filesystem.read(self)
    }

    pub fn write(&mut self, stuff: &[u8]) -> Result<(), FileError> {
        self.filesystem.write(self, stuff)
    }

    pub fn close(self) {
        self.filesystem.close(&self);
    }
}

impl Drop for File {
    fn drop(&mut self) {
        self.filesystem.close(&self);
    }
}

/// A filesystem is something that contains files (or can be treated as if it abstractly contained
/// files). It provides the implementations for managing the file on it.
/// These do not take the correct types, because if they did we couldn't borrow the filesystem as
/// well. These therefore should not be called manually, only internally. Use the methods on
/// `File` instead.
pub trait Filesystem {
    fn open(&self, filesystem: Rc<Filesystem>, path: &str) -> Result<File, FileError>;
    fn close(&self, file: &File);
    fn read<'a>(&self, file: &'a File) -> Result<&'a [u8], FileError>;
    fn write(&self, file: &File, stuff: &[u8]) -> Result<(), FileError>;
}

fn parse_path<'a>(path: &'a str) -> Result<Split<'a, char>, FileError> {
    // TODO: canonicalise the path
    //  * If not absolute, prepend with current working directory
    //  * Expand '.' and '..'
    //  * Remove assert below

    assert!(path.starts_with('/'), "Path isn't absolute");
    Ok(path[1..].split('/'))
}

/// This manages a set of filesystems and presents them as one virtual filesystem.
pub struct FileManager {
    filesystems: BTreeMap<String, Rc<Filesystem>>,
}

impl FileManager {
    pub fn new() -> FileManager {
        FileManager {
            filesystems: BTreeMap::new(),
        }
    }

    /// Mount a filesystem at the specified path
    pub fn mount(&mut self, mount_point: &str, filesystem: Rc<Filesystem>) {
        assert!(
            mount_point.starts_with('/'),
            "Filesystem mount points must be absolute paths!"
        );
        self.filesystems
            .insert(String::from(&mount_point[1..]), filesystem);
    }

    pub fn open(&mut self, path: &str) -> Result<File, FileError> {
        let mut path_parts = parse_path(path)?;

        match self.filesystems.get(path_parts.next().unwrap()) {
            Some(filesystem) => {
                let file = filesystem.open(filesystem.clone(), &(path_parts.collect(): String))?;
                Ok(file)
            }

            None => Err(FileError::DoesNotExist),
        }
    }
}
