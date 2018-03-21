/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use core::ptr;
use spin::Mutex;
use bit_field::BitField;
use ::memory::{MemoryController,FrameAllocator};
use ::memory::paging::{PhysicalAddress,VirtualAddress,EntryFlags,PhysicalMapping};
use interrupts::InterruptStackFrame;

pub static mut LOCAL_APIC   : LocalApic = LocalApic::placeholder();
pub static mut IO_APIC      : IoApic    = IoApic::placeholder();

pub extern "C" fn apic_timer_handler(_ : &InterruptStackFrame)
{
    unsafe { LOCAL_APIC.send_eoi(); }
}

#[derive(Clone,Debug)]
pub struct LocalApic
{
    is_enabled      : bool,
    register_base   : PhysicalAddress,
    mapping         : Option<PhysicalMapping<u32>>,
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
            mapping         : None,
        }
    }

    pub unsafe fn register_ptr(&self, offset : usize) -> *mut u32
    {
        let mapping = self.mapping.as_ref().expect("Tried to get register ptr to unmapped local APIC");
        VirtualAddress::from(mapping.ptr).offset(offset as isize).mut_ptr() as *mut u32
    }

    pub unsafe fn enable<A>(&mut self,
                            register_base       : PhysicalAddress,
                            memory_controller   : &mut MemoryController<A>)
        where A : FrameAllocator
    {
        assert!(!self.is_enabled);

        // Map the configuration space into virtual memory
        self.mapping = Some(memory_controller.kernel_page_table
                                             .map_physical_region(register_base,
                                                                  register_base.offset(4096-1),
                                                                  EntryFlags::WRITABLE /*| EntryFlags::NO_CACHE*/,
                                                                  &mut memory_controller.frame_allocator));

        /*
         * - Enable the local APIC by setting bit 8
         * - Set eh spurious interrupt vector
         */
        let spurious_interrupt_vector = (1<<8) | ::interrupts::APIC_SPURIOUS_INTERRUPT as u32;
        ptr::write_volatile(self.register_ptr(0xF0), spurious_interrupt_vector);
    }

    /// Set the local APIC timer to interrupt every `duration` ms, and then enable it
    pub fn enable_timer(&self, duration : usize)
    {
        trace!("Timing local APIC bus frequency [freezing here suggests problem with PIT sleep]");
        unsafe
        {
            /*
             * Set divide value to 16 and initial counter value to -1. We use 16 because apparently
             * some hardware has issues with other divide values (especially 1, which would be the
             * simplest otherwise). 16 seems to be the most supported.
             */
            ptr::write_volatile(self.register_ptr(0x3E0), 0x3);
            ptr::write_volatile(self.register_ptr(0x380), 0xFFFFFFFF);

            /*
             * Sleep for 10ms with the PIT and then stop the APIC timer
             */
            ::pit::PIT.do_sleep(10);
            ptr::write_volatile(self.register_ptr(0x320), 0x10000);

            let ticks_in_10ms = (0xFFFFFFFF - ptr::read_volatile(self.register_ptr(0x390))) as usize;
            trace!("Timing of local APIC bus frequency complete");

            /*
             * Start the APIC timer in Periodic mode with a divide value of 16 again, to interrupt
             * every 10 ms.
             */
            ptr::write_volatile(self.register_ptr(0x320), ::interrupts::LOCAL_APIC_TIMER as u32 | 0x20000);
            ptr::write_volatile(self.register_ptr(0x3E0), 0x3);
            ptr::write_volatile(self.register_ptr(0x380), ((ticks_in_10ms / 10) * duration) as u32);
        }
    }

    pub fn send_eoi(&self)
    {
        /*
         * To send an EOI, we write 0 to the register with offset 0xB0. Writing any other value
         * will cause a #GP.
         */
        unsafe { ptr::write_volatile(self.register_ptr(0xB0), 0) };
    }
}

#[derive(Clone,Debug)]
pub struct IoApic
{
    is_enabled              : bool,
    register_base           : PhysicalAddress,
    mapping                 : Option<PhysicalMapping<u32>>,
    global_interrupt_base   : u8,
}

#[allow(unused)]
pub enum DeliveryMode
{
    Fixed,
    LowestPriority,
    SMI,
    NMI,
    INIT,
    ExtINT,
}

pub enum PinPolarity
{
    Low,
    High,
}

pub enum TriggerMode
{
    Edge,
    Level,
}

impl IoApic
{
    const fn placeholder() -> IoApic
    {
        IoApic
        {
            is_enabled              : false,
            register_base           : PhysicalAddress::new(0),
            mapping                 : None,
            global_interrupt_base   : 0,
        }
    }

