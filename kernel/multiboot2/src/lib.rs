#![no_std]

use core::fmt;

use header::{Tag, TagIter};
pub use boot_loader_name::BootLoaderNameTag;
pub use elf_sections::{ElfSectionsTag, ElfSection, ElfSectionIter, ElfSectionType, ElfSectionFlags, StringTable};
pub use elf_sections::{ELF_SECTION_WRITABLE, ELF_SECTION_ALLOCATED, ELF_SECTION_EXECUTABLE};
pub use memory_map::{MemoryMapTag, MemoryArea, MemoryAreaIter};
pub use module::{ModuleTag, ModuleIter};
pub use command_line::CommandLineTag;

#[macro_use]
extern crate bitflags;

mod header;
mod boot_loader_name;
mod elf_sections;
mod memory_map;
mod module;
mod command_line;

pub unsafe fn load(address: usize) -> &'static BootInformation {
    if !cfg!(test) {
        assert_eq!(0, address & 0b111);
    }
    let multiboot = &*(address as *const BootInformation);
    assert_eq!(0, multiboot.total_size & 0b111);
    assert!(multiboot.has_valid_end_tag());
    multiboot
}

#[repr(C)]
pub struct BootInformation {
    total_size: u32,
    _reserved: u32,
    first_tag: Tag,
}

impl BootInformation {
    pub fn start_address(&self) -> usize {
        self as *const _ as usize
    }

    pub fn end_address(&self) -> usize {
        self.start_address() + self.total_size()
    }

    pub fn total_size(&self) -> usize {
        self.total_size as usize
    }

    pub fn elf_sections_tag(&self) -> Option<&'static ElfSectionsTag> {
        self.get_tag(9).map(|tag| unsafe { &*(tag as *const Tag as *const ElfSectionsTag) })
    }

    pub fn memory_map_tag(&self) -> Option<&'static MemoryMapTag> {
        self.get_tag(6).map(|tag| unsafe { &*(tag as *const Tag as *const MemoryMapTag) })
    }

    pub fn module_tags(&self) -> ModuleIter {
        module::module_iter(self.tags())
    }

    pub fn boot_loader_name_tag(&self) -> Option<&'static BootLoaderNameTag> {
        self.get_tag(2).map(|tag| unsafe { &*(tag as *const Tag as *const BootLoaderNameTag) })
    }

    pub fn command_line_tag(&self) -> Option<&'static CommandLineTag> {
        self.get_tag(1).map(|tag| unsafe { &*(tag as *const Tag as *const CommandLineTag) })
    }

    fn has_valid_end_tag(&self) -> bool {
        const END_TAG: Tag = Tag{typ:0, size:8};

        let self_ptr = self as *const _;
        let end_tag_addr = self_ptr as usize + (self.total_size - END_TAG.size) as usize;
        let end_tag = unsafe{&*(end_tag_addr as *const Tag)};

        end_tag.typ == END_TAG.typ && end_tag.size == END_TAG.size
    }

    fn get_tag(&self, typ: u32) -> Option<&'static Tag> {
        self.tags().find(|tag| tag.typ == typ)
    }

    fn tags(&self) -> TagIter {
        TagIter { current: &self.first_tag as *const _ }
    }
}

