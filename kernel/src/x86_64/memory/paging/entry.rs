/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use x86_64::memory::Frame;
use multiboot2::ElfSection;

pub struct Entry(u64);

bitflags!
{
    pub struct EntryFlags : u64
    {
        const PRESENT           = 1<<0;
        const WRITABLE          = 1<<1;
        const USER_ACCESSIBLE   = 1<<2;
        const WRITE_THROUGH     = 1<<3;
        const NO_CACHE          = 1<<4;
        const ACCESSED          = 1<<5;
        const DIRTY             = 1<<6;
        const HUGE_PAGE         = 1<<7;
        const GLOBAL            = 1<<8;
        const NO_EXECUTE        = 1<<63;
    }
}

impl EntryFlags
{
    pub fn from_elf_section(section : &ElfSection) -> EntryFlags
    {
        use multiboot2::{ELF_SECTION_ALLOCATED,ELF_SECTION_WRITABLE,ELF_SECTION_EXECUTABLE};
        let mut flags = EntryFlags::empty();

        if  section.flags().contains(ELF_SECTION_ALLOCATED)  { flags |= EntryFlags::PRESENT;    }
        if  section.flags().contains(ELF_SECTION_WRITABLE)   { flags |= EntryFlags::WRITABLE;   }
        if !section.flags().contains(ELF_SECTION_EXECUTABLE) { flags |= EntryFlags::NO_EXECUTE; }

        flags
    }
}

impl Entry
{
    pub fn is_unused(&self) -> bool
    {
        self.0 == 0
    }

    pub fn set_unused(&mut self)
    {
        self.0 = 0;
    }

    pub fn flags(&self) -> EntryFlags
    {
        EntryFlags::from_bits_truncate(self.0)
    }

    pub fn get_pointed_frame(&self) -> Option<Frame>
    {
        if self.flags().contains(EntryFlags::PRESENT)
        {
            Some(Frame::get_containing_frame((self.0 as usize & 0x000fffff_fffff000).into()))
        }
        else
        {
            None
        }
    }

    pub fn set(&mut self, frame : Frame, flags : EntryFlags)
    {
        assert!(usize::from(frame.get_start_address()) & !0x000fffff_fffff000 == 0);
        self.0 = (usize::from(frame.get_start_address()) as u64) | flags.bits();
    }
}
