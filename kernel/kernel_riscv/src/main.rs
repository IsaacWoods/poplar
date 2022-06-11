#![no_std]
#![no_main]

core::arch::global_asm!(
    "
    .section .text
    .global _start
    _start:
        // la sp, _stack_top
        // mv fp, sp

        li s1, 0x10000000
        li s2, 0x48
        sb s2, 0(s1)

        // li a0, 65
        // li a6, 0
        // li a7, 1
        // ecall

        j kmain
"
);

#[no_mangle]
pub fn kmain() -> ! {
    loop {}
}

#[panic_handler]
pub fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
