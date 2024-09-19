// SPDX-License-Identifier: MPL-2.0
// Copyright 2022, Isaac Woods

use bit_field::BitField as _;
use hal::memory::VAddr;
use volatile::Volatile;

/// The register block of a UART16550-compatible serial device. The usage of the registers are
/// explained well [here](https://www.lammertbies.nl/comm/info/serial-uart).
#[repr(C)]
pub struct Registers<R> {
    data: Volatile<R>,
    interrupt_enable: Volatile<R>,
    interrupt_identity: Volatile<R>,
    line_control: Volatile<R>,
    modem_control: Volatile<R>,
    line_status: Volatile<R>,
    modem_status: Volatile<R>,
    scratch: Volatile<R>,
}

pub enum Uart16550<'a> {
    One(&'a mut Registers<u8>),
    Four(&'a mut Registers<u32>),
}

impl<'a> Uart16550<'a> {
    pub unsafe fn new(addr: VAddr, reg_width: usize) -> Uart16550<'a> {
        match reg_width {
            1 => Self::One(unsafe { &mut *(addr.mut_ptr() as *mut Registers<u8>) }),
            4 => Self::Four(unsafe { &mut *(addr.mut_ptr() as *mut Registers<u32>) }),
            _ => panic!("Unsupported register width!"),
        }
    }

    pub fn init(&self) {
        // TODO: repeating this is pretty not great. Rewrite register definitions to make reg width
        // problems go away without doing this
        match self {
            Self::One(registers) => {
                // 8 data bits
                registers.line_control.write(0x03);
                // Clear pending interrupt (if any), no FIFOs, no modem status changes
                registers.interrupt_identity.write(0x01);
                // Interrupt on data received
                registers.interrupt_enable.write(0x01);

                // Setting bit 7 of LCR exposes the DLL and DLM registers
                let mut line_control = registers.line_control.read();
                line_control.set_bit(7, true);
                registers.line_control.write(line_control);
                // Set a baud rate of 115200 (DLL=0x01, DLM=0x00)
                registers.data.write(0x01);
                registers.interrupt_enable.write(0x00);
                // Unlatch the devisor registers again
                line_control.set_bit(7, false);
                registers.line_control.write(line_control);
            }
            Self::Four(registers) => {
                // 8 data bits
                registers.line_control.write(0x03);
                // Clear pending interrupt (if any), no FIFOs, no modem status changes
                registers.interrupt_identity.write(0x01);
                // Interrupt on data received
                registers.interrupt_enable.write(0x01);

                // Setting bit 7 of LCR exposes the DLL and DLM registers
                let mut line_control = registers.line_control.read();
                line_control.set_bit(7, true);
                registers.line_control.write(line_control);
                // Set a baud rate of 115200 (DLL=0x01, DLM=0x00)
                registers.data.write(0x01);
                registers.interrupt_enable.write(0x00);
                // Unlatch the devisor registers again
                line_control.set_bit(7, false);
                registers.line_control.write(line_control);
            }
        }
    }

    fn line_status(&self) -> u8 {
        match self {
            Self::One(registers) => registers.line_status.read(),
            Self::Four(registers) => registers.line_status.read() as u8,
        }
    }

    pub fn write(&self, data: u8) {
        match self {
            Self::One(registers) => registers.data.write(data),
            Self::Four(registers) => registers.data.write(data as u32),
        }
        while !self.line_status().get_bit(5) {}
    }

    pub fn read(&self) -> Option<u8> {
        if self.line_status().get_bit(0) {
            match self {
                Self::One(registers) => Some(registers.data.read()),
                Self::Four(registers) => Some(registers.data.read() as u8),
            }
        } else {
            None
        }
    }
}

impl<'a> core::fmt::Write for Uart16550<'a> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            self.write(byte);
        }
        Ok(())
    }
}
