use memory::Frame;
use multiboot2::ElfSectionFlags;

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

impl Default for EntryFlags {
    fn default() -> EntryFlags {
        EntryFlags::PRESENT
    }
}

impl EntryFlags {
    pub fn from_elf_section(section: &::multiboot2::ElfSection) -> EntryFlags {
        let mut flags = EntryFlags::empty();

        if section.flags().contains(ElfSectionFlags::ALLOCATED) {
            flags |= EntryFlags::PRESENT;
        }
        if section.flags().contains(ElfSectionFlags::WRITABLE) {
            flags |= EntryFlags::WRITABLE;
        }
        if !section.flags().contains(ElfSectionFlags::EXECUTABLE) {
            flags |= EntryFlags::NO_EXECUTE;
        }

        flags
    }

    /*
     * True if the given set of flags is compatible with `self`. False if not compatible, or would
     * create potential security vulnerability, such as:
     *      * Allows execution as code when `self` prevents it
     *      * Is accessible in user-mode when `self` is not
     *      * Can be written to when `self` can't be
     *  TODO: maybe actually think about whether they can be different but still compatible
     */
    pub fn is_compatible(&self, other: EntryFlags) -> bool {
        *self == other
    }
}

impl Entry {
    pub fn is_unused(&self) -> bool {
        self.0 == 0
    }

    pub fn set_unused(&mut self) {
        self.0 = 0;
    }

    pub fn flags(&self) -> EntryFlags {
        EntryFlags::from_bits_truncate(self.0)
    }

    pub fn pointed_frame(&self) -> Option<Frame> {
        if self.flags().contains(EntryFlags::PRESENT) {
            const ADDRESS_MASK: usize = 0x000f_ffff_ffff_f000;
            Some(Frame::containing_frame(
                (self.0 as usize & ADDRESS_MASK).into(),
            ))
        } else {
            None
        }
    }

    pub fn set(&mut self, frame: Frame, flags: EntryFlags) {
        self.0 = (usize::from(frame.start_address()) as u64) | flags.bits();
    }
}
