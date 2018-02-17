/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use core::ptr;
use spin::Mutex;
use ::memory::{Frame,MemoryController,FrameAllocator};
use ::memory::map::{LOCAL_APIC_REGISTER_SPACE,IOAPIC_REGISTER_SPACE};
use ::memory::paging::{PhysicalAddress,Page,EntryFlags};

pub static LOCAL_APIC   : Mutex<LocalApic> = Mutex::new(LocalApic::placeholder());
pub static IO_APIC      : Mutex<IoApic>    = Mutex::new(IoApic::placeholder());

#[derive(Clone,Copy,Debug)]
pub struct LocalApic
{
    is_enabled      : bool,
    register_base   : PhysicalAddress,
}

impl LocalApic
{
    /*
     * This creates a placeholder LocalApic so we can initialise the Mutex statically.
     * XXX: This does not actually initialise the APIC
     */
    const fn placeholder() -> LocalApic
    {
        LocalApic
        {
            is_enabled      : false,
            register_base   : PhysicalAddress::new(0),
        }
    }

    pub unsafe fn get_register_ptr(&self, offset : usize) -> *mut u32
    {
        LOCAL_APIC_REGISTER_SPACE.offset(offset as isize).mut_ptr() as *mut u32
    }

    pub unsafe fn enable<A>(&self,
                            register_base       : PhysicalAddress,
                            memory_controller   : &mut MemoryController<A>)
        where A : FrameAllocator
    {
        assert!(!self.is_enabled);

        // Map the configuration space into virtual memory
        assert!(register_base.is_frame_aligned(), "Expected local APIC registers to be frame aligned");
        memory_controller.active_table.map_to(Page::get_containing_page(LOCAL_APIC_REGISTER_SPACE),
                                              Frame::get_containing_frame(register_base),
                                              EntryFlags::WRITABLE,
                                              &mut memory_controller.frame_allocator);

        /*
         * Enable the APIC by setting bit 8 of the Spurious Interrupt Vector Register. Also set the
         * number of the spurious interrupt to 0xFF.
         */
        ptr::write(self.get_register_ptr(0xF0), (1<<8) | 0xFF);
    }

    pub fn register_interrupt_source_override(&self, bus : u8, irq : u8, global_interrupt : u32)
    {
        // TODO
    }

    pub fn enable_timer(&self, frequency : u64)
    {
        // TODO: use the PIT or something to actually calculate the frequency the APIC is running at
        unsafe
        {
            ptr::write(self.get_register_ptr(0x320), 32 | 0x20000); // LVT = IRQ0, periodic mode
            ptr::write(self.get_register_ptr(0x3E0), 0x3);          // Set the timer divisor = 16
            ptr::write(self.get_register_ptr(0x380), 100000000);    // Set initial count
        }
    }

    pub fn send_eoi(&self)
    {
        /*
         * To send an EOI, we write 0 to the register with offset 0xB0. Writing any other value
         * will cause a #GP.
         */
        unsafe { ptr::write(self.get_register_ptr(0xB0), 0) };
    }
}

#[derive(Clone,Copy,Debug)]
pub struct IoApic
{
    is_enabled      : bool,
    register_base   : PhysicalAddress,
}

impl IoApic
{
    const fn placeholder() -> IoApic
    {
        IoApic
        {
            is_enabled      : false,
            register_base   : PhysicalAddress::new(0),
        }
    }

    pub unsafe fn enable<A>(&self,
                            register_base      : PhysicalAddress,
                            memory_controller  : &mut MemoryController<A>)
        where A : FrameAllocator
    {
        assert!(!self.is_enabled);

        // Map the configuration space to virtual memory
        assert!(register_base.is_frame_aligned(), "Expected IOAPIC registers to be frame aligned");
        memory_controller.active_table.map_to(Page::get_containing_page(IOAPIC_REGISTER_SPACE),
                                              Frame::get_containing_frame(register_base),
                                              EntryFlags::WRITABLE,
                                              &mut memory_controller.frame_allocator);
    }
}
