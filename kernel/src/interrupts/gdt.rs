/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use x86_64::PrivilegeLevel;
use x86_64::structures::gdt::SegmentSelector;
use x86_64::structures::tss::TaskStateSegment;
use bit_field::BitField;
use core::mem::size_of;

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

pub enum GdtDescriptor
{
    UserSegment(u64),
    SystemSegment(u64,u64)
}

impl GdtDescriptor
{
    pub fn create_tss_segment(tss : &'static TaskStateSegment) -> GdtDescriptor
    {
        let ptr = (tss as *const _) as u64;
        let mut low = DescriptorFlags::PRESENT.bits();

        // Base
        low.set_bits(16..40, ptr.get_bits(0..24));
        low.set_bits(56..64, ptr.get_bits(24..32));

        // Limit (which is inclusive so 1 less than size)
        low.set_bits(0..16, (size_of::<TaskStateSegment>() - 1) as u64);

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

    pub fn load(&'static self)
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
            asm!("lgdt ($0)" :: "r" (&ptr) : "memory");
        }
    }
}
