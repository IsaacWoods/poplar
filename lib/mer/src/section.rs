use crate::{Elf, ElfError};
use bit_field::BitField;
use core::str;
use scroll::Pread;

#[derive(PartialEq, Eq)]
pub enum SectionType {
    /// The first section in a valid ELF's section table will be a null section. It does not detail
    /// a real section.
    Null,

    /// Contains information defined by the program.
    ProgBits,

    /// Contains a symbol table.
    SymTab,

    /// Contains a string table.
    StrTab,

    /// Contains "Rela"-type relocations.
    Rela,

    /// Contains a symbol hash table.
    Hash,

    /// Contains tables used during dynamic linking.
    Dynamic,

    /// Contains note information.
    Note,

    /// Defines a section as containing uninitialized space. This section does not take up any
    /// space in the file and is usually loaded with `0`s during program loading.
    NoBits,

    /// Contains "Rel"-type relocations.
    Rel,

    /// Reserved by the spec.
    ShLib,

    /// Contains a dynamic loader symbol table.
    DynSym,

    /// A section with type `0x60000000` through `0x6fffffff` inclusive is defined to be
    /// environment-specific.
    Os(u32),

    /// A section with type `0x70000000` through `0x7fffffff` inclusive is defined to be
    /// processor-specific.
    Proc(u32),
}

#[derive(Debug, Pread)]
#[repr(C)]
pub struct SectionHeader {
    pub name: u32,
    pub section_type: u32,
    pub flags: u64,
    pub address: u64,
    pub offset: u64,
    pub size: u64,

    /// Some sections are 'linked' to another section. This field contains the index of the linked
    /// section.
    pub link: u32,

    /// Can contain extra information about a section.
    pub info: u32,
    pub alignment: u64,

    /// If this section contains a table, this is the size of one entry
    pub entry_size: u64,
}

impl SectionHeader {
    pub(crate) fn validate(&self) -> Result<(), ElfError> {
        match self.section_type {
            0..=11 | 0x60000000..=0x7fffffff => Ok(()),
            _ => Err(ElfError::SectionInvalidType),
        }?;

        Ok(())
    }

    pub fn section_type(&self) -> SectionType {
        match self.section_type {
            0 => SectionType::Null,
            1 => SectionType::ProgBits,
            2 => SectionType::SymTab,
            3 => SectionType::StrTab,
            4 => SectionType::Rela,
            5 => SectionType::Hash,
            6 => SectionType::Dynamic,
            7 => SectionType::Note,
            8 => SectionType::NoBits,
            9 => SectionType::Rel,
            10 => SectionType::ShLib,
            11 => SectionType::DynSym,
            0x60000000..=0x6fffffff => SectionType::Os(self.section_type),
            0x70000000..=0x7fffffff => SectionType::Proc(self.section_type),

            _ => panic!("section_type called on section with invalid type. Was validate called?"),
        }
    }

    pub fn name<'a>(&self, elf: &'a Elf) -> Option<&'a str> {
        if self.name == 0 {
            return None;
        }

        let string_table = elf.sections().nth(elf.header.string_table_index as usize)?;
        crate::from_utf8_null_terminated(&string_table.data(elf)?[(self.name as usize)..]).ok()
    }

    /// Get this section's data, as a byte slice. Returns `None` if the image isn't represented in
    /// the file (for example, `NoBits` sections don't have any data).
    pub fn data<'a>(&self, elf: &'a Elf) -> Option<&'a [u8]> {
        match self.section_type() {
            SectionType::Null | SectionType::NoBits => return None,
            _ => (),
        }

        Some(&elf.bytes[(self.offset as usize)..((self.offset + self.size) as usize)])
    }

    /// Whether this section contains writable data
    pub fn is_writable(&self) -> bool {
        self.flags.get_bit(0)
    }

    /// Whether this section should be allocated into the memory image of the program
    pub fn is_allocated(&self) -> bool {
        self.flags.get_bit(1)
    }

    /// Whether this section contains executable instructions
    pub fn is_executable(&self) -> bool {
        self.flags.get_bit(2)
    }
}
