use scroll_derive::Pread;
use bit_field::BitField;
use crate::{Elf, section::SectionType};

pub enum SymbolBinding {
    /// Only visible inside the object file that defines it.
    Local,
    
    /// Global symbol - visible to all object files.
    Global,

    /// Global scope, but with a lower precedence than global symbols.
    Weak,

    /// Environment-specific use.
    Os(u8),

    /// Processor-specific use.
    Proc(u8),
}

pub enum SymbolType {
    NoType,
    Object,
    Func,
    Section,
    File,
    Os(u8),
    Proc(u8),
}

#[derive(Pread)]
#[repr(C)]
pub struct Symbol {
    /// The offset into the string table, in bytes, to the symbol name. If this is `0`, the symbol
    /// doesn't have a name.
    pub name: u32,
    pub info: u8,
    /// Reserved. Must be `0`.
    _other: u8,
    pub section_table_index: u16,
    pub value: u64,
    pub size: u64,
}

impl Symbol {
    pub fn binding(&self) -> SymbolBinding {
        let binding = self.info.get_bits(4..8);
        match binding {
            0 => SymbolBinding::Local,
            1 => SymbolBinding::Global,
            2 => SymbolBinding::Weak,
            10..=12 => SymbolBinding::Os(binding),
            13..=15 => SymbolBinding::Proc(binding),
            _ => panic!("Invalid symbol binding: {}", binding),
        }
    }

    pub fn symbol_type(&self) -> SymbolType {
        let symbol_type = self.info.get_bits(0..4);
        match symbol_type {
            0 => SymbolType::NoType,
            1 => SymbolType::Object,
            2 => SymbolType::Func,
            3 => SymbolType::Section,
            4 => SymbolType::File,
            10..=12 => SymbolType::Os(symbol_type),
            13..=15 => SymbolType::Proc(symbol_type),
            _ => panic!("Invalid symbol type: {}", symbol_type),
        }
    }

    pub fn name<'a>(&self, elf: &'a Elf) -> Option<&'a str> {
        if self.name == 0 {
            return None;
        }

        match &elf.symbol_table {
            /*
             * NOTE: This is unreachable because we can't create symbols without a symbol table.
             */
            None => unreachable!(),

            Some(ref symbol_table) => {
                /*
                 * NOTE: the `link` field of the symbol table contains the index of the string
                 * table that contains the names of the symbols.
                 */
                let string_table = elf.sections().nth(symbol_table.link as usize).unwrap();

                if string_table.section_type() != SectionType::StrTab {
                    return None;
                }

                crate::from_utf8_null_terminated(&string_table.data(elf)?[(self.name as usize)..]).ok()
            }
        }
    }
}