impl fmt::Debug for BootInformation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "multiboot information")?;

        writeln!(f, "S: {:#010X}, E: {:#010X}, L: {:#010X}",
            self.start_address(), self.end_address(), self.total_size())?;

        if let Some(boot_loader_name_tag) = self.boot_loader_name_tag() {
            writeln!(f, "boot loader name: {}", boot_loader_name_tag.name())?;
        }

        if let Some(command_line_tag) = self.command_line_tag() {
            writeln!(f, "command line: {}", command_line_tag.command_line())?;
        }

        if let Some(memory_map_tag) = self.memory_map_tag() {
            writeln!(f, "memory areas:")?;
            for area in memory_map_tag.memory_areas() {
                writeln!(f, "    S: {:#010X}, E: {:#010X}, L: {:#010X}",
                    area.start_address(), area.end_address(), area.size())?;
            }
        }

        if let Some(elf_sections_tag) = self.elf_sections_tag() {
            let string_table = elf_sections_tag.string_table();
            writeln!(f, "kernel sections:")?;
            for s in elf_sections_tag.sections() {
                writeln!(f, "    name: {:15}, S: {:#08X}, E: {:#08X}, L: {:#08X}, F: {:#04X}",
                    string_table.section_name(s), s.start_address(),
                    s.start_address() + s.size(), s.size(), s.flags().bits())?;
            }
        }

        writeln!(f, "module tags:")?;
        for mt in self.module_tags() {
            writeln!(f, "    name: {:15}, S: {:#010X}, E: {:#010X}",
                mt.name(), mt.start_address(), mt.end_address())?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::load;
    use super::{ElfSectionFlags, ELF_SECTION_EXECUTABLE, ELF_SECTION_ALLOCATED, ELF_SECTION_WRITABLE};
    use super::ElfSectionType;

    #[test]
    fn no_tags() {
        let bytes: [u8; 16] = [
            16, 0, 0, 0, // total_size
            0, 0, 0, 0,  // reserved
            0, 0, 0, 0,  // end tag type
            8, 0, 0, 0,  // end tag size
        ];
        let addr = bytes.as_ptr() as usize;
        let bi = unsafe { load(addr) };
        assert_eq!(addr, bi.start_address());
        assert_eq!(addr + bytes.len(), bi.end_address());
        assert_eq!(bytes.len(), bi.total_size());
        assert!(bi.elf_sections_tag().is_none());
        assert!(bi.memory_map_tag().is_none());
        assert!(bi.module_tags().next().is_none());
        assert!(bi.boot_loader_name_tag().is_none());
        assert!(bi.command_line_tag().is_none());
    }


    #[test]
    #[should_panic]
    fn invalid_total_size() {
        let bytes: [u8; 15] = [
            15, 0, 0, 0, // total_size
            0, 0, 0, 0,  // reserved
            0, 0, 0, 0,  // end tag type
            8, 0, 0,     // end tag size
        ];
        let addr = bytes.as_ptr() as usize;
        let bi = unsafe { load(addr) };
        assert_eq!(addr, bi.start_address());
        assert_eq!(addr + bytes.len(), bi.end_address());
        assert_eq!(bytes.len(), bi.total_size());
        assert!(bi.elf_sections_tag().is_none());
        assert!(bi.memory_map_tag().is_none());
        assert!(bi.module_tags().next().is_none());
        assert!(bi.boot_loader_name_tag().is_none());
        assert!(bi.command_line_tag().is_none());
    }


    #[test]
    #[should_panic]
    fn invalid_end_tag() {
        let bytes: [u8; 16] = [
            16, 0, 0, 0, // total_size
            0, 0, 0, 0,  // reserved
            0, 0, 0, 0,  // end tag type
            9, 0, 0, 0,  // end tag size
        ];
        let addr = bytes.as_ptr() as usize;
        let bi = unsafe { load(addr) };
        assert_eq!(addr, bi.start_address());
        assert_eq!(addr + bytes.len(), bi.end_address());
        assert_eq!(bytes.len(), bi.total_size());
        assert!(bi.elf_sections_tag().is_none());
        assert!(bi.memory_map_tag().is_none());
        assert!(bi.module_tags().next().is_none());
        assert!(bi.boot_loader_name_tag().is_none());
        assert!(bi.command_line_tag().is_none());
    }

    #[test]
    fn name_tag() {
        let bytes: [u8; 32] = [
            32, 0, 0, 0,       // total_size
            0, 0, 0, 0,        // reserved
            2, 0, 0, 0,        // boot loader name tag type
            13, 0, 0, 0,       // boot loader name tag size
            110, 97, 109, 101, // boot loader name 'name'
            0, 0, 0, 0,        // boot loader name null + padding
            0, 0, 0, 0,        // end tag type
            8, 0, 0, 0,        // end tag size
        ];
        let addr = bytes.as_ptr() as usize;
        let bi = unsafe { load(addr) };
        assert_eq!(addr, bi.start_address());
        assert_eq!(addr + bytes.len(), bi.end_address());
        assert_eq!(bytes.len(), bi.total_size());
        assert!(bi.elf_sections_tag().is_none());
        assert!(bi.memory_map_tag().is_none());
        assert!(bi.module_tags().next().is_none());
        assert_eq!("name", bi.boot_loader_name_tag().unwrap().name());
        assert!(bi.command_line_tag().is_none());
    }

    #[cfg(not(feature = "elf32"))]
    #[test]
    fn grub2() {
        let mut bytes: [u8; 960] = [
            192, 3, 0, 0,       // total_size
            0, 0, 0, 0,         // reserved
            1, 0, 0, 0,         // boot command tag type
            9, 0, 0, 0,         // boot command tag size
            0, 0, 0, 0,         // boot command null + padding
            0, 0, 0, 0,         // boot command padding
            2, 0, 0, 0,         // boot loader name tag type
            26, 0, 0, 0,        // boot loader name tag size
            71, 82, 85, 66,     // boot loader name
            32, 50, 46, 48,     // boot loader name
            50, 126, 98, 101,   // boot loader name
            116, 97, 51, 45,    // boot loader name
            53, 0, 0, 0,        // boot loader name null + padding
            0, 0, 0, 0,         // boot loader name padding
            10, 0, 0, 0,        // APM tag type
            28, 0, 0, 0,        // APM tag size
            2, 1, 0, 240,       // APM version, cseg
            207, 212, 0, 0,     // APM offset
            0, 240, 0, 240,     // APM cseg_16, dseg
            3, 0, 240, 255,     // APM flags, cseg_len
            240, 255, 240, 255, // APM cseg_16_len, dseg_len
            0, 0, 0, 0,         // APM padding
            6, 0, 0, 0,         // memory map tag type
            160, 0, 0, 0,       // memory map tag size
            24, 0, 0, 0,        // memory map entry_size
            0, 0, 0, 0,         // memory map entry_version
            0, 0, 0, 0,         // memory map entry 0 base_addr
            0, 0, 0, 0,         // memory map entry 0 base_addr
            0, 252, 9, 0,       // memory map entry 0 length
            0, 0, 0, 0,         // memory map entry 0 length
            1, 0, 0, 0,         // memory map entry 0 type
            0, 0, 0, 0,         // memory map entry 0 reserved
            0, 252, 9, 0,       // memory map entry 1 base_addr
            0, 0, 0, 0,         // memory map entry 1 base_addr
            0, 4, 0, 0,         // memory map entry 1 length
            0, 0, 0, 0,         // memory map entry 1 length
            2, 0, 0, 0,         // memory map entry 1 type
            0, 0, 0, 0,         // memory map entry 1 reserved
            0, 0, 15, 0,        // memory map entry 2 base_addr
            0, 0, 0, 0,         // memory map entry 2 base_addr
            0, 0, 1, 0,         // memory map entry 2 length
            0, 0, 0, 0,         // memory map entry 2 length
            2, 0, 0, 0,         // memory map entry 2 type
            0, 0, 0, 0,         // memory map entry 2 reserved
            0, 0, 16, 0,        // memory map entry 3 base_addr
            0, 0, 0, 0,         // memory map entry 3 base_addr
            0, 0, 238, 7,       // memory map entry 3 length
            0, 0, 0, 0,         // memory map entry 3 length
            1, 0, 0, 0,         // memory map entry 3 type
            0, 0, 0, 0,         // memory map entry 3 reserved
            0, 0, 254, 7,       // memory map entry 4 base_addr
            0, 0, 0, 0,         // memory map entry 4 base_addr
            0, 0, 2, 0,         // memory map entry 4 length
            0, 0, 0, 0,         // memory map entry 4 length
            2, 0, 0, 0,         // memory map entry 4 type
            0, 0, 0, 0,         // memory map entry 4 reserved
            0, 0, 252, 255,     // memory map entry 5 base_addr
            0, 0, 0, 0,         // memory map entry 5 base_addr
            0, 0, 4, 0,         // memory map entry 5 length
            0, 0, 0, 0,         // memory map entry 5 length
            2, 0, 0, 0,         // memory map entry 5 type
            0, 0, 0, 0,         // memory map entry 5 reserved
            9, 0, 0, 0,         // elf symbols tag type
            84, 2, 0, 0,        // elf symbols tag size
            9, 0, 0, 0,         // elf symbols num
            64, 0, 0, 0,        // elf symbols entsize
            8, 0, 0, 0,         // elf symbols shndx
            0, 0, 0, 0,         // elf symbols entry 0 name
            0, 0, 0, 0,         // elf symbols entry 0 type
            0, 0, 0, 0,         // elf symbols entry 0 flags
            0, 0, 0, 0,         // elf symbols entry 0 flags
            0, 0, 0, 0,         // elf symbols entry 0 addr
            0, 0, 0, 0,         // elf symbols entry 0 addr
            0, 0, 0, 0,         // elf symbols entry 0 offset
            0, 0, 0, 0,         // elf symbols entry 0 offset
            0, 0, 0, 0,         // elf symbols entry 0 size
            0, 0, 0, 0,         // elf symbols entry 0 size
            0, 0, 0, 0,         // elf symbols entry 0 link
            0, 0, 0, 0,         // elf symbols entry 0 info
            0, 0, 0, 0,         // elf symbols entry 0 addralign
            0, 0, 0, 0,         // elf symbols entry 0 addralign
            0, 0, 0, 0,         // elf symbols entry 0 entsize
            0, 0, 0, 0,         // elf symbols entry 0 entsize
            27, 0, 0, 0,        // elf symbols entry 1 name
            1, 0, 0, 0,         // elf symbols entry 1 type
            2, 0, 0, 0,         // elf symbols entry 1 flags
            0, 0, 0, 0,         // elf symbols entry 1 flags
            0, 0, 16, 0,        // elf symbols entry 1 addr
            0, 128, 255, 255,   // elf symbols entry 1 addr
            0, 16, 0, 0,        // elf symbols entry 1 offset
            0, 0, 0, 0,         // elf symbols entry 1 offset
            0, 48, 0, 0,        // elf symbols entry 1 size
            0, 0, 0, 0,         // elf symbols entry 1 size
            0, 0, 0, 0,         // elf symbols entry 1 link
            0, 0, 0, 0,         // elf symbols entry 1 info
            16, 0, 0, 0,        // elf symbols entry 1 addralign
            0, 0, 0, 0,         // elf symbols entry 1 addralign
            0, 0, 0, 0,         // elf symbols entry 1 entsize
            0, 0, 0, 0,         // elf symbols entry 1 entsize
            35, 0, 0, 0,        // elf symbols entry 2 name
            1, 0, 0, 0,         // elf symbols entry 2 type
            6, 0, 0, 0,         // elf symbols entry 2 flags
            0, 0, 0, 0,         // elf symbols entry 2 flags
            0, 48, 16, 0,       // elf symbols entry 2 addr
            0, 128, 255, 255,   // elf symbols entry 2 addr
            0, 64, 0, 0,        // elf symbols entry 2 offset
            0, 0, 0, 0,         // elf symbols entry 2 offset
            0, 144, 0, 0,       // elf symbols entry 2 size
            0, 0, 0, 0,         // elf symbols entry 2 size
            0, 0, 0, 0,         // elf symbols entry 2 link
            0, 0, 0, 0,         // elf symbols entry 2 info
            16, 0, 0, 0,        // elf symbols entry 2 addralign
            0, 0, 0, 0,         // elf symbols entry 2 addralign
            0, 0, 0, 0,         // elf symbols entry 2 entsize
            0, 0, 0, 0,         // elf symbols entry 2 entsize
            41, 0, 0, 0,        // elf symbols entry 3 name
            1, 0, 0, 0,         // elf symbols entry 3 type
            3, 0, 0, 0,         // elf symbols entry 3 flags
            0, 0, 0, 0,         // elf symbols entry 3 flags
            0, 192, 16, 0,      // elf symbols entry 3 addr
            0, 128, 255, 255,   // elf symbols entry 3 addr
            0, 208, 0, 0,       // elf symbols entry 3 offset
            0, 0, 0, 0,         // elf symbols entry 3 offset
            0, 32, 0, 0,        // elf symbols entry 3 size
            0, 0, 0, 0,         // elf symbols entry 3 size
            0, 0, 0, 0,         // elf symbols entry 3 link
            0, 0, 0, 0,         // elf symbols entry 3 info
            8, 0, 0, 0,         // elf symbols entry 3 addralign
            0, 0, 0, 0,         // elf symbols entry 3 addralign
            0, 0, 0, 0,         // elf symbols entry 3 entsize
            0, 0, 0, 0,         // elf symbols entry 3 entsize
            47, 0, 0, 0,        // elf symbols entry 4 name
            8, 0, 0, 0,         // elf symbols entry 4 type
            3, 0, 0, 0,         // elf symbols entry 4 flags
            0, 0, 0, 0,         // elf symbols entry 4 flags
            0, 224, 16, 0,      // elf symbols entry 4 addr
            0, 128, 255, 255,   // elf symbols entry 4 addr
            0, 240, 0, 0,       // elf symbols entry 4 offset
            0, 0, 0, 0,         // elf symbols entry 4 offset
            0, 80, 0, 0,        // elf symbols entry 4 size
            0, 0, 0, 0,         // elf symbols entry 4 size
            0, 0, 0, 0,         // elf symbols entry 4 link
            0, 0, 0, 0,         // elf symbols entry 4 info
            0, 16, 0, 0,        // elf symbols entry 4 addralign
            0, 0, 0, 0,         // elf symbols entry 4 addralign
            0, 0, 0, 0,         // elf symbols entry 4 entsize
            0, 0, 0, 0,         // elf symbols entry 4 entsize
            52, 0, 0, 0,        // elf symbols entry 5 name
            1, 0, 0, 0,         // elf symbols entry 5 type
            3, 0, 0, 0,         // elf symbols entry 5 flags
            0, 0, 0, 0,         // elf symbols entry 5 flags
            0, 48, 17, 0,       // elf symbols entry 5 addr
            0, 128, 255, 255,   // elf symbols entry 5 addr
            0, 240, 0, 0,       // elf symbols entry 5 offset
            0, 0, 0, 0,         // elf symbols entry 5 offset
            0, 0, 0, 0,         // elf symbols entry 5 size
            0, 0, 0, 0,         // elf symbols entry 5 size
            0, 0, 0, 0,         // elf symbols entry 5 link
            0, 0, 0, 0,         // elf symbols entry 5 info
            1, 0, 0, 0,         // elf symbols entry 5 addralign
            0, 0, 0, 0,         // elf symbols entry 5 addralign
            0, 0, 0, 0,         // elf symbols entry 5 entsize
            0, 0, 0, 0,         // elf symbols entry 5 entsize
            1, 0, 0, 0,         // elf symbols entry 6 name
            2, 0, 0, 0,         // elf symbols entry 6 type
            0, 0, 0, 0,         // elf symbols entry 6 flags
            0, 0, 0, 0,         // elf symbols entry 6 flags
            0, 48, 17, 0,       // elf symbols entry 6 addr
            0, 0, 0, 0,         // elf symbols entry 6 addr
            0, 240, 0, 0,       // elf symbols entry 6 offset
            0, 0, 0, 0,         // elf symbols entry 6 offset
            224, 43, 0, 0,      // elf symbols entry 6 size
            0, 0, 0, 0,         // elf symbols entry 6 size
            7, 0, 0, 0,         // elf symbols entry 6 link
            102, 1, 0, 0,       // elf symbols entry 6 info
            8, 0, 0, 0,         // elf symbols entry 6 addralign
            0, 0, 0, 0,         // elf symbols entry 6 addralign
            24, 0, 0, 0,        // elf symbols entry 6 entsize
            0, 0, 0, 0,         // elf symbols entry 6 entsize
            9, 0, 0, 0,         // elf symbols entry 7 name
            3, 0, 0, 0,         // elf symbols entry 7 type
            0, 0, 0, 0,         // elf symbols entry 7 flags
            0, 0, 0, 0,         // elf symbols entry 7 flags
            224, 91, 17, 0,     // elf symbols entry 7 addr
            0, 0, 0, 0,         // elf symbols entry 7 addr
            224, 27, 1, 0,      // elf symbols entry 7 offset
            0, 0, 0, 0,         // elf symbols entry 7 offset
            145, 55, 0, 0,      // elf symbols entry 7 size
            0, 0, 0, 0,         // elf symbols entry 7 size
            0, 0, 0, 0,         // elf symbols entry 7 link
            0, 0, 0, 0,         // elf symbols entry 7 info
            1, 0, 0, 0,         // elf symbols entry 7 addralign
            0, 0, 0, 0,         // elf symbols entry 7 addralign
            0, 0, 0, 0,         // elf symbols entry 7 entsize
            0, 0, 0, 0,         // elf symbols entry 7 entsize
            17, 0, 0, 0,        // elf symbols entry 8 name
            3, 0, 0, 0,         // elf symbols entry 8 type
            0, 0, 0, 0,         // elf symbols entry 8 flags
            0, 0, 0, 0,         // elf symbols entry 8 flags
            113, 147, 17, 0,    // elf symbols entry 8 addr
            0, 0, 0, 0,         // elf symbols entry 8 addr
            113, 83, 1, 0,      // elf symbols entry 8 offset
            0, 0, 0, 0,         // elf symbols entry 8 offset
            65, 0, 0, 0,        // elf symbols entry 8 size
            0, 0, 0, 0,         // elf symbols entry 8 size
            0, 0, 0, 0,         // elf symbols entry 8 link
            0, 0, 0, 0,         // elf symbols entry 8 info
            1, 0, 0, 0,         // elf symbols entry 8 addralign
            0, 0, 0, 0,         // elf symbols entry 8 addralign
            0, 0, 0, 0,         // elf symbols entry 8 entsize
            0, 0, 0, 0,         // elf symbols entry 8 entsize
            0, 0, 0, 0,         // elf symbols padding
            4, 0, 0, 0,         // basic memory tag type
            16, 0, 0, 0,        // basic memory tag size
            127, 2, 0, 0,       // basic memory mem_lower
            128, 251, 1, 0,     // basic memory mem_upper
            5, 0, 0, 0,         // BIOS boot device tag type
            20, 0, 0, 0,        // BIOS boot device tag size
            224, 0, 0, 0,       // BIOS boot device biosdev
            255, 255, 255, 255, // BIOS boot device partition
            255, 255, 255, 255, // BIOS boot device subpartition
            0, 0, 0, 0,         // BIOS boot device padding
            8, 0, 0, 0,         // framebuffer info tag type
            32, 0, 0, 0,        // framebuffer info tag size
            0, 128, 11, 0,      // framebuffer info framebuffer_addr
            0, 0, 0, 0,         // framebuffer info framebuffer_addr
            160, 0, 0, 0,       // framebuffer info framebuffer_pitch
            80, 0, 0, 0,        // framebuffer info framebuffer_width
            25, 0, 0, 0,        // framebuffer info framebuffer_height
            16, 2, 0, 0,        // framebuffer info framebuffer_[bpp,type], reserved, color_info
            14, 0, 0, 0,        // ACPI old tag type
            28, 0, 0, 0,        // ACPI old tag size
            82, 83, 68, 32,     // ACPI old
            80, 84, 82, 32,     // ACPI old
            89, 66, 79, 67,     // ACPI old
            72, 83, 32, 0,      // ACPI old
            220, 24, 254, 7,    // ACPI old
            0, 0, 0, 0,         // ACPI old padding
            0, 0, 0, 0,         // end tag type
            8, 0, 0, 0,         // end tag size
        ];
        let string_bytes: [u8; 65] = [
            0, 46, 115, 121,
            109, 116, 97, 98,
            0, 46, 115, 116,
            114, 116, 97, 98,
            0, 46, 115, 104,
            115, 116, 114, 116,
            97, 98, 0, 46,
            114, 111, 100, 97,
            116, 97, 0, 46,
            116, 101, 120, 116,
            0, 46, 100, 97,
            116, 97, 0, 46,
            98, 115, 115, 0,
            46, 100, 97, 116,
            97, 46, 114, 101,
            108, 46, 114, 111,
            0,
        ];
        let string_addr = string_bytes.as_ptr() as u64;
        for i in 0..8 {
            bytes[796 + i] = (string_addr >> (i * 8)) as u8;
        }
        let addr = bytes.as_ptr() as usize;
        let bi = unsafe { load(addr) };
        assert_eq!(addr, bi.start_address());
        assert_eq!(addr + bytes.len(), bi.end_address());
        assert_eq!(bytes.len(), bi.total_size() as usize);
        let es = bi.elf_sections_tag().unwrap();
        let st = es.string_table();
        assert_eq!(string_addr, st as *const _ as u64);
        let mut s = es.sections();
        let s1 = s.next().unwrap();
        assert_eq!(".rodata", st.section_name(s1));
        assert_eq!(0xFFFF_8000_0010_0000, s1.start_address());
        assert_eq!(0xFFFF_8000_0010_3000, s1.end_address());
        assert_eq!(0x0000_0000_0000_3000, s1.size());
        assert_eq!(ELF_SECTION_ALLOCATED, s1.flags());
        assert_eq!(ElfSectionType::ProgramSection, s1.section_type());
        let s2 = s.next().unwrap();
        assert_eq!(".text", st.section_name(s2));
        assert_eq!(0xFFFF_8000_0010_3000, s2.start_address());
        assert_eq!(0xFFFF_8000_0010_C000, s2.end_address());
        assert_eq!(0x0000_0000_0000_9000, s2.size());
        assert_eq!(ELF_SECTION_EXECUTABLE | ELF_SECTION_ALLOCATED, s2.flags());
        assert_eq!(ElfSectionType::ProgramSection, s2.section_type());
        let s3 = s.next().unwrap();
        assert_eq!(".data", st.section_name(s3));
        assert_eq!(0xFFFF_8000_0010_C000, s3.start_address());
        assert_eq!(0xFFFF_8000_0010_E000, s3.end_address());
        assert_eq!(0x0000_0000_0000_2000, s3.size());
        assert_eq!(ELF_SECTION_ALLOCATED | ELF_SECTION_WRITABLE, s3.flags());
        assert_eq!(ElfSectionType::ProgramSection, s3.section_type());
        let s4 = s.next().unwrap();
        assert_eq!(".bss", st.section_name(s4));
        assert_eq!(0xFFFF_8000_0010_E000, s4.start_address());
        assert_eq!(0xFFFF_8000_0011_3000, s4.end_address());
        assert_eq!(0x0000_0000_0000_5000, s4.size());
        assert_eq!(ELF_SECTION_ALLOCATED | ELF_SECTION_WRITABLE, s4.flags());
        assert_eq!(ElfSectionType::Uninitialized, s4.section_type());
        let s5 = s.next().unwrap();
        assert_eq!(".data.rel.ro", st.section_name(s5));
        assert_eq!(0xFFFF_8000_0011_3000, s5.start_address());
        assert_eq!(0xFFFF_8000_0011_3000, s5.end_address());
        assert_eq!(0x0000_0000_0000_0000, s5.size());
        assert_eq!(ELF_SECTION_ALLOCATED | ELF_SECTION_WRITABLE, s5.flags());
        assert_eq!(ElfSectionType::ProgramSection, s5.section_type());
        let s6 = s.next().unwrap();
        assert_eq!(".symtab", st.section_name(s6));
        assert_eq!(0x0000_0000_0011_3000, s6.start_address());
        assert_eq!(0x0000_0000_0011_5BE0, s6.end_address());
        assert_eq!(0x0000_0000_0000_2BE0, s6.size());
        assert_eq!(ElfSectionFlags::empty(), s6.flags());
        assert_eq!(ElfSectionType::LinkerSymbolTable, s6.section_type());
        let s7 = s.next().unwrap();
        assert_eq!(".strtab", st.section_name(s7));
        assert_eq!(0x0000_0000_0011_5BE0, s7.start_address());
        assert_eq!(0x0000_0000_0011_9371, s7.end_address());
        assert_eq!(0x0000_0000_0000_3791, s7.size());
        assert_eq!(ElfSectionFlags::empty(), s7.flags());
        assert_eq!(ElfSectionType::StringTable, s7.section_type());
        assert!(s.next().is_none());
        let mut mm = bi.memory_map_tag().unwrap().memory_areas();
        let mm1 = mm.next().unwrap();
        assert_eq!(0x00000000, mm1.start_address());
        assert_eq!(0x009_FC00, mm1.end_address());
        assert_eq!(0x009_FC00, mm1.size());
        let mm2 = mm.next().unwrap();
        assert_eq!(0x010_0000, mm2.start_address());
        assert_eq!(0x7FE_0000, mm2.end_address());
        assert_eq!(0x7EE_0000, mm2.size());
        assert!(mm.next().is_none());
        assert!(bi.module_tags().next().is_none());
        assert_eq!("GRUB 2.02~beta3-5", bi.boot_loader_name_tag().unwrap().name());
        assert_eq!("", bi.command_line_tag().unwrap().command_line());
    }
}
