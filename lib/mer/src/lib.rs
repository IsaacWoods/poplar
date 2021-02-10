#![no_std]

pub mod header;
pub mod note;
pub mod program;
pub mod section;
pub mod symbol;

use crate::{
    header::Header,
    program::ProgramHeader,
    section::{SectionHeader, SectionType},
    symbol::Symbol,
};
use core::{marker::PhantomData, mem, str};
use scroll::{ctx::TryFromCtx, Pread};

/// An ELF binary
#[derive(Debug)]
pub struct Elf<'a> {
    bytes: &'a [u8],
    header: Header,
    symbol_table: Option<SectionHeader>,
}

impl Elf<'_> {
    /// Create an `Elf` from a stream of bytes
    pub fn new<'a>(bytes: &'a [u8]) -> Result<Elf<'a>, ElfError> {
        if bytes.len() < mem::size_of::<Header>() {
            return Err(ElfError::TooShort);
        }

        let header = bytes.pread::<Header>(0).map_err(|_| ElfError::MalformedHeader)?;
        header.validate()?;

        let mut elf = Elf { bytes, header, symbol_table: None };

        elf.sections().map(|section| section.validate()).collect::<Result<_, ElfError>>()?;
        elf.segments().map(|segment| segment.validate()).collect::<Result<_, ElfError>>()?;

        // Cache the symbol table, if there is one
        elf.symbol_table = match elf.sections().find(|section| section.name(&elf) == Some(".symtab")) {
            Some(symbol_table) => {
                if symbol_table.section_type() != SectionType::SymTab {
                    return Err(ElfError::InvalidSymbolTable);
                }

                Some(symbol_table)
            }

            None => None,
        };

        Ok(elf)
    }

    /// Create a `SectionIter` that iterates over this ELF's section header.
    pub fn sections(&self) -> EntryIter<SectionHeader> {
        let start = self.header.section_header_offset as usize;
        let end = start
            + self.header.section_header_entry_size as usize * self.header.number_of_section_headers as usize;

        EntryIter::new(
            &self.bytes[start..end],
            self.header.number_of_section_headers as u64,
            self.header.section_header_entry_size as u64,
        )
    }

    pub fn segments(&self) -> EntryIter<ProgramHeader> {
        let start = self.header.program_header_offset as usize;
        let end = start
            + self.header.program_header_entry_size as usize * self.header.number_of_program_headers as usize;

        EntryIter::new(
            &self.bytes[start..end],
            self.header.number_of_program_headers as u64,
            self.header.program_header_entry_size as u64,
        )
    }

    pub fn symbols(&self) -> EntryIter<Symbol> {
        match &self.symbol_table {
            None => EntryIter::new(&[], 0, 0),

            Some(ref symbol_table) => match symbol_table.data(&self) {
                Some(data) => {
                    EntryIter::new(data, symbol_table.size / symbol_table.entry_size, symbol_table.entry_size)
                }
                None => EntryIter::new(&[], 0, 0),
            },
        }
    }

    pub fn entry_point(&self) -> usize {
        self.header.entry_point as usize
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum ElfError {
    /*
     * Errors that can be produced parsing the header.
     */
    /// The provided byte stream was too short to be a valid ELF.
    TooShort,

    /// The header was malformed in some way.
    MalformedHeader,

    /// The magic number at the beginning of the file (should be `0x7f, 'E', 'L', 'F'`) is
    /// incorrect.
    IncorrectMagic,

    /*
     * Errors that can be produced parsing section headers.
     */
    SectionInvalidType,
    /// The `.symtab` section is not actually a symbol table.
    InvalidSymbolTable,

    /*
     * Errors that can be produced parsing program headers.
     */
    SegmentInvalidType,
}

pub struct EntryIter<'a, T: TryFromCtx<'a, scroll::Endian, Error = scroll::Error>> {
    /// Reference to the start of the header / table, within the ELF's byte stream.
    bytes: &'a [u8],

    /// The index of the entry this iterator currently points to.
    current_index: u64,

    /// The number of entries the header / table contains.
    num_entries: u64,

    /// The size of one entry in the header / table.
    entry_size: u64,
    _phantom: PhantomData<T>,
}

impl<'a, T> EntryIter<'a, T>
where
    T: TryFromCtx<'a, scroll::Endian, Error = scroll::Error>,
{
    pub(crate) fn new(bytes: &'a [u8], num_entries: u64, entry_size: u64) -> EntryIter<'a, T> {
        EntryIter { bytes, current_index: 0, num_entries, entry_size, _phantom: PhantomData }
    }
}

impl<'a, T> Iterator for EntryIter<'a, T>
where
    T: TryFromCtx<'a, scroll::Endian, Error = scroll::Error>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index == self.num_entries {
            return None;
        }

        let entry = self
            .bytes
            .pread::<T>((self.current_index * self.entry_size) as usize)
            .expect("Failed to read table / header");
        self.current_index += 1;
        Some(entry)
    }
}

/// Utility function to extract a null-terminated, UTF-8 `&str` from string tables, symbol tables
/// etc.
pub(crate) fn from_utf8_null_terminated(bytes: &[u8]) -> Result<&str, str::Utf8Error> {
    let null_terminator_index = bytes.iter().position(|&c| c == b'\0').unwrap_or(bytes.len());
    str::from_utf8(&bytes[0..null_terminator_index])
}
