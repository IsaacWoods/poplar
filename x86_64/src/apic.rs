/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use core::ptr;
use spin::Mutex;
use bit_field::BitField;
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

    pub unsafe fn register_ptr(&self, offset : usize) -> *mut u32
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
        memory_controller.kernel_page_table.map_to(Page::containing_page(LOCAL_APIC_REGISTER_SPACE),
                                                   Frame::containing_frame(register_base),
                                                   EntryFlags::WRITABLE,
                                                   &mut memory_controller.frame_allocator);

        let spurious_interrupt_vector = (1<<8) |                                        // Enable the local APIC by setting bit 8
                                        ::interrupts::APIC_SPURIOUS_INTERRUPT as u32;   // Set the interrupt vector of the spurious interrupt
        ptr::write_volatile(self.register_ptr(0xF0), spurious_interrupt_vector);
    }

    pub fn enable_timer(&self, frequency : u64)
    {
        // TODO: use the PIT or something to actually calculate the frequency the APIC is running at
        unsafe
        {
            ptr::write_volatile(self.register_ptr(0x320), ::interrupts::LOCAL_APIC_TIMER as u32 | 0x20000); // Set the LVT entry
            ptr::write_volatile(self.register_ptr(0x3E0), 0x3);          // Set the timer divisor = 16
            ptr::write_volatile(self.register_ptr(0x380), 100000000);    // Set initial count
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

#[derive(Clone,Copy,Debug)]
pub struct IoApic
{
    is_enabled              : bool,
    register_base           : PhysicalAddress,
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
        assert!(register_base.is_frame_aligned(), "Expected IOAPIC registers to be frame aligned");
        memory_controller.kernel_page_table.map_to(Page::containing_page(IOAPIC_REGISTER_SPACE),
                                                   Frame::containing_frame(register_base),
                                                   EntryFlags::WRITABLE,
                                                   &mut memory_controller.frame_allocator);

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
        ptr::write_volatile(IOAPIC_REGISTER_SPACE.mut_ptr() as *mut u32, register);
        ptr::read_volatile(IOAPIC_REGISTER_SPACE.offset(0x10).ptr() as *const u32)
    }

    unsafe fn write_register(&self, register : u32, value : u32)
    {
        ptr::write_volatile(IOAPIC_REGISTER_SPACE.mut_ptr() as *mut u32, register);
        ptr::write_volatile(IOAPIC_REGISTER_SPACE.offset(0x10).mut_ptr() as *mut u32, value);
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
