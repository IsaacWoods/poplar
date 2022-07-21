#![no_std]
#![no_main]

/*
 * This is the entry-point jumped to from OpenSBI. It needs to be at the very start of the ELF, so we put it in its
 * own section and then place it manually during linking.
 */
core::arch::global_asm!(
    "
    .section .text.entry
    .global _start
    _start:
        la sp, _stack_top
        mv fp, sp

        li a0, '!'
        li a6, 0
        li a7, 1
        ecall
        li a0, '\n'
        ecall

        j kmain
"
);

#[no_mangle]
pub fn kmain() -> ! {
    let uart = unsafe { &mut *(0x10000000 as *mut hal_riscv::hw::uart16550::Uart16550) };
    use core::fmt::Write;
    write!(uart, "Hello, World!").unwrap();
    loop {}
}

#[panic_handler]
pub fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
