use alloc::slice;
use core::{ffi::CStr, mem};

pub struct Elf<'a> {
    pub bytes: &'a [u8],
}

impl Elf<'_> {
    pub fn new(bytes: &[u8]) -> Elf<'_> {
        assert!(bytes.len() > mem::size_of::<Header>());
        Elf { bytes }
    }

    pub fn header(&self) -> &Header {
        unsafe { &*(self.bytes.as_ptr() as *const Header) }
    }

    pub fn validate(&self) -> Result<(), &str> {
        let header = self.header();

        if header.magic != [0x7f, b'E', b'L', b'F'] {
            return Err("ELF magic is incorrect!");
        }
        // TODO: validate machine type, exe type, etc.

        Ok(())
    }

    pub fn segments(&self) -> &[ProgramHeader] {
        let header = self.header();
        unsafe {
            slice::from_raw_parts(
                self.bytes.as_ptr().byte_add(header.ph_offset as usize) as *const ProgramHeader,
                header.num_ph as usize,
            )
        }
    }

    pub fn sections(&self) -> &[SectionHeader] {
        let header = self.header();
        unsafe {
            slice::from_raw_parts(
                self.bytes.as_ptr().byte_add(header.sh_offset as usize) as *const SectionHeader,
                header.num_sh as usize,
            )
        }
    }

    pub fn section_data(&self, section: &SectionHeader) -> &[u8] {
        unsafe {
            slice::from_raw_parts(
                self.bytes.as_ptr().byte_add(section.offset as usize),
                section.size as usize,
            )
        }
    }

    pub fn read_string(&self, str_tab: Option<u16>, offset: u32) -> Result<&str, ()> {
        let header = self.header();
        let str_tab_index = str_tab.unwrap_or(header.string_table_index);
        let str_tab = &self.sections()[str_tab_index as usize];

        let data = self.section_data(&str_tab);
        Ok(CStr::from_bytes_until_nul(&data[(offset as usize)..])
            .unwrap()
            .to_str()
            .unwrap())
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct Header {
    pub magic: [u8; 4],
    pub class: u8,
    pub data: u8,
    pub header_version: u8,
    pub abi: u8,
    pub abi_version: u8,
    pub _padding: [u8; 7],
    pub typ: u16,
    pub machine: u16,
    pub version: u32,
    pub entry_point: u64,
    pub ph_offset: u64,
    pub sh_offset: u64,
    pub flags: u32,
    pub header_size: u16,
    pub ph_entry_size: u16,
    pub num_ph: u16,
    pub sh_entry_size: u16,
    pub num_sh: u16,
    pub string_table_index: u16,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct ProgramHeader {
    pub typ: u32,
    pub flags: ProgramHeaderFlags,
    pub offset: u64,
    pub vaddr: u64,
    pub paddr: u64,
    pub file_size: u64,
    pub mem_size: u64,
    pub align: u64,
}

mycelium_bitfield::bitfield! {
    pub struct ProgramHeaderFlags<u32> {
        pub const EXECUTABLE: bool;
        pub const WRITABLE: bool;
        pub const READABLE: bool;
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SegmentType {
    Null,
    Load,
    Dynamic,
    Interp,
    Note,
    Shlib,
    Phdr,
    Tls,
    Os(u32),
    Proc(u32),
}

impl ProgramHeader {
    pub fn typ(&self) -> SegmentType {
        match self.typ {
            0 => SegmentType::Null,
            1 => SegmentType::Load,
            2 => SegmentType::Dynamic,
            3 => SegmentType::Interp,
            4 => SegmentType::Note,
            5 => SegmentType::Shlib,
            6 => SegmentType::Phdr,
            7 => SegmentType::Tls,
            0x6000_0000..=0x6fff_ffff => SegmentType::Os(self.typ),
            0x7000_0000..=0x7fff_ffff => SegmentType::Proc(self.typ),
            _ => panic!("Unrecognised segment type in ELF: {:#x}", self.typ),
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct SectionHeader {
    pub name_offset: u32,
    pub section_typ: u32,
    pub flags: u64,
    pub address: u64,
    pub offset: u64,
    pub size: u64,
    pub link: u32,
    pub info: u32,
    pub alignment: u32,
    pub entry_size: u64,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Symbol {
    /// Offset into the string table, of the symbol name. `0` indicates no name.
    pub name_offset: u32,
    pub info: u8,
    _reserved: u8,
    pub section_table_index: u16,
    pub value: u64,
    pub size: u64,
}
