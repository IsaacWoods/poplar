use super::tss::Tss;
use crate::memory::VirtualAddress;
use bit_field::BitField;
use bitflags::bitflags;
use core::mem;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PrivilegeLevel {
    Ring0 = 0,
    Ring1 = 1,
    Ring2 = 2,
    Ring3 = 3,
}

impl From<u8> for PrivilegeLevel {
    fn from(value: u8) -> Self {
        match value {
            0 => PrivilegeLevel::Ring0,
            1 => PrivilegeLevel::Ring1,
            2 => PrivilegeLevel::Ring2,
            3 => PrivilegeLevel::Ring3,
            _ => panic!("Invalid privilege level used!"),
        }
    }
}

impl Into<u8> for PrivilegeLevel {
    fn into(self) -> u8 {
        match self {
            PrivilegeLevel::Ring0 => 0,
            PrivilegeLevel::Ring1 => 1,
            PrivilegeLevel::Ring2 => 2,
            PrivilegeLevel::Ring3 => 3,
        }
    }
}

/// An index into the GDT, specifying a particular segment. These are loaded into the segment
/// registers to reference segments.
#[derive(Clone, Copy, Debug)]
pub struct SegmentSelector(pub u16);

impl SegmentSelector {
    pub const fn new(index: u16, rpl: PrivilegeLevel) -> SegmentSelector {
        SegmentSelector(index << 3 | (rpl as u16))
    }

    pub const fn table_offset(&self) -> u16 {
        (self.0 >> 3) * 0x8
    }
}

bitflags! {
    pub struct DescriptorFlags : u64
    {
        /// Applicable only to data segments
        const WRITABLE      = 1 << 41;
        const CONFORMING    = 1 << 42;
        const EXECUTABLE    = 1 << 43;
        /// 0 => system segment, 1 => user segment
        const USER_SEGMENT  = 1 << 44;
        const PRESENT       = 1 << 47;
        const LONG_MODE     = 1 << 53;
    }
}

/// Describes a GDT segment. TODO
pub enum Segment {
    User(u64),
    System(u64, u64),
}

impl Segment {
    pub fn new_code_segment(ring: PrivilegeLevel) -> Segment {
        let flags = DescriptorFlags::USER_SEGMENT
            | DescriptorFlags::PRESENT
            | DescriptorFlags::EXECUTABLE
            | DescriptorFlags::LONG_MODE;

        Segment::User(flags.bits() | u64::from(ring.into(): u8) << 45)
    }

    pub fn new_data_segment(ring: PrivilegeLevel) -> Segment {
        let flags =
            DescriptorFlags::USER_SEGMENT | DescriptorFlags::PRESENT | DescriptorFlags::LONG_MODE;

        Segment::User(flags.bits() | u64::from(ring.into(): u8) << 45)
    }

    pub fn new_tss_segment(tss: &'static Tss) -> Segment {
        let tss_address = (tss as *const _) as u64;
        let mut low = DescriptorFlags::PRESENT.bits();
        let mut high = 0;

        // Base address
        low.set_bits(16..40, tss_address.get_bits(0..24));
        low.set_bits(56..64, tss_address.get_bits(24..32));
        high.set_bits(0..32, tss_address.get_bits(32..64));

        // Limit (`size_of::<Tss>() - 1` because `base + limit` is inclusive)
        low.set_bits(0..16, (mem::size_of::<Tss>() - 1) as u64);

        // Type (0b1001 = available 64-bit TSS)
        low.set_bits(40..44, 0b1001);

        Segment::System(low, high)
    }
}

pub const GDT_MAX_ENTRIES: usize = 16;

/// A GDT that can be used in 64-bit mode. The structure of the GDT in Long Mode differs from that
/// in x86 or 32-bit mode on x86_64.
///
/// This structure is created with a static number of entries, which can be dynamically added.
/// Adding more than the maximum number will cause the kernel to panic. Note that the number of
/// entries does not necessarily correspond to the number of segments described by the table;
/// `SystemSegment`s will take two entries, and the first entry will always be the null segment.
///
/// The GDT must be loaded with the `load` method. After loading, adding more entries will not have
/// effect, as the limit will be set to the end of the table upon loading, and so `load` must be
/// called again.
#[repr(C, packed)]
pub struct Gdt {
    table: [u64; GDT_MAX_ENTRIES],
    next_free: usize,
}

impl Gdt {
    /// Create an empty GDT, containing zero entries.
    pub const fn empty() -> Gdt {
        Gdt {
            table: [0; GDT_MAX_ENTRIES],

            /// The first segment of the GDT must always be the null segment. Therefore, we start at
            /// index `1` in the table.
            next_free: 1,
        }
    }

    pub fn add_segment(&mut self, segment: Segment) -> SegmentSelector {
        match segment {
            Segment::User(entry) => {
                // Make sure the segment will fit
                if (self.next_free + 1) > GDT_MAX_ENTRIES {
                    panic!("Tried to add an entry to the GDT, but it's full!");
                }

                let index = self.next_free;
                self.table[self.next_free] = entry;
                self.next_free += 1;
                SegmentSelector::new(index as u16, PrivilegeLevel::Ring0)
            }

            Segment::System(low, high) => {
                // Make sure the segment will fit
                if (self.next_free + 2) > GDT_MAX_ENTRIES {
                    panic!("Tried to add an entry to the GDT, but it's full!");
                }

                let index = self.next_free;
                self.table[self.next_free] = low;
                self.table[self.next_free + 1] = high;
                SegmentSelector::new(index as u16, PrivilegeLevel::Ring0)
            }
        }
    }

    /// TODO
    pub unsafe fn load(
        &'static self,
        code_selector: SegmentSelector,
        data_selector: SegmentSelector,
        tss_selector: SegmentSelector,
    ) {
        #[repr(C, packed)]
        pub struct GdtPointer {
            /// `(base + limit)` is the maximum addressable byte of the GDT (so this is not the size)
            limit: u16,

            /// Virtual address of the start of the GDT
            base: VirtualAddress,
        }

        let gdt_ptr = GdtPointer {
            limit: ((self.next_free - 1) * mem::size_of::<u64>() - 1) as u16,
            base: VirtualAddress::new(self.table.as_ptr() as usize).unwrap(),
        };

        // TODO: rewrite as one big asm!

        // Load the GDT
        asm!("lgdt [$0]"
             :
             : "r"(&gdt_ptr)
             : "rax", "memory"
             : "intel", "volatile");

        // Load the new data segments
        asm!("mov ds, ax
              mov es, ax
              mov fs, ax
              mov gs, ax"
             :
             : "rax"(data_selector.0)
             : "rax"
             : "intel", "volatile");

        // Load the new CS
        asm!("push $0
              lea rax, [rip+0x3]
              push rax
              retfq
              1:"
             :
             : "r"(code_selector.0)
             : "rax", "memory"
             : "intel", "volatile");

        // Load the task register with the TSS selector
        asm!("ltr $0"
             :
             : "r" (tss_selector.0)
             :
             : "intel", "volatile");
    }
}
