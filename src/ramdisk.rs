/*
 * Copyright (C) 2018, Pebble Developers.
 * See LICENCE.md
 *
 * We use a TAR archive as a ramdisk. It contains a number of 512-byte headers, one for each file,
 * followed by the content.
 */

use core::{mem,str,slice};
use alloc::{String,Vec,boxed::Box};
use ::arch::MemoryAddress;
use ::vfs::{File,Filesystem,Result,FileError};

#[derive(Clone,Copy)]
#[repr(C)]
struct TarHeader
{
    filename    : [u8; 100],
    mode        : [u8; 8],
    uid         : [u8; 8],
    gid         : [u8; 8],
    size        : [u8; 12],
    mtime       : [u8; 12],
    chksum      : [u8; 8],
    typeflag    : u8,
    _padding    : [u8; 512-100-8-8-8-12-12-8-1],
}

#[derive(Clone)]
struct RamdiskFileData
{
    path    : String,
    pointer : *const u8,
    size    : usize,
}

impl TarHeader
{
    /*
     * The size in a TAR is written in octal ASCII characters.
     */
    fn size(&self) -> usize
    {
        const FIELD_LENGTH : usize = 12;

        let mut size : usize = 0;
        let mut place = 1;

        for i in (0..FIELD_LENGTH-1).rev()
        {
            size += (self.size[i] - b'0') as usize * place;
            place *= 8;
        }

        size
    }
}

pub struct Ramdisk
{
    start   : MemoryAddress,
    end     : MemoryAddress,
    files   : Vec<RamdiskFileData>,
}

impl Ramdisk
{
    pub fn new(start : MemoryAddress, end : MemoryAddress) -> Ramdisk
    {
        assert!(mem::size_of::<TarHeader>() == 512);

        let mut ramdisk = Ramdisk
                          {
                              start : start,
                              end   : end,
                              files : Vec::new(),
                          };

        ramdisk.parse_headers();
        ramdisk
    }

    fn parse_headers(&mut self)
    {
        unsafe
        {
            let mut header_address = self.start;
    
            loop
            {
                let header_ptr = header_address as *const TarHeader;

                if (*header_ptr).filename[0] == b'\0'
                {
                    info!("Found {} files on ramdisk", self.files.len());
                    return;
                }

                let size = (*header_ptr).size();

                /*
                 * We have to strip the trailing null bytes from the end of the slice.
                 */
                let mut filename = String::from(str::from_utf8(&(*header_ptr).filename).expect("Couldn't decode TAR header filename"));
                filename.retain(|c| c != '\u{0}');

                self.files.push(RamdiskFileData
                                {
                                    path    : filename,
                                    pointer : (header_address + mem::size_of::<TarHeader>()) as *const u8,
                                    size,
                                });

                header_address += ((size / 512) + 1) * 512;
                if size % 512 != 0
                {
                    header_address += 512;
                }
            }
        }

    }
}

impl Filesystem for Ramdisk
{
    fn open(&self, path : &str) -> Result<File>
    {
        for file in self.files.iter()
        {
            if path == file.path
            {
                return Ok(File
                          {
                              name          : String::from(path),
                              file_system   : self,
                              data          : Box::new(file.clone()),
                          });
            }
        }

        Err(FileError::DoesNotExist(String::from(path)))
    }

    fn close(&self, _file : File)
    {
    }

    fn read(&self, file : &File) -> Result<Vec<u8>>
    {
        let file_data = file.data.downcast_ref::<RamdiskFileData>().unwrap();
        Ok(unsafe { slice::from_raw_parts(file_data.pointer, file_data.size) }.to_vec())
    }

    fn write(&self, file : &mut File, _ : &[u8]) -> Result<()>
    {
        Err(FileError::IsReadOnly(file.name.clone()))
    }
}
