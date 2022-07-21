// SPDX-License-Identifier: MPL-2.0
// Copyright 2022, Isaac Woods

use volatile::Volatile;

// TODO: this is the same as the one on x86, but accessed via MMIO rather than port-based. Can we abstract over
// this to make a common driver in e.g. `hal`?
#[repr(C)]
pub struct Uart16550 {
    data: Volatile<u8>,
    interrupt_enable: Volatile<u8>,
    interrupt_identity: Volatile<u8>,
    line_control: Volatile<u8>,
    modem_control: Volatile<u8>,
    line_status: Volatile<u8>,
    modem_status: Volatile<u8>,
    scratch: Volatile<u8>,
}

impl Uart16550 {
    fn line_status(&self) -> u8 {
        self.line_status.read()
    }

    pub fn write(&self, data: u8) {
        while (self.line_status() & 0x20) == 0 {}
        self.data.write(data);
    }
}

impl core::fmt::Write for Uart16550 {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            self.write(byte);
        }
        Ok(())
    }
}
