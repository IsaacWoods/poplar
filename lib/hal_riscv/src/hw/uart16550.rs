// SPDX-License-Identifier: MPL-2.0
// Copyright 2022, Isaac Woods

use volatile::Volatile;

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
    pub unsafe fn new(addr: usize, reg_width: usize) -> Uart16550<'a> {
        match reg_width {
            1 => Self::One(unsafe { &mut *(addr as *mut Registers<u8>) }),
            4 => Self::Four(unsafe { &mut *(addr as *mut Registers<u32>) }),
            _ => panic!("Unsupported register width!"),
        }
    }

    fn line_status(&self) -> u8 {
        match self {
            Self::One(registers) => registers.line_status.read(),
            Self::Four(registers) => registers.line_status.read() as u8,
        }
    }

    pub fn write(&self, data: u8) {
        while (self.line_status() & 0x20) == 0 {}
        match self {
            Self::One(registers) => registers.data.write(data),
            Self::Four(registers) => registers.data.write(data as u32),
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
