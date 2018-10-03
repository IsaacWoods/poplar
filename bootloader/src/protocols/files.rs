use core::{mem, ops::Drop, slice};
use crate::boot_services::{utf16_to_str, BootServices, Guid, Pool, Protocol};
use crate::types::{Bool, Char16, BootMemory, Status};
use crate::memory::MemoryType;

/// Provides file based access to supported file systems
#[repr(C)]
pub struct File {
    pub revision: u64,
    pub _open: extern "win64" fn(
        this: &File,
        new_handle: &mut BootMemory<File>,
        file_name: *const Char16,
        open_mode: FileMode,
        attributes: FileAttributes,
    ) -> Status,
    pub _close: extern "win64" fn(this: &File) -> Status,
    pub _delete: extern "win64" fn() -> Status,
    pub _read: extern "win64" fn(this: &File, buffer_size: &mut usize, buffer: *mut u8) -> Status,
    pub _write: extern "win64" fn() -> Status,
    pub _get_position: extern "win64" fn() -> Status,
    pub _set_position: extern "win64" fn() -> Status,
    pub _get_info: extern "win64" fn(
        this: &File,
        information_type: &Guid,
        buffer_size: &mut usize,
        buffer: *mut u8,
    ) -> Status,
    pub _set_info: extern "win64" fn() -> Status,
    pub _flush: extern "win64" fn() -> Status,
}

impl File {
    /// Opens a new file relative to this file's location
    pub fn open(
        &self,
        file_name: &[Char16],
        open_mode: FileMode,
        attributes: FileAttributes,
    ) -> Result<BootMemory<File>, Status> {
        let mut file = unsafe { BootMemory::new() };
        (self._open)(self, &mut file, file_name.as_ptr(), open_mode, attributes).as_result()?;

        if file.is_null() {
            Err(Status::NotFound)
        } else {
            Ok(file)
        }
    }

    /// Reads data from this file
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, Status> {
        let mut len = buf.len();
        (self._read)(self, &mut len, buf.as_mut_ptr())
            .as_result()
            .map(|_| len)
    }

    /// Returns information about a file
    pub fn get_info<'a, T>(&self, boot_services: &'a BootServices) -> Result<Pool<'a, T>, Status>
    where
        T: FileInformationType + Sized,
    {
        let mut buf_size = mem::size_of::<T>();
        let buf = boot_services.allocate_pool(MemoryType::LoaderData, buf_size)?;
        let res = (self._get_info)(self, T::guid(), &mut buf_size, buf);
        if res == Status::Success {
            // If the initial buffer happened to be large enough, return it
            // This should never happen, because the length of the file name or volume label should
            // always be greater than 1
            return Ok(unsafe { Pool::new_unchecked(buf as *mut T, boot_services) });
        } else if res != Status::BufferTooSmall {
            return Err(res);
        }

        // Reallocate the buffer with the specified size
        boot_services.free_pool(buf)?;
        let buf = boot_services.allocate_pool(MemoryType::LoaderData, buf_size)?;
        (self._get_info)(self, T::guid(), &mut buf_size, buf)
            .as_result()
            .map(|_| unsafe { Pool::new_unchecked(buf as *mut T, boot_services) })
    }
}

impl Drop for File {
    fn drop(&mut self) {
        let _ = (self._close)(self);
    }
}

bitflags! {
    /// Attribute bits for a file
    pub struct FileAttributes: u64 {
        const READ_ONLY = 0x0000_0000_0000_0001;
        const HIDDEN = 0x0000_0000_0000_0002;
        const SYSTEM = 0x0000_0000_0000_0004;
        const RESERVED = 0x0000_0000_0000_0008;
        const DIRECTORY = 0x0000_0000_0000_0010;
        const ARCHIVE = 0x0000_0000_0000_0020;
        const VALID_ATTR = 0x0000_0000_0000_0037;
    }
}

bitflags! {
    /// Mode to open a file
    pub struct FileMode: u64 {
        const READ = 0x0000_0000_0000_0001;
        const WRITE = 0x0000_0000_0000_0002;
        const CREATE = 0x8000_0000_0000_0000;
    }
}

/// Type of information that can be retrieved about a file
pub trait FileInformationType {
    fn guid() -> &'static Guid;
}

/// Generic information about a file
#[derive(Debug)]
#[repr(C)]
pub struct FileInfo {
    pub size: u64,
    pub file_size: u64,
    pub physical_size: u64,
    pub create_time: usize,       // TODO
    pub last_access_time: usize,  // TODO
    pub modification_time: usize, // TODO
    pub attribute: u64,           // TODO
    _file_name: Char16,
}

impl FileInformationType for FileInfo {
    fn guid() -> &'static Guid {
        &FILE_INFO_GUID
    }
}

/// Information about the system volume
#[derive(Debug)]
#[repr(C)]
pub struct FileSystemInfo {
    _size: usize,
    pub read_only: Bool,
    pub volume_size: u64,
    pub free_space: u64,
    pub block_size: u32,
    _volume_label: Char16,
}

impl FileSystemInfo {
    pub fn volume_label<'a>(
        &self,
        boot_services: &'a BootServices,
    ) -> Result<Pool<'a, str>, Status> {
        let buf = unsafe {
            let buf_size =
                self._size - (mem::size_of::<FileSystemInfo>() - mem::size_of::<Char16>());
            slice::from_raw_parts(&(self._volume_label), buf_size)
        };

        utf16_to_str(buf, boot_services)
    }
}

impl FileInformationType for FileSystemInfo {
    fn guid() -> &'static Guid {
        &FILE_SYSTEM_INFO_GUID
    }
}

static FILE_INFO_GUID: Guid = Guid {
    data_1: 0x09576e92,
    data_2: 0x6d3f,
    data_3: 0x11d2,
    data_4: [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
};

static FILE_SYSTEM_INFO_GUID: Guid = Guid {
    data_1: 0x09576e93,
    data_2: 0x6d3f,
    data_3: 0x11d2,
    data_4: [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
};

/// Provides a minimal interface for file-type access to a device
#[repr(C)]
pub struct SimpleFileSystem {
    pub revision: u64,
    pub _open_volume: extern "win64" fn(this: &SimpleFileSystem, root: &mut BootMemory<File>) -> Status,
}

impl SimpleFileSystem {
    /// Opens the root directory on a volume
    pub fn open_volume(&self) -> Result<BootMemory<File>, Status> {
        let mut file = unsafe { BootMemory::new() };
        (self._open_volume)(self, &mut file).as_result()?;

        if file.is_null() {
            Err(Status::NotFound)
        } else {
            Ok(file)
        }
    }
}

impl Protocol for SimpleFileSystem {
    fn guid() -> &'static Guid {
        &SIMPLE_FILE_SYSTEM_GUID
    }
}

static SIMPLE_FILE_SYSTEM_GUID: Guid = Guid {
    data_1: 0x0964e5b22,
    data_2: 0x6459,
    data_3: 0x11d2,
    data_4: [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
};
