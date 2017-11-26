/*
 * Copyright (C) 2016, Philipp Oppermann.
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#![no_std]

use core::fmt;

use header::{Tag, TagIter};
pub use boot_loader_name::BootLoaderNameTag;
pub use elf_sections::{ElfSectionsTag, ElfSection, ElfSectionIter, ElfSectionType, ElfSectionFlags, StringTable};
pub use elf_sections::{ELF_SECTION_WRITABLE, ELF_SECTION_ALLOCATED, ELF_SECTION_EXECUTABLE};
pub use memory_map::{MemoryMapTag, MemoryArea, MemoryAreaIter};
pub use module::{ModuleTag, ModuleIter};
pub use command_line::CommandLineTag;

#[macro_use] extern crate bitflags;

mod header;
mod boot_loader_name;
mod elf_sections;
mod memory_map;
mod module;
mod command_line;

#[repr(C)]
pub struct MultibootStruct
{
    total_size  : u32,
    _reserved   : u32,
    first_tag   : Tag,
}

pub struct BootInformation
{
    virtual_base : usize,
    multiboot_struct : &'static MultibootStruct,
}

impl BootInformation
{
    /*
     * The Multiboot structure uses physical addresses, so if non-identity paging is used, we need to
     * offset everything by a virtual base.
     */
    pub unsafe fn load(address: usize, virtual_base : usize) -> BootInformation
    {
        assert_eq!(0, address & 0b111);

        let multiboot = &*((address + virtual_base) as *const MultibootStruct);
        assert_eq!(0, multiboot.total_size & 0b111);
    
        let boot_info = BootInformation
                        {
                            virtual_base : virtual_base,
                            multiboot_struct : multiboot,
                        };
        assert!(boot_info.has_valid_end_tag());
        boot_info
    }

    pub fn virtual_base(&self) -> usize
    {
        self.virtual_base
    }

    pub fn multiboot_struct(&self) -> &'static MultibootStruct
    {
        self.multiboot_struct
    }

    pub fn start_address(&self) -> usize
    {
        self.multiboot_struct as *const _ as usize
    }

    pub fn end_address(&self) -> usize
    {
        self.start_address() + self.total_size()
    }

    pub fn total_size(&self) -> usize
    {
        self.multiboot_struct.total_size as usize
    }

    pub fn elf_sections(&self) -> Option<&'static ElfSectionsTag>
    {
        self.tag(9).map(|tag| unsafe { &*(tag as *const Tag as *const ElfSectionsTag) })
    }

    pub fn memory_map(&self) -> Option<&'static MemoryMapTag>
    {
        self.tag(6).map(|tag| unsafe { &*(tag as *const Tag as *const MemoryMapTag) })
    }

    pub fn modules(&self) -> ModuleIter
    {
        module::module_iter(self.tags())
    }

    pub fn boot_loader_name(&self) -> Option<&'static BootLoaderNameTag>
    {
        self.tag(2).map(|tag| unsafe { &*(tag as *const Tag as *const BootLoaderNameTag) })
    }

    pub fn command_line(&self) -> Option<&'static CommandLineTag>
    {
        self.tag(1).map(|tag| unsafe { &*(tag as *const Tag as *const CommandLineTag) })
    }

    fn has_valid_end_tag(&self) -> bool
    {
        const END_TAG : Tag = Tag { typ  : 0,
                                    size : 8
                                  };

        let multiboot_ptr = self.multiboot_struct as *const _;
        let end_tag_addr  = multiboot_ptr as usize + (self.multiboot_struct.total_size - END_TAG.size) as usize;
        let end_tag       = unsafe { &*(end_tag_addr as *const Tag) };

        (end_tag.typ == END_TAG.typ && end_tag.size == END_TAG.size)
    }

    fn tags(&self) -> TagIter
    {
        TagIter
        {
            current : &self.multiboot_struct.first_tag as *const _
        }
    }

    fn tag(&self, typ : u32) -> Option<&'static Tag>
    {
        self.tags().find(|tag| tag.typ == typ)
    }
}

impl fmt::Debug for BootInformation
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result
    {
        writeln!(f, "multiboot information")?;

        writeln!(f, "S: {:#010X}, E: {:#010X}, L: {:#010X}", self.start_address(),
                                                             self.end_address(),
                                                             self.total_size())?;

        if let Some(boot_loader_name_tag) = self.boot_loader_name()
        {
            writeln!(f, "boot loader name: {}", boot_loader_name_tag.name())?;
        }

        if let Some(command_line_tag) = self.command_line()
        {
            writeln!(f, "command line: {}", command_line_tag.command_line())?;
        }

        if let Some(memory_map_tag) = self.memory_map()
        {
            writeln!(f, "memory areas:")?;

            for area in memory_map_tag.memory_areas()
            {
                writeln!(f, "    S: {:#010X}, E: {:#010X}, L: {:#010X}",
                    area.start_address(), area.end_address(), area.size())?;
            }
        }

        if let Some(elf_sections_tag) = self.elf_sections()
        {
            let string_table = elf_sections_tag.string_table(self);
            writeln!(f, "kernel sections:")?;

            for s in elf_sections_tag.sections()
            {
                writeln!(f, "    name: {:15}, S: {:#08X}, E: {:#08X}, L: {:#08X}, F: {:#04X}",
                    string_table.section_name(s), s.start_address(),
                    s.start_address() + s.size(), s.size(), s.flags().bits())?;
            }
        }

        writeln!(f, "module tags:")?;

        for module in self.modules()
        {
            writeln!(f, "    name: {:15}, S: {:#010X}, E: {:#010X}", module.name(),
                                                                     module.start_address(),
                                                                     module.end_address())?;
        }

        Ok(())
    }
}
