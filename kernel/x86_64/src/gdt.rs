/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use core::mem::size_of;
use bit_field::BitField;
use ::tss::Tss;

#[derive(Copy,Clone,PartialEq,Eq)]
#[repr(u8)]
pub enum PrivilegeLevel
{
    Ring0 = 0,
    Ring1 = 1,
    Ring2 = 2,
    Ring3 = 3,
}

impl From<u16> for PrivilegeLevel
{
    fn from(value : u16) -> Self
    {
        match value
        {
            0 => PrivilegeLevel::Ring0,
            1 => PrivilegeLevel::Ring1,
            2 => PrivilegeLevel::Ring2,
            3 => PrivilegeLevel::Ring3,
            _ => panic!("Invalid privilege level used!"),
        }
    }
}

impl Into<u16> for PrivilegeLevel
{
    fn into(self) -> u16
    {
        match self
        {
            PrivilegeLevel::Ring0 => 0,
            PrivilegeLevel::Ring1 => 1,
            PrivilegeLevel::Ring2 => 2,
            PrivilegeLevel::Ring3 => 3,
        }
    }
}

#[derive(Clone,Copy,Debug)]
pub struct SegmentSelector(pub u16);

impl SegmentSelector
{
    pub const fn new(index : u16, rpl : PrivilegeLevel) -> SegmentSelector
    {
        SegmentSelector(index << 3 | (rpl as u16))
    }

    pub fn table_offset(&self) -> u16
    {
        (self.0 >> 3) * 0x8
    }
}

bitflags!
{
    pub struct DescriptorFlags : u64
    {
        const WRITABLE      = 1 << 41;  // Applicable only to data segments
        const CONFORMING    = 1 << 42;
        const EXECUTABLE    = 1 << 43;
        const USER_SEGMENT  = 1 << 44;  // 0 => system segment, 1 => user segment (data or code)
        const PRESENT       = 1 << 47;
        const LONG_MODE     = 1 << 53;
    }
}

struct UserSegment(u64);
struct SystemSegment(u64,u64);

#[repr(packed)]
pub struct Gdt
{
    null        : UserSegment,
    kernel_code : UserSegment,
    kernel_data : UserSegment,
    user_code   : UserSegment,
    user_data   : UserSegment,
    tss         : SystemSegment,
}

#[derive(Clone,Copy,Debug)]
pub struct GdtSelectors
{
    pub kernel_code : SegmentSelector,
    pub kernel_data : SegmentSelector,
    pub user_code   : SegmentSelector,
    pub user_data   : SegmentSelector,
    pub tss         : SegmentSelector,
}

static mut GDT : Gdt = Gdt::placeholder();

impl Gdt
{
    const fn placeholder() -> Gdt
    {
        Gdt
        {
            null        : UserSegment(0),
            kernel_code : UserSegment(0),
            kernel_data : UserSegment(0),
            user_code   : UserSegment(0),
            user_data   : UserSegment(0),
            tss         : SystemSegment(0,0),
        }
    }

    pub fn install(tss : &'static mut Tss) -> GdtSelectors
    {
        assert_first_call!("Tried to install GDT more than once!");

        unsafe
        {
            GDT.kernel_code = Gdt::create_code_segment(PrivilegeLevel::Ring0);      // 0x8
            GDT.kernel_data = Gdt::create_data_segment(PrivilegeLevel::Ring0);      // 0x10
            GDT.user_code   = Gdt::create_code_segment(PrivilegeLevel::Ring3);      // 0x18
            GDT.user_data   = Gdt::create_data_segment(PrivilegeLevel::Ring3);      // 0x20
            GDT.tss         = Gdt::create_tss_segment(tss, PrivilegeLevel::Ring0);  // 0x28
        }

        let selectors = GdtSelectors
                        {
                            kernel_code : SegmentSelector::new(1, PrivilegeLevel::Ring0),
                            kernel_data : SegmentSelector::new(2, PrivilegeLevel::Ring0),
                            user_code   : SegmentSelector::new(3, PrivilegeLevel::Ring3),
                            user_data   : SegmentSelector::new(4, PrivilegeLevel::Ring3),
                            tss         : SegmentSelector::new(5, PrivilegeLevel::Ring3),
                        };

        #[repr(C,packed)]
        struct GdtPointer
        {
            limit : u16,    // The maximum addressable byte of the table
            base  : u64,    // Virtual address of the start of the table
        }

        let ptr = GdtPointer
                  {
                      limit : (size_of::<Gdt>() - 1) as u16,
                      base  : unsafe { &GDT as *const _ as u64 },
                  };

        unsafe
        {
            // Load the GDT
            asm!("lgdt ($0)" :: "r" (&ptr) : "memory");

            // Load the new data segments
            asm!("mov ds, ax
                  mov es, ax
                  mov fs, ax
                  mov gs, ax"
                 :
                 : "rax"(u64::from(selectors.kernel_data.0))
                 : "rax"
                 : "intel", "volatile");

            // Load the new CS
            asm!("pushq $0; \
                  leaq 1f(%rip), %rax; \
                  pushq %rax; \
                  lretq; \
                  1:" :: "ri" (u64::from(selectors.kernel_code.0)) : "rax" "memory");

            // Load the task register with the TSS selector
            asm!("ltr $0" :: "r" (selectors.tss.0));
        }

        selectors
    }

    fn create_code_segment(privilege_level : PrivilegeLevel) -> UserSegment
    {
        let flags = DescriptorFlags::USER_SEGMENT   |
                    DescriptorFlags::PRESENT        |
                    DescriptorFlags::EXECUTABLE     |
                    DescriptorFlags::LONG_MODE;

        UserSegment(flags.bits() | ((privilege_level.into() : u16) as u64) << 45)
    }

    fn create_data_segment(privilege_level : PrivilegeLevel) -> UserSegment
    {
        let flags = DescriptorFlags::USER_SEGMENT   |
                    DescriptorFlags::PRESENT        |
                    DescriptorFlags::WRITABLE;

        UserSegment(flags.bits() | ((privilege_level.into() : u16) as u64) << 45)
    }

    fn create_tss_segment(tss : &'static Tss, privilege_level : PrivilegeLevel) -> SystemSegment
    {
        let ptr = (tss as *const _) as u64;
        let mut low = DescriptorFlags::PRESENT.bits();

        // Base
        low.set_bits(16..40, ptr.get_bits(0..24));
        low.set_bits(56..64, ptr.get_bits(24..32));

        // Limit (which is inclusive so 1 less than size)
        low.set_bits(0..16, (size_of::<Tss>() - 1) as u64);

        // Type (0b1001 = available 64-bit TSS)
        low.set_bits(40..44, 0b1001);

        let mut high = 0;
        high.set_bits(0..32, ptr.get_bits(32..64));

        SystemSegment(low, high)
    }
}
