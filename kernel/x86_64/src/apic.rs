/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use core::ptr;
use ::memory::{Frame,MemoryController,FrameAllocator};
use ::memory::map::{LOCAL_APIC_REGISTER_SPACE,IOAPIC_REGISTER_SPACE};
use ::memory::paging::{PhysicalAddress,Page,EntryFlags};

#[derive(Clone,Copy,Debug)]
pub struct LocalApic
{
    register_base   : PhysicalAddress,
}

impl LocalApic
{
    pub unsafe fn new<A>(register_base     : PhysicalAddress,
                         memory_controller : &mut MemoryController<A>) -> LocalApic
        where A : FrameAllocator
    {
        assert!(register_base.is_frame_aligned(), "Expected local APIC registers to be frame aligned");
        memory_controller.active_table.map_to(Page::get_containing_page(LOCAL_APIC_REGISTER_SPACE),
                                              Frame::get_containing_frame(register_base),
                                              EntryFlags::WRITABLE,
                                              &mut memory_controller.frame_allocator);


        LocalApic
        {
            register_base,
        }
    }

    fn get_register_ptr(&self, offset : usize) -> *mut u32
    {
        LOCAL_APIC_REGISTER_SPACE.offset(offset as isize).mut_ptr() as *mut u32
    }

    pub fn enable(&self)
    {
        /*
         * Enable the APIC by setting bit 8 of the Spurious Interrupt Vector Register. Also set the
         * number of the spurious interrupt to 0xFF.
         */
        unsafe { ptr::write(self.get_register_ptr(0xF0), (1<<8) | 0xFF) };
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
    register_base   : PhysicalAddress,
}

impl IoApic
{
    pub unsafe fn new<A>(register_base      : PhysicalAddress,
                         memory_controller  : &mut MemoryController<A>) -> IoApic
        where A : FrameAllocator
    {
        // Map the register space to virtual memory
        assert!(register_base.is_frame_aligned(), "Expected IOAPIC registers to be frame aligned");
        memory_controller.active_table.map_to(Page::get_containing_page(IOAPIC_REGISTER_SPACE),
                                              Frame::get_containing_frame(register_base),
                                              EntryFlags::WRITABLE,
                                              &mut memory_controller.frame_allocator);
        serial_println!("Mapped IOAPIC register space");

        IoApic
        {
            register_base,
        }
    }
}
