#![no_std]
#![no_main]

use bit_field::BitField;
use core::fmt::Write;
use volatile::Volatile;

core::arch::global_asm!(
    "
    .section .text.start
    .global _start
    _start:
        // Zero the BSS
        la t0, _bss_start
        la t1, _bss_end
        bgeu t0, t1, .bss_zero_loop_end
    .bss_zero_loop:
        sd zero, (t0)
        addi t0, t0, 8
        bltu t0, t1, .bss_zero_loop
    .bss_zero_loop_end:

        la sp, _stack_top

        jal main
        unimp
"
);

/*
 * TODO:
 * We originally thought we'd need this boot shim to load OpenSBI even when booting over FEL.
 * However, we've managed to do that without one - this is left over and committed because we'll
 * need it at some point to load from persistent media on the D1. It'll need plenty more work,
 * including code to initialize DRAM, special headers to be loaded by the BROM, and a small SDHC
 * driver to load OpenSBI and Seed from the SD card.
 *
 * For now, it should basically be ignored, and is just in-tree to prevent me from having to do the
 * work of setting it up again when we work on the next bit.
 */

// TODO: We use UART0 on the MangoPi and UART1 on the uConsole as it's exposed via GPIO
// const UART_ADDRESS: usize = 0x0250_0000;
const UART_ADDRESS: usize = 0x0250_0400;

#[no_mangle]
pub fn main() -> ! {
    // XXX: for the uConsole, we need to do weird stuff to use UART1 on the GPIO pins
    {
        unsafe {
            // APB1_CFG_REG should already be configured by the DDR init setting up UART0

            // Configure PG6 and PG7 to be UART1 TX and RX
            let pg_cfg0 = &mut *((0x0200_0000 + 0x0120) as *mut Volatile<u32>);
            let mut value = pg_cfg0.read();
            value.set_bits(24..28, 0b0010);
            pg_cfg0.write(value);

            let mut value = pg_cfg0.read();
            value.set_bits(28..32, 0b0010);
            pg_cfg0.write(value);

            // Set PG6 and PG7 to be internally pulled-up (doesn't seem to be done by xfel but
            // recommended in manual?)
            // let pg_pull0 = &mut *((0x0200_0000 + 0x0144) as *mut Volatile<u32>);
            // let mut value = pg_pull0.read();
            // value.set_bits(12..14, 0b01);
            // value.set_bits(14..16, 0b01);
            // pg_pull0.write(value);

            // Open the clock gate for UART1
            let uart_bgr_reg = &mut *((0x0200_1000 + 0x090c) as *mut Volatile<u32>);
            let mut value = uart_bgr_reg.read();
            value.set_bit(1, true);
            uart_bgr_reg.write(value);

            // De-assert UART1's reset
            let mut value = uart_bgr_reg.read();
            value.set_bit(17, true);
            uart_bgr_reg.write(value);
        }
    }

    let serial = unsafe { &mut *(UART_ADDRESS as *mut Uart) };
    serial.init();
    writeln!(serial, "Poplar's boot0 is running!").unwrap();

    let hart_id = unsafe {
        let value: usize;
        core::arch::asm!("csrr {}, mhartid", out(reg) value);
        value
    };
    writeln!(serial, "HART id: {}", hart_id).unwrap();

    loop {}
}

#[repr(C)]
pub struct Uart {
    data: Volatile<u32>,
    interrupt_enable: Volatile<u32>,
    interrupt_identity: Volatile<u32>,
    line_control: Volatile<u32>,
    modem_control: Volatile<u32>,
    line_status: Volatile<u32>,
    modem_status: Volatile<u32>,
    scratch: Volatile<u32>,
}

impl Uart {
    pub fn init(&self) {
        // TODO: this does differ from the manual in a few ways but not sure if it'd have any
        // impact - it doesn't seem to break the D1 on the MqPro? (oh wait it actually does in the
        // kernel but not here?? Even weirder)

        // 8 data bits
        self.line_control.write(0x03);
        // Clear pending interrupt (if any), no FIFOs, no modem status changes
        self.interrupt_identity.write(0x01);
        // Interrupt on data received
        self.interrupt_enable.write(0x01);

        // Setting bit 7 of LCR exposes the DLL and DLM registers
        let line_control = self.line_control.read();
        self.line_control.write(line_control | (1 << 7));
        // Set a baud rate of 115200 (DLL=0x01, DLM=0x00)
        self.data.write(0x01);
        self.interrupt_enable.write(0x00);
        self.line_control.write(line_control);
    }

    fn line_status(&self) -> u32 {
        self.line_status.read()
    }

    pub fn write(&self, data: u8) {
        while (self.line_status() & 0x20) == 0 {}
        self.data.write(data as u32);
    }
}

impl core::fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            self.write(byte);
        }
        Ok(())
    }
}

#[cfg(not(test))]
#[panic_handler]
pub fn panic(_: &core::panic::PanicInfo) -> ! {
    let serial = unsafe { &mut *(UART_ADDRESS as *mut Uart) };
    let _ = write!(serial, "boot0: PANIC!");
    loop {}
}
