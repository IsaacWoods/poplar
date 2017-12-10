/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use memory::VirtualAddress;
use core::mem::size_of;
use core::ops::{Index,IndexMut};
use bit_field::BitField;

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
    pub fn missing() -> IdtEntry
    {
        let mut flags : u8 = 0;
        flags.set_bits(1..4, 0b111);    // Must be 1

        IdtEntry
        {
            address_0_15    : 0,
            gdt_selector    : 0,
            ist_offset      : 0,
            flags           : flags,
            address_16_31   : 0,
            address_32_63   : 0,
            reserved        : 0,
        }
    }

    pub fn set_handler(&mut self, handler : HandlerFunc)
    {
        // TODO: don't hardcode code selector - get from new GDT
        const KERNEL_CODE_SELECTOR : u16 = 0x8;
        self.gdt_selector = KERNEL_CODE_SELECTOR;

        let mut flags : u8 = 0;
        flags.set_bits(1..4, 0b111);    // Must be 1
        flags.set_bit(7, true);         // Set Present
        self.flags = flags;

        let address = handler as u64;
        self.address_0_15  = address as u16;
        self.address_16_31 = (address >> 16) as u16;
        self.address_32_63 = (address >> 32) as u32;
    }
}

#[repr(C,packed)]
pub struct Idt
{
    entries : [IdtEntry; 256],
}

impl Idt
{
    pub fn new() -> Idt
    {
        Idt
        {
            entries : [IdtEntry::missing(); 256],
        }
    }

    pub fn divide_error                 (&mut self) -> &mut IdtEntry { &mut (self[ 0]) }
    pub fn debug_exception              (&mut self) -> &mut IdtEntry { &mut (self[ 1]) }
    pub fn nmi                          (&mut self) -> &mut IdtEntry { &mut (self[ 2]) }
    pub fn breakpoint                   (&mut self) -> &mut IdtEntry { &mut (self[ 3]) }
    pub fn overflow                     (&mut self) -> &mut IdtEntry { &mut (self[ 4]) }
    pub fn bound_range_exceeded         (&mut self) -> &mut IdtEntry { &mut (self[ 5]) }
    pub fn invalid_opcode               (&mut self) -> &mut IdtEntry { &mut (self[ 6]) }
    pub fn device_not_available         (&mut self) -> &mut IdtEntry { &mut (self[ 7]) }
    pub fn double_fault                 (&mut self) -> &mut IdtEntry { &mut (self[ 8]) }
    pub fn coprocessor_segment_overrun  (&mut self) -> &mut IdtEntry { &mut (self[ 9]) }
    pub fn invalid_tss                  (&mut self) -> &mut IdtEntry { &mut (self[10]) }
    pub fn segment_not_present          (&mut self) -> &mut IdtEntry { &mut (self[11]) }
    pub fn stack_segment_fault          (&mut self) -> &mut IdtEntry { &mut (self[12]) }
    pub fn general_proctection_fault    (&mut self) -> &mut IdtEntry { &mut (self[13]) }
    pub fn page_fault                   (&mut self) -> &mut IdtEntry { &mut (self[14]) }
    // XXX: 15 - Intel reserved
    pub fn x87_fault                    (&mut self) -> &mut IdtEntry { &mut (self[16]) }
    pub fn alignment_check              (&mut self) -> &mut IdtEntry { &mut (self[17]) }
    pub fn machine_check                (&mut self) -> &mut IdtEntry { &mut (self[18]) }
    pub fn simd_exception               (&mut self) -> &mut IdtEntry { &mut (self[19]) }
    pub fn virtualization_exception     (&mut self) -> &mut IdtEntry { &mut (self[20]) }
    // XXX: 21 to 31 - Intel reserved
    
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

impl Index<usize> for Idt
{
    type Output = IdtEntry;

    fn index(&self, index : usize) -> &IdtEntry
    {
        &self.entries[index]
    }
}

impl IndexMut<usize> for Idt
{
    fn index_mut(&mut self, index : usize) -> &mut IdtEntry
    {
        &mut self.entries[index]
    }
}
