use bit_field::BitField;
use core::ptr;
use hal::memory::VirtualAddress;

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

#[derive(Debug)]
pub struct IoApic {
    pub config_area_base: VirtualAddress,
    pub global_interrupt_base: u32,
}

impl IoApic {
    /// Create a new `IoApic` with the specified GSI (global system interrupt)
    /// base, whose config area is mapped to the specified virtual address.
    ///
    /// # Unsafety
    /// Assumes that the config area is correctly mapped to the given virtual
    /// address.
    pub unsafe fn new(config_area_base: VirtualAddress, global_interrupt_base: u32) -> IoApic {
        IoApic { config_area_base, global_interrupt_base }
    }

    pub fn set_irq_mask(&mut self, irq: u32, masked: bool) {
        let mut entry = self.read_raw_entry(irq);
        entry.set_bit(16, masked);
        self.write_raw_entry(irq, entry);
    }

    /// Write an IRQs entry using a more convenient interface than
    /// `write_raw_entry`. This always uses Physical Destination Mode.
    pub fn write_entry(
        &mut self,
        irq: u32,
        vector: u8,
        delivery_mode: DeliveryMode,
        polarity: PinPolarity,
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
            match polarity {
                PinPolarity::Low => true,
                PinPolarity::High => false,
            },
        );
        entry.set_bit(14, false); // Remote IRR
        entry.set_bit(
            15,
            match trigger_mode {
                TriggerMode::Edge => false,
                TriggerMode::Level => true,
            },
        );
        entry.set_bit(16, masked);
        entry.set_bits(56..64, u64::from(destination));

        self.write_raw_entry(irq, entry);
    }

    pub fn num_redirection_entries(&self) -> u32 {
        /*
         * Bits 16-23 contain the maximum redirection entry index, so add 1 to get
         * the number of redirection entries.
         */
        unsafe { self.read_register(0x1) }.get_bits(16..24) + 1
    }

    unsafe fn read_register(&self, register: u32) -> u32 {
        ptr::write_volatile(self.config_area_base.mut_ptr(), register);
        ptr::read_volatile((self.config_area_base + 0x10).ptr())
    }

    unsafe fn write_register(&mut self, register: u32, value: u32) {
        ptr::write_volatile(self.config_area_base.mut_ptr(), register);
        ptr::write_volatile((self.config_area_base + 0x10).mut_ptr(), value);
    }

    fn read_raw_entry(&self, irq: u32) -> u64 {
        let register_base = 0x10 + irq * 2;
        let mut entry: u64 = 0;

        entry.set_bits(0..32, u64::from(unsafe { self.read_register(register_base + 0) }));
        entry.set_bits(32..64, u64::from(unsafe { self.read_register(register_base + 1) }));

        entry
    }

    fn write_raw_entry(&mut self, irq: u32, entry: u64) {
        let register_base = 0x10 + irq * 2;
        unsafe {
            self.write_register(register_base + 0, entry.get_bits(0..32) as u32);
            self.write_register(register_base + 1, entry.get_bits(32..64) as u32);
        }
    }
}
