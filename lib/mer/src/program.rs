use crate::{Elf, ElfError};
use bit_field::BitField;
use scroll_derive::Pread;

#[derive(PartialEq, Eq, Debug)]
pub enum SegmentType {
    Null,
    Load,
    Dynamic,
    Interp,
    Note,
    Shlib,
    Phdr,

    /// A section with type `0x60000000` through `0x6fffffff` inclusive is defined to be
    /// environment-specific.
    Os(u32),

    /// A section with type `0x70000000` through `0x7fffffff` inclusive is defined to be
    /// processor-specific.
    Proc(u32),
}

#[derive(Debug, Pread)]
#[repr(C)]
pub struct ProgramHeader {
    pub segment_type: u32,
    pub flags: u32,
    pub offset: u64,
    pub virtual_address: u64,
    pub physical_address: u64,
    pub file_size: u64,
    pub mem_size: u64,
    pub alignment: u64,
}

impl ProgramHeader {
    pub(crate) fn validate(&self) -> Result<(), ElfError> {
        match self.segment_type {
            0..=6 | 0x60000000..=0x7fffffff => Ok(()),
            _ => Err(ElfError::SegmentInvalidType),
        }?;

        Ok(())
    }

    pub fn segment_type(&self) -> SegmentType {
        match self.segment_type {
            0 => SegmentType::Null,
            1 => SegmentType::Load,
            2 => SegmentType::Dynamic,
            3 => SegmentType::Interp,
            4 => SegmentType::Note,
            5 => SegmentType::Shlib,
            6 => SegmentType::Phdr,
            0x60000000..=0x6fffffff => SegmentType::Os(self.segment_type),
            0x70000000..=0x7fffffff => SegmentType::Proc(self.segment_type),

            _ => panic!("segment_type called on segment with invalid type. Was validate called?"),
        }
    }

    pub fn data<'a>(&self, elf: &'a Elf) -> &'a [u8] {
        &elf.bytes[(self.offset as usize)..((self.offset + self.file_size) as usize)]
    }

    pub fn is_executable(&self) -> bool {
        self.flags.get_bit(0)
    }

    pub fn is_writable(&self) -> bool {
        self.flags.get_bit(1)
    }

    pub fn is_readable(&self) -> bool {
        self.flags.get_bit(2)
    }
}
