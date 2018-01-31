/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use core::fmt;
use spin::Mutex;
use port::Port;

#[macro_export]
macro_rules! serial_println
{
    ($fmt:expr) => (serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (serial_print!(concat!($fmt, "\n"), $($arg)*));
}

#[macro_export]
macro_rules! serial_print
{
    ($($arg:tt)*) =>
        ({
            $crate::serial::print(format_args!($($arg)*));
        });
}

pub fn initialise()
{
    assert_first_call!("Tried to initialise serial ports multiple times!");

    unsafe
    {
        COM1.lock().initialise();
    }
}

pub fn print(args : fmt::Arguments)
{
    use core::fmt::Write;
    COM1.lock().write_fmt(args).unwrap();
}

pub static COM1 : Mutex<SerialPort> = Mutex::new(unsafe { SerialPort::new(0x3f8) });

pub struct SerialPort
{
    data_register               : Port<u8>,
    interrupt_enable_register   : Port<u8>,
    interrupt_identity_register : Port<u8>,
    line_control_register       : Port<u8>,
    modem_control_register      : Port<u8>,
    line_status_register        : Port<u8>,
}

impl fmt::Write for SerialPort
{
    fn write_str(&mut self, s : &str) -> fmt::Result
    {
        for byte in s.bytes()
        {
            unsafe
            {
                match byte
                {
                    /*
                     * XXX: Serial ports expect both a carriage return and a line feed ("\n\r")
                     */
                    b'\n' =>
                    {
                        self.write(b'\n');
                        self.write(b'\r');
                    },

                    _ => self.write(byte),
                }
            }
        }
        Ok(())
    }
}

impl SerialPort
{
    pub const unsafe fn new(address : u16) -> SerialPort
    {
        SerialPort
        {
            data_register               : Port::new(address + 0),
            interrupt_enable_register   : Port::new(address + 1),
            interrupt_identity_register : Port::new(address + 2),
            line_control_register       : Port::new(address + 3),
            modem_control_register      : Port::new(address + 4),
            line_status_register        : Port::new(address + 5),
        }
    }

    pub unsafe fn initialise(&mut self)
    {
        self.interrupt_enable_register.write(0x00);     // Disable IRQs
        self.line_control_register.write(0x80);         // Command - SET BAUD RATE DIVISOR
        self.data_register.write(0x03);                 // Set divisor to 0x03: 38400 baud (lo byte)
        self.interrupt_enable_register.write(0x00);     //                                 (hi byte)
        self.line_control_register.write(0x03);         // 8 bits, no parity, one stop bit
        self.interrupt_identity_register.write(0xC7);   // Enable FIFO, clear buffer, 14-byte thresh
        self.modem_control_register.write(0x0B);        // Enable IRQs, set RTS/DSR
    }

/*    pub unsafe fn read(&self) -> u8
    {
        while (self.line_status_register.read() & 1) == 0
        {
            // XXX: Required to stop loop from being optimized away
            asm!("" :::: "volatile");
        }

        self.data_register.read()
    }*/

    pub unsafe fn write(&mut self, value : u8)
    {
        while (self.line_status_register.read() & 0x20) == 0
        {
            asm!("" :::: "volatile");
        }

        self.data_register.write(value);
    }
}
