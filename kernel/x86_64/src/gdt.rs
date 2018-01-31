/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use super::PrivilegeLevel;
use super::tss::Tss;
use bit_field::BitField;
use core::mem::size_of;

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
        const CONFORMING    = 1 << 42;
        const EXECUTABLE    = 1 << 43;
        const USER_SEGMENT  = 1 << 44;
        const PRESENT       = 1 << 47;
        const LONG_MODE     = 1 << 53;
    }
}

#[derive(Debug)]
pub enum GdtDescriptor
{
    UserSegment(u64),
    SystemSegment(u64,u64)
}

impl GdtDescriptor
{
    pub fn create_tss_segment(tss : &'static Tss) -> GdtDescriptor
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

        GdtDescriptor::SystemSegment(low, high)
    }
}

pub struct Gdt
{
    table     : [u64; 8],
    next_free : usize
}

impl Gdt
{
    pub fn new() -> Gdt
    {
        Gdt
        {
            table : [0; 8],
            next_free : 1       // NOTE: The 0th entry must always be the null selector
        }
    }

    pub fn add_entry(&mut self, entry : GdtDescriptor) -> SegmentSelector
    {
        let index = match entry
                    {
                        GdtDescriptor::UserSegment(value) => self.push(value),

                        GdtDescriptor::SystemSegment(low,high) =>
                        {
                            let index = self.push(low);
                            self.push(high);
                            index
                        }
                    };

        SegmentSelector::new(index as u16, PrivilegeLevel::Ring0)
    }

    fn push(&mut self, value : u64) -> usize
    {
        if self.next_free < self.table.len()
        {
            let index = self.next_free;
            self.table[index] = value;
            self.next_free += 1;
            index
        }
        else
        {
            panic!("Run out of GDT entries");
        }
    }

    pub fn load(&'static self, code_selector : SegmentSelector,
                               tss_selector  : SegmentSelector)
    {
        #[repr(C,packed)]
        struct GdtPointer
        {
            limit : u16,    // The maximum addressable byte of the table
            base  : u64,    // Virtual address of the start of the table
        }

        let ptr = GdtPointer
                  {
                      limit : (self.table.len() * size_of::<u64>() - 1) as u16,
                      base  : self.table.as_ptr() as u64,
                  };

        unsafe
        {
            // Load the GDT
            asm!("lgdt ($0)" :: "r" (&ptr) : "memory");

            // Load the new CS
            asm!("pushq $0; \
                  leaq 1f(%rip), %rax; \
                  pushq %rax; \
                  lretq; \
                  1:" :: "ri" (u64::from(code_selector.0)) : "rax" "memory");

            // Load the task register with the TSS selector
            asm!("ltr $0" :: "r" (tss_selector.0));
        }
    }
}
