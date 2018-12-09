use super::port::Port;
use core::fmt;

pub const COM1: u16 = 0x3f8;

pub struct SerialPort {
    data_register: Port<u8>,
    interrupt_enable_register: Port<u8>,
    interrupt_identity_register: Port<u8>,
    line_control_register: Port<u8>,
    modem_control_register: Port<u8>,
    line_status_register: Port<u8>,
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            unsafe {
                match byte {
                    /*
                     * Serial ports expect both a carriage return and a line feed ("\n\r") for
                     * newlines.
                     */
                    b'\n' => {
                        self.write(b'\n');
                        self.write(b'\r');
                    }

                    _ => self.write(byte),
                }
            }
        }
        Ok(())
    }
}

impl SerialPort {
    pub const unsafe fn new(address: u16) -> SerialPort {
        SerialPort {
            data_register: Port::new(address),
            interrupt_enable_register: Port::new(address + 1),
            interrupt_identity_register: Port::new(address + 2),
            line_control_register: Port::new(address + 3),
            modem_control_register: Port::new(address + 4),
            line_status_register: Port::new(address + 5),
        }
    }

    pub unsafe fn initialise(&mut self) {
        // Disable IRQs
        self.interrupt_enable_register.write(0x00);

        // Set baud rate divisor to 0x0003 (38400 baud rate)
        self.line_control_register.write(0x80);
        self.data_register.write(0x03);
        self.interrupt_enable_register.write(0x00);

        // 8 bits, no parity bits, one stop bit
        self.line_control_register.write(0x03);

        // Enable FIFO, clear buffer, 14-byte thresh
        self.interrupt_identity_register.write(0xC7);

        // Enable IRQs again, set RTS/DSR
        self.modem_control_register.write(0x0B);
    }

    #[allow(unused)]
    pub unsafe fn read(&self) -> u8 {
        while (self.line_status_register.read() & 1) == 0 {
            // XXX: Required to stop loop from being optimized away
            asm!("" :::: "volatile");
        }

        self.data_register.read()
    }

    pub unsafe fn write(&mut self, value: u8) {
        while (self.line_status_register.read() & 0x20) == 0 {
            asm!("" :::: "volatile");
        }

        self.data_register.write(value);
    }
}
