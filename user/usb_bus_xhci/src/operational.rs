use bit_field::BitField;
use core::ptr;

pub struct OperationRegisters {
    base: usize,
    num_ports: u8,
}

impl OperationRegisters {
    pub unsafe fn new(base: usize, num_ports: u8) -> OperationRegisters {
        OperationRegisters { base, num_ports }
    }

    pub fn usb_command(&self) -> UsbCommand {
        UsbCommand(unsafe { self.read_register(0x00) })
    }

    pub fn usb_status(&self) -> UsbStatus {
        UsbStatus(unsafe { self.read_register(0x04) })
    }

    pub fn device_notification_control(&self) -> u32 {
        unsafe { self.read_register(0x14) }
    }

    pub fn set_device_context_base_address_array_pointer(&self, pointer: u64) {
        assert_eq!(pointer.get_bits(0..6), 0x0);
        unsafe {
            self.write_register(0x30, pointer);
        }
    }

    pub fn update_config<F>(&mut self, f: F)
    where
        F: FnOnce(Config) -> Config,
    {
        let config = Config(unsafe { self.read_register(0x38) });
        let new_config = f(config);
        unsafe {
            self.write_register(0x38, new_config.0);
        }
    }

    unsafe fn read_register<T>(&self, offset: usize) -> T {
        unsafe { ptr::read_volatile((self.base + offset) as *const T) }
    }

    unsafe fn write_register<T>(&self, offset: usize, value: T) {
        unsafe {
            ptr::write_volatile((self.base + offset) as *mut T, value);
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct UsbCommand(u32);

impl UsbCommand {
    pub fn is_running(&self) -> bool {
        self.0.get_bit(0)
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct UsbStatus(u32);

impl UsbStatus {
    pub fn controller_not_ready(&self) -> bool {
        self.0.get_bit(11)
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct Config(u32);

impl Config {
    pub fn device_slots_enabled(&self) -> u8 {
        self.0.get_bits(0..8) as u8
    }

    pub fn set_device_slots_enabled(&mut self, slots: u8) {
        self.0.set_bits(0..8, slots as u32);
    }
}
