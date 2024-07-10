use crate::ElfError;
use scroll::Pread;

/// The ELF header
#[derive(Debug, Default, Pread)]
#[repr(C)]
pub struct Header {
    pub magic: [u8; 4],
    pub class: u8,
    pub data: u8,
    pub header_version: u8,
    pub abi: u8,
    pub abi_version: u8,
    _padding: [u8; 7],
    pub file_type: u16,
    pub machine_type: u16,
    pub version: u32,
    pub entry_point: u64,
    pub program_header_offset: u64,
    pub section_header_offset: u64,
    pub flags: u32,
    pub header_size: u16,
    pub program_header_entry_size: u16,
    pub number_of_program_headers: u16,
    pub section_header_entry_size: u16,
    pub number_of_section_headers: u16,

    /// This is the section index of the string table that contains the names of the sections.
    pub string_table_index: u16,
}

impl Header {
    pub fn validate(&self) -> Result<(), ElfError> {
        if self.magic != [0x7f, b'E', b'L', b'F'] {
            return Err(ElfError::IncorrectMagic);
        }

        Ok(())
    }
}
