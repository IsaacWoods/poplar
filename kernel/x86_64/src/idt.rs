/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use core::mem::size_of;
use core::ops::{Index,IndexMut};
use bit_field::BitField;
use gdt::SegmentSelector;

/*
 * `flags` looks like:
 *    7                           0
 *  +---+---+---+---+---+---+---+---+
 *  | P |  DPL  | 0 |    GateType   |
 *  +---+---+---+---+---+---+---+---+
 *
 *  P = Present
 *  DPL = Descriptor Privilege Level
 */
#[derive(Debug,Clone,Copy)]
#[repr(C,packed)]
pub struct IdtEntry
{
    address_0_15    : u16,
    gdt_selector    : u16,
    ist_offset      : u8 ,
    flags           : u8 ,
    address_16_31   : u16,
    address_32_63   : u32,
    reserved        : u32,
}

/*
 * XXX: Marked as diverging, as we don't 'return' from interrupts per se.
 */
pub type HandlerFunc = extern "C" fn () -> !;

impl IdtEntry
{
    pub const fn missing() -> IdtEntry
    {
        IdtEntry
        {
            address_0_15    : 0,
            gdt_selector    : 0,
            ist_offset      : 0,
            flags           : 0b1110,
            address_16_31   : 0,
            address_32_63   : 0,
            reserved        : 0,
        }
    }

    pub fn set_handler(&mut self, handler : HandlerFunc, code_selector : SegmentSelector) -> &mut Self
    {
        let mut flags : u8 = 0;
        flags.set_bits(1..4, 0b111);    // Must be 1
        flags.set_bit(7, true);         // Set Present
        self.flags = flags;

        self.gdt_selector = code_selector.table_offset();

        let address = handler as u64;
        self.address_0_15  = address as u16;
        self.address_16_31 = (address >> 16) as u16;
        self.address_32_63 = (address >> 32) as u32;

        self
    }

    pub fn set_ist_handler(&mut self, stack_offset : u8) -> &mut Self
    {
        self.ist_offset = stack_offset;
        self
    }
}

#[repr(C,packed)]
pub struct Idt
{
    entries : [IdtEntry; 256],
}

macro_rules! idt_entry
{
    ($name : ident, $entry : expr) =>
    {
        #[allow(dead_code)] pub fn $name(&mut self) -> &mut IdtEntry
        {
            &mut (self[$entry])
        }
    };
}

impl Idt
{
    pub const fn new() -> Idt
    {
        Idt
        {
            entries : [IdtEntry::missing(); 256],
        }
    }

    idt_entry!(divide_error                 , 0 );
    idt_entry!(debug_exception              , 1 );
    idt_entry!(nmi                          , 2 );
    idt_entry!(breakpoint                   , 3 );
    idt_entry!(overflow                     , 4 );
    idt_entry!(bound_range_exceeded         , 5 );
    idt_entry!(invalid_opcode               , 6 );
    idt_entry!(device_not_available         , 7 );
    idt_entry!(double_fault                 , 8 );
    idt_entry!(coprocessor_segment_overrun  , 9 );
    idt_entry!(invalid_tss                  , 10);
    idt_entry!(segment_not_present          , 11);
    idt_entry!(stack_segment_fault          , 12);
    idt_entry!(general_protection_fault     , 13);
    idt_entry!(page_fault                   , 14);
    // XXX: 15 is reserved by Intel
    idt_entry!(x87_fault                    , 16);
    idt_entry!(alignment_check              , 17);
    idt_entry!(machine_check                , 18);
    idt_entry!(simd_exception               , 19);
    idt_entry!(virtualization_exception     , 20);
    // XXX: 21 to 31 are reserved by Intel

    /*
     * This assumes that the interrupt is issued by the APIC
     */
    pub fn apic_irq(&mut self, index : u8) -> &mut IdtEntry
    {
        &mut self[::interrupts::IOAPIC_BASE + index]
    }

    pub fn load(&'static self)
    {
        #[repr(C,packed)]
        struct IdtPointer
        {
            limit   : u16,      // The maximum addressable byte of the table
            base    : u64,      // Virtual address of the start of the table
        }

        let ptr = IdtPointer
                  {
                      limit : (size_of::<Self>() - 1) as u16,
                      base  : self as *const _ as u64,
                  };

        unsafe
        {
            asm!("lidt ($0)" :: "r" (&ptr) : "memory");
        }
    }
}

impl Index<u8> for Idt
{
    type Output = IdtEntry;

    fn index(&self, index : u8) -> &IdtEntry
    {
        &self.entries[index as usize]
    }
}

impl IndexMut<u8> for Idt
{
    fn index_mut(&mut self, index : u8) -> &mut IdtEntry
    {
        &mut self.entries[index as usize]
    }
}
