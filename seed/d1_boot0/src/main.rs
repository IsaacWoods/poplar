#![no_std]
#![no_main]

core::arch::global_asm!("
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
");

#[no_mangle]
pub fn main() -> ! {
    let uart_data: *mut u8 = 0x0250_0000 as *mut u8;
    unsafe {
        uart_data.write_volatile(b'H');
        uart_data.write_volatile(b'e');
        uart_data.write_volatile(b'l');
        uart_data.write_volatile(b'l');
        uart_data.write_volatile(b'o');
        uart_data.write_volatile(b'!');
    }
    
    loop {}
}

#[panic_handler]
pub fn panic(_: &core::panic::PanicInfo) -> ! {
    let uart_data: *mut u8 = 0x0250_0000 as *mut u8;
    unsafe {
        uart_data.write_volatile(b'X');
    }
    // if let Some(message) = info.message() {
    //     if let Some(location) = info.location() {
    //         let _ = writeln!(
    //             LOGGER.serial.lock(),
    //             "PANIC: {} ({} - {}:{})",
    //             message,
    //             location.file(),
    //             location.line(),
    //             location.column()
    //         );
    //     } else {
    //         let _ = writeln!(LOGGER.serial.lock(), "PANIC: {} (no location info)", message);
    //     }
    // }
    loop {}
}