    pub unsafe fn enable<A>(&mut self,
                            register_base           : PhysicalAddress,
                            global_interrupt_base   : u8,
                            memory_controller       : &mut MemoryController<A>)
        where A : FrameAllocator
    {
        assert!(!self.is_enabled);
        self.global_interrupt_base = global_interrupt_base;

        // Map the configuration space to virtual memory
        self.mapping = Some(memory_controller.kernel_page_table
                                             .map_physical_region(register_base,
                                                                  register_base.offset(4096-1),
                                                                  EntryFlags::WRITABLE | EntryFlags::NO_CACHE,
                                                                  &mut memory_controller.frame_allocator));

        /*
         * Map all ISA IRQs (these can be remapped by Interrupt Source Override entries in the
         * MADT tho)
         */
        for irq in 0..16
        {
            // Assume all non-overriden ISA IRQs are active-high and edge-triggered
            self.write_entry(irq,
                             ::interrupts::IOAPIC_BASE + irq,
                             DeliveryMode::Fixed,
                             PinPolarity::High,
                             TriggerMode::Edge,
                             true,  // Masked by default
                             0xff);
        }
    }

    pub fn global_interrupt_base(&self) -> u8
    {
        self.global_interrupt_base
    }

    unsafe fn read_register(&self, register : u32) -> u32
    {
        let mapping = self.mapping.as_ref().expect("Tried to read register for unmapped IOAPIC");
        ptr::write_volatile(mapping.ptr, register);
        ptr::read_volatile(VirtualAddress::from(mapping.ptr).offset(0x10).ptr())
    }

    unsafe fn write_register(&self, register : u32, value : u32)
    {
        let mapping = self.mapping.as_ref().expect("Tried to read register for unmapped IOAPIC");
        ptr::write_volatile(mapping.ptr, register);
        ptr::write_volatile(VirtualAddress::from(mapping.ptr).offset(0x10).mut_ptr(), value);
    }

    pub fn set_irq_mask(&self, irq : u8, masked : bool)
    {
        let mut entry = self.read_entry_raw(irq);
        entry.set_bit(16, masked);
        unsafe { self.write_entry_raw(irq, entry); }
    }

    fn read_entry_raw(&self, irq : u8) -> u64
    {
        let register_base = 0x10 + (irq as u32) * 2;
        let mut entry : u64 = 0;
        unsafe
        {
            entry.set_bits(0..32 , self.read_register(register_base + 0) as u64);
            entry.set_bits(32..64, self.read_register(register_base + 1) as u64);
        }
        entry
    }

    unsafe fn write_entry_raw(&self, irq : u8, entry : u64)
    {
        let register_base = 0x10 + (irq as u32) * 2;
        self.write_register(register_base + 0, entry.get_bits(0..32) as u32);
        self.write_register(register_base + 1, entry.get_bits(32..64) as u32);
    }

    /*
     * NOTE: We always use Physical Destination Mode.
     */
    pub fn write_entry(&self,
                       irq              : u8,
                       vector           : u8,
                       delivery_mode    : DeliveryMode,
                       pin_polarity     : PinPolarity,
                       trigger_mode     : TriggerMode,
                       masked           : bool,
                       destination      : u8)
    {
        let mut entry : u64 = 0;
        entry.set_bits(0..8, vector as u64);
        entry.set_bits(8..11, match delivery_mode
                              {
                                  DeliveryMode::Fixed           => 0b000,
                                  DeliveryMode::LowestPriority  => 0b001,
                                  DeliveryMode::SMI             => 0b010,
                                  DeliveryMode::NMI             => 0b100,
                                  DeliveryMode::INIT            => 0b101,
                                  DeliveryMode::ExtINT          => 0b111,
                              });
        entry.set_bit(11, false);   // Destination mode - 0 => Physical Destination Mode
        entry.set_bit(12, false);   // Delivery status - 0 => IRQ is relaxed
        entry.set_bit(13, match pin_polarity
                          {
                              PinPolarity::Low  => true,
                              PinPolarity::High => false,
                          });
        entry.set_bit(14, false);   // Remote IRR - TODO: what does this do?
        entry.set_bit(15, match trigger_mode
                          {
                              TriggerMode::Edge     => false,
                              TriggerMode::Level    => true,
                          });
        entry.set_bit(16, masked);
        entry.set_bits(56..64, destination as u64);

        unsafe { self.write_entry_raw(irq, entry); }
    }
}
