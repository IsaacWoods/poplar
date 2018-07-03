//! We only support PCI access mechanism 1. Mechanism 2 is only likely to exist in hardware from
//! 1992-1993, and systems with memory-mapped PCI access will also support mechanism 1 for
//! backwards compatibility.
//!
//! In the future, we should add support for memory-mapped access for PCIe. We should also check
//! the ACPI tables to see if the hardware supports each mechanism - it can be dangerous to probe
//! IO ports that don't exist.

use bit_field::BitField;
use port::Port;

pub struct Pci {
    address_port: Port<u32>,
    data_port: Port<u32>,
}

impl Pci {
    pub unsafe fn new() -> Pci {
        Pci {
            address_port: Port::new(0xcf8),
            data_port: Port::new(0xcfc),
        }
    }

    pub fn scan(&mut self) {
        info!("Scanning PCI bus");
        let header_type = self.get_header_type(0, 0, 0);

        if header_type.get_bit(7) {
            /*
             * There are multiple PCI host controllers.
             */
            for function in 0..8 {
                if self.get_vendor_id(0, 0, function) != 0xffff {
                    break;
                }
                self.check_bus(function);
            }
        } else {
            /*
             * It's a single PCI host controller.
             */
            self.check_bus(0);
        }
    }

    fn check_bus(&mut self, bus: u8) {
        for device in 0..32 {
            self.check_device(bus, device);
        }
    }

    fn check_device(&mut self, bus: u8, device: u8) {
        let vendor_id = self.get_vendor_id(bus, device, 0);

        // Return if the device doesn't exist
        if vendor_id == 0xffff {
            return;
        }

        let device_id = self.get_device_id(bus, device, 0);
        info!("Found PCI device with vendor id: {:#x}, device id: {:#x}", vendor_id, device_id);

        self.check_function(bus, device, 0);
        let header_type = self.get_header_type(bus, device, 0);

        // If it's a multi-function device, we check every function
        if header_type.get_bit(7) {
            for function in 1..8 {
                if self.get_vendor_id(bus, device, function) != 0xffff {
                    self.check_function(bus, device, function);
                }
            }
        }
    }

    fn check_function(&mut self, bus: u8, device: u8, function: u8) {
        let (class, subclass) = {
            let word = self.read_config_word(bus, device, function, 0xa);
            (word.get_bits(8..15) as u8, word.get_bits(0..8) as u8)
        };
        info!("Found PCI function with class={:#x}, subclass={:#x}", class, subclass);

        // If the function is a PCI-PCI bridge, we need to scan the bus it connects to
        if class == 0x06 && subclass == 0x04 {
            let secondary_bus = self.read_config_word(bus, device, function, 0x18).get_bits(8..16) as u8;
            self.check_bus(secondary_bus);
        }
    }

    fn get_header_type(&mut self, bus: u8, device: u8, function: u8) -> u8 {
        self.read_config_word(bus, device, function, 0xe).get_bits(0..8) as u8
    }

    fn get_vendor_id(&mut self, bus: u8, device: u8, function: u8) -> u16 {
        self.read_config_word(bus, device, function, 0x0)
    }

    fn get_device_id(&mut self, bus: u8, device: u8, function: u8) -> u16 {
        self.read_config_word(bus, device, function, 0x2)
    }

    fn read_config_word(&mut self, bus: u8, device: u8, function: u8, offset: u8) -> u16 {
        /*
         * |------------|----------|------------|---------------|-----------------|---|---|
         * |     31     |  23-16   |   15-11    |      10-8     |      7-2        | 1 | 0 |
         * |------------|----------|------------|---------------|-----------------|---|---|
         * | Enable Bit | Reserved | Bus Number | Device Number | Register Number | 0 | 0 |
         * |------------|----------|------------|---------------|-----------------|---|---|
         *
         * The Enable Bit should be set if accesses to the data port should be translated to
         * configuration cycles.
         *
         * The register number in the address is number of the 32-bit register (`offset & 0xfc` to
         * set the last two bits to 0), then we mask to get the correct word of the register.
         */
        let mut address: u32 = 0;
        address.set_bit(31, true); // Set the Enable Bit
        address.set_bits(16..24, bus as u32);
        address.set_bits(11..16, device as u32);
        address.set_bits(8..11, function as u32);
        address.set_bits(0..8, (offset & 0xfc) as u32);

        unsafe {
            self.address_port.write(address);
        }
        (unsafe { self.data_port.read() } >> ((offset & 2) * 8) & 0xffff) as u16
    }
}
