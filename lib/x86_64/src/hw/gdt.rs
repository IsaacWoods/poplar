use super::{tss::Tss, DescriptorTablePointer};
use crate::memory::VirtualAddress;
use bit_field::BitField;
use core::{mem, ops::Deref, pin::Pin};

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PrivilegeLevel {
    Ring0 = 0,
    Ring1 = 1,
    Ring2 = 2,
    Ring3 = 3,
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

const ACCESSED: u64 = 1 << 40;
const READABLE: u64 = 1 << 41;
const WRITABLE: u64 = 1 << 41;
const USER_SEGMENT: u64 = 1 << 44;
const PRESENT: u64 = 1 << 47;
const LONG_MODE: u64 = 1 << 53;

#[derive(Debug)]
pub struct CodeSegment(u64);

impl CodeSegment {
    pub const fn new(ring: PrivilegeLevel) -> CodeSegment {
        /*
         * XXX: the Accessed and Readable bits of 64-bit code segments should be ignored, but my
         * old-ish AMD #GPs if they're not set ¯\_(ツ)_/¯
         */
        CodeSegment(
            ACCESSED + READABLE + (1 << 43) + USER_SEGMENT + PRESENT + LONG_MODE + ((ring as u64) << 45),
        )
    }
}

#[derive(Debug)]
pub struct DataSegment(u64);

impl DataSegment {
    pub const fn new(ring: PrivilegeLevel) -> DataSegment {
        DataSegment(ACCESSED + WRITABLE + PRESENT + USER_SEGMENT + ((ring as u64) << 45))
    }
}

#[derive(Clone, Copy)]
pub struct TssSegment(u64, u64);

impl TssSegment {
    pub const fn empty() -> TssSegment {
        TssSegment(0, 0)
    }

    pub fn new(tss: Pin<&Tss>) -> TssSegment {
        // Get the address of the *underlying TSS*
        let tss_address = (tss.deref() as *const _) as u64;
        let mut low = PRESENT;
        let mut high = 0;

        // Base address
        low.set_bits(16..40, tss_address.get_bits(0..24));
        low.set_bits(56..64, tss_address.get_bits(24..32));
        high.set_bits(0..32, tss_address.get_bits(32..64));

        // Limit (`size_of::<Tss>() - 1` because `base + limit` is inclusive)
        low.set_bits(0..16, (mem::size_of::<Tss>() - 1) as u64);

        // Type (0b1001 = available 64-bit TSS)
        low.set_bits(40..44, 0b1001);

        TssSegment(low, high)
    }
}

pub const KERNEL_CODE_SELECTOR: SegmentSelector = SegmentSelector::new(1, PrivilegeLevel::Ring0);
pub const KERNEL_DATA_SELECTOR: SegmentSelector = SegmentSelector::new(2, PrivilegeLevel::Ring0);
pub const USER_COMPAT_CODE_SELECTOR: SegmentSelector = SegmentSelector::new(3, PrivilegeLevel::Ring3);
pub const USER_DATA_SELECTOR: SegmentSelector = SegmentSelector::new(4, PrivilegeLevel::Ring3);
pub const USER_CODE64_SELECTOR: SegmentSelector = SegmentSelector::new(5, PrivilegeLevel::Ring3);

// NOTE: these have to account for the null segment
pub const NUM_STATIC_ENTRIES: usize = 7;
pub const OFFSET_TO_FIRST_TSS: usize = 0x30;
pub const MAX_CPUS: usize = 8;

/// A GDT suitable for the kernel to use. The order of the segments is important: `sysret` relies
/// on the Ring-3 segments going in the order "32-bit Code Segment", "Data Segment", "64-bit Code
/// Segment".
#[repr(C, packed)]
pub struct Gdt {
    null: u64,
    kernel_code: CodeSegment,
    kernel_data: DataSegment,

    /// This is a placeholder segment for returning to Ring 3 in Compatability Mode. We don't
    /// support this so this is just set to a null segment.
    user_compat_code: u64,
    user_data: DataSegment,
    user_code64: CodeSegment,

    tsss: [TssSegment; MAX_CPUS],

    /// This field is not part of the actual GDT; we just use it to keep track of how many TSS
    /// entries have been used
    next_free_tss: usize,
}

impl Gdt {
    /// Create a `Gdt` with pre-populated code and data segments, and `MAX_CPUS` empty TSSs. The
    /// kernel should populate a TSS for each processor it plans to bring up, then call the
    /// `load` method to load the new GDT and switch to the new kernel code and data segments.
    pub const fn new() -> Gdt {
        Gdt {
            null: 0,
            kernel_code: CodeSegment::new(PrivilegeLevel::Ring0),
            kernel_data: DataSegment::new(PrivilegeLevel::Ring0),
            user_compat_code: 0,
            user_data: DataSegment::new(PrivilegeLevel::Ring3),
            user_code64: CodeSegment::new(PrivilegeLevel::Ring3),
            tsss: [TssSegment::empty(); MAX_CPUS],
            next_free_tss: 0,
        }
    }

    /// Add a new TSS, if there's space for it. The first TSS added **must** be for the bootstrap
    /// processor (the one that should be touching the GDT), then subsequent TSSs for the
    /// application processors may be added.
    ///
    /// ### Panics
    /// Panics if we have already added as many TSSs as this GDT can hold.
    pub fn add_tss(&mut self, tss: TssSegment) -> SegmentSelector {
        if self.next_free_tss == MAX_CPUS {
            panic!("Not enough space in the GDT for the number of TSSs we need!");
        }

        let offset = OFFSET_TO_FIRST_TSS + self.next_free_tss * mem::size_of::<TssSegment>();
        self.tsss[self.next_free_tss] = tss;
        self.next_free_tss += 1;

        SegmentSelector(offset as u16)
    }

    /// Load the new GDT, switch to the new `kernel_code` code segment, clear DS, ES, FS, GS, and
    /// SS to the null segment, and switch TR to the first TSS.
    // TODO: we should probably take a Pin or something to ensure it doesn't move
    pub unsafe fn load(&self, tss_selector: SegmentSelector) {
        if self.next_free_tss == 0 {
            panic!("Tried to load kernel GDT before adding bootstrap TSS!");
        }

        let gdt_ptr = DescriptorTablePointer {
            limit: (NUM_STATIC_ENTRIES * mem::size_of::<u64>() + MAX_CPUS * mem::size_of::<TssSegment>() - 1)
                as u16,
            base: VirtualAddress::new(self as *const _ as usize).unwrap(),
        };

        asm!("// Load the new GDT
              lgdt [$0]
             
              // Load the new kernel data segment
              mov ds, ax
              mov es, ax
              mov fs, ax
              mov gs, ax
              mov ss, ax
              
              // Switch to the new code segment
              push rbx
              lea rax, [rip+0x3]
              push rax
              retfq
              1:
              
              // Load the TSS
              ltr cx"
        :
        : "r"(&gdt_ptr),
          "{rax}"(KERNEL_DATA_SELECTOR),
          "{rbx}"(KERNEL_CODE_SELECTOR),
          "{rcx}"(tss_selector)
        : "rax"
        : "intel"
        );
    }
}
