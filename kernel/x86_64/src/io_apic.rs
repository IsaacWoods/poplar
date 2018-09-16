use bit_field::BitField;
use core::ptr;
use memory::paging::{EntryFlags, PhysicalAddress, PhysicalMapping, VirtualAddress};
use memory::MemoryController;

#[derive(Clone, Copy, Debug)]
pub enum DeliveryMode {
    Fixed,
    LowestPriority,
    SMI,
    NMI,
    INIT,
    ExtINT,
}

#[derive(Clone, Copy, Debug)]
pub enum PinPolarity {
    Low,
    High,
}

#[derive(Clone, Copy, Debug)]
pub enum TriggerMode {
    Edge,
    Level,
}

#[derive(Clone, Debug)]
pub struct IoApic {
    register_base: PhysicalAddress,
    mapping: PhysicalMapping<u32>,
    global_interrupt_base: u8,
}

impl IoApic {
    pub unsafe fn new(
        register_base: PhysicalAddress,
        global_interrupt_base: u8,
        memory_controller: &mut MemoryController,
    ) -> IoApic {
        IoApic {
            register_base,
            mapping: memory_controller.kernel_page_table.map_physical_region(
                register_base,
                register_base.offset(4096 - 1),
                EntryFlags::WRITABLE | EntryFlags::NO_CACHE,
                &mut memory_controller.frame_allocator,
            ),
            global_interrupt_base,
        }

        /*
         * Map all ISA IRQs (these can be remapped by Interrupt Source Override entries in the
         * MADT tho)
         * TODO: should we be doing this? Definitely not here
         */
        // for irq in 0..16 {
        //     // Assume all non-overriden ISA IRQs are active-high and edge-triggered
        //     self.write_entry(
        //         irq,
        //         ::interrupts::IOAPIC_BASE + irq,
        //         DeliveryMode::Fixed,
        //         PinPolarity::High,
        //         TriggerMode::Edge,
        //         true, // Masked by default
        //         0xff,
        //     );
        // }
    }

    pub fn global_interrupt_base(&self) -> u8 {
        self.global_interrupt_base
    }

    unsafe fn read_register(&self, register: u32) -> u32 {
        ptr::write_volatile(self.mapping.ptr, register);
        ptr::read_volatile(VirtualAddress::from(self.mapping.ptr).offset(0x10).ptr())
    }

    unsafe fn write_register(&self, register: u32, value: u32) {
        ptr::write_volatile(self.mapping.ptr, register);
        ptr::write_volatile(
            VirtualAddress::from(self.mapping.ptr)
                .offset(0x10)
                .mut_ptr(),
            value,
        );
    }

    pub fn set_irq_mask(&self, irq: u8, masked: bool) {
        let mut entry = self.read_entry_raw(irq);
        entry.set_bit(16, masked);
        unsafe {
            self.write_entry_raw(irq, entry);
        }
    }

    fn read_entry_raw(&self, irq: u8) -> u64 {
        let register_base = 0x10 + u32::from(irq) * 2;
        let mut entry: u64 = 0;
        unsafe {
            entry.set_bits(0..32, u64::from(self.read_register(register_base + 0)));
            entry.set_bits(32..64, u64::from(self.read_register(register_base + 1)));
        }
        entry
    }

    unsafe fn write_entry_raw(&self, irq: u8, entry: u64) {
        let register_base = 0x10 + u32::from(irq) * 2;
        self.write_register(register_base + 0, entry.get_bits(0..32) as u32);
        self.write_register(register_base + 1, entry.get_bits(32..64) as u32);
    }

    /*
     * NOTE: We always use Physical Destination Mode.
     */
    #[allow(too_many_arguments)]
    pub fn write_entry(
        &self,
        irq: u8,
        vector: u8,
        delivery_mode: DeliveryMode,
        pin_polarity: PinPolarity,
        trigger_mode: TriggerMode,
        masked: bool,
        destination: u8,
    ) {
        let mut entry: u64 = 0;
        entry.set_bits(0..8, u64::from(vector));
        entry.set_bits(
            8..11,
            match delivery_mode {
                DeliveryMode::Fixed => 0b000,
                DeliveryMode::LowestPriority => 0b001,
                DeliveryMode::SMI => 0b010,
                DeliveryMode::NMI => 0b100,
                DeliveryMode::INIT => 0b101,
                DeliveryMode::ExtINT => 0b111,
            },
        );
        entry.set_bit(11, false); // Destination mode - 0 => Physical Destination Mode
        entry.set_bit(12, false); // Delivery status - 0 => IRQ is relaxed
        entry.set_bit(
            13,
            match pin_polarity {
                PinPolarity::Low => true,
                PinPolarity::High => false,
            },
        );
        entry.set_bit(14, false); // Remote IRR - TODO: what does this do?
        entry.set_bit(
            15,
            match trigger_mode {
                TriggerMode::Edge => false,
                TriggerMode::Level => true,
            },
        );
        entry.set_bit(16, masked);
        entry.set_bits(56..64, u64::from(destination));

        unsafe {
            self.write_entry_raw(irq, entry);
        }
    }
}
