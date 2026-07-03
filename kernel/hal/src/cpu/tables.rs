use crate::mem::VAddr;
use bnb::BitOps;
use core::mem;

#[repr(C, packed)]
pub struct DescriptorTablePtr {
    pub limit: u16,
    pub base: u64,
}

bitfield::bitfield! {
    /// Describes a code or data segment within the GDT.
    pub struct GdtDescriptor<u64> {
        const LIMIT_LO = 16;
        const BASE_LO = 16;
        const BASE_MI = 8;
        /// Set by the CPU when the segment is accessed. Best left set, as will cause a #PF if GDT
        /// is not mapped as writable.
        const ACCESSED: bool;
        /// For code segments, whether the segment is readable. For data segments, whether the
        /// segment is writable.
        const READABLE_WRITABLE: bool;
        /// For code segments, whether segment is conforming (can be executed from a lower privilege
        /// level). For data segments, marks the direction (whether the segment grows up/down).
        /// For system segments, denotes a TSS selector.
        const DIRECTION_CONFORMING: bool;
        /// When clear, the segment is a data segment. When set, it is a code segment.
        const EXECUTABLE: bool;
        /// When clear, the segment is a system segment. When set, it is a data/code segment.
        const IS_CODE_OR_DATA: bool;
        const DPL = 2;
        /// Marks the descriptor as a valid segment.
        const PRESENT: bool;
        const LIMIT_HI = 4;
        const _RESERVED0: bool;
        const LONG_MODE: bool;
        /// When set, marks the segment as a 32-bit segment. When clear, 16-bit.
        const BIG: bool;
        /// Indicates the scale of the segment's limit. When set, uses a 4KiB granularity. When
        /// clear, uses byte granularity.
        const GRANULARITY: bool;
        const BASE_HI = 8;
    }
}

impl GdtDescriptor {
    const BASE_CODE_OR_DATA: GdtDescriptor = GdtDescriptor::new()
        .with(GdtDescriptor::LIMIT_LO, 0xffff)
        .with(GdtDescriptor::ACCESSED, true)
        .with(GdtDescriptor::IS_CODE_OR_DATA, true)
        .with(GdtDescriptor::PRESENT, true)
        .with(GdtDescriptor::LIMIT_HI, 0b1111)
        .with(GdtDescriptor::GRANULARITY, true);

    pub const KERNEL_DATA64: GdtDescriptor = Self::BASE_CODE_OR_DATA
        .with(GdtDescriptor::READABLE_WRITABLE, true)
        .with(GdtDescriptor::BIG, true);

    pub const KERNEL_CODE64: GdtDescriptor = Self::BASE_CODE_OR_DATA
        .with(GdtDescriptor::READABLE_WRITABLE, true)
        .with(GdtDescriptor::EXECUTABLE, true)
        .with(GdtDescriptor::LONG_MODE, true);

    pub const USER_CODE32: GdtDescriptor = Self::BASE_CODE_OR_DATA
        .with(GdtDescriptor::READABLE_WRITABLE, true)
        .with(GdtDescriptor::EXECUTABLE, true)
        .with(GdtDescriptor::BIG, true)
        .with(GdtDescriptor::DPL, 3);

    pub const USER_CODE64: GdtDescriptor = Self::BASE_CODE_OR_DATA
        .with(GdtDescriptor::READABLE_WRITABLE, true)
        .with(GdtDescriptor::EXECUTABLE, true)
        .with(GdtDescriptor::DPL, 3)
        .with(GdtDescriptor::LONG_MODE, true);

    pub const USER_DATA64: GdtDescriptor = Self::BASE_CODE_OR_DATA
        .with(GdtDescriptor::READABLE_WRITABLE, true)
        .with(GdtDescriptor::DPL, 3);

    /// Construct a `GdtDescriptor` that can be used as the *lower* entry for a TSS selector. This
    /// should be followed by another entry of the format:
    /// ```ignore
    ///    31                                       0
    ///     ----------------------------------------
    ///     |               Reserved               |  12
    ///     ----------------------------------------
    ///     |         Address of TSS 63:32         |  8
    ///     ----------------------------------------
    ///     |           Returned entry             |  4
    ///     ----------------------------------------
    ///     |           Returned entry             |  0
    ///     ----------------------------------------
    /// ```
    pub const fn new_tss_lo(address: VAddr) -> GdtDescriptor {
        let address = usize::from(address);
        GdtDescriptor::new()
            .with(GdtDescriptor::LIMIT_LO, mem::size_of::<Tss>() - 1)
            .with(GdtDescriptor::BASE_LO, address.bits(0..16))
            .with(GdtDescriptor::BASE_MI, address.bits(16..24))
            .with(GdtDescriptor::ACCESSED, true)
            .with(GdtDescriptor::EXECUTABLE, true)
            .with(GdtDescriptor::DPL, 0)
            .with(GdtDescriptor::PRESENT, true)
            .with(GdtDescriptor::BASE_HI, address.bits(24..32))
    }
}

/// The TSS continues to exist on x86_64, despite a lack of support for hardware task switching. It
/// is used to hold some architectural information needed for various things.
#[repr(C, packed(4))]
pub struct Tss {
    _reserved0: u32,
    pub privilege_stack_table: [VAddr; 3],
    _reserved1: u32,
    _reserved2: u32,
    pub interrupt_stack_table: [VAddr; 7],
    _reserved3: u32,
    _reserved4: u32,
    _reserved5: u16,
    pub io_map_base: u16,
}

impl Tss {
    pub const fn new() -> Tss {
        Tss {
            _reserved0: 0,
            privilege_stack_table: [VAddr::new(0x0); 3],
            _reserved1: 0,
            _reserved2: 0,
            interrupt_stack_table: [VAddr::new(0x0); 7],
            _reserved3: 0,
            _reserved4: 0,
            _reserved5: 0,
            io_map_base: 0,
        }
    }
}
