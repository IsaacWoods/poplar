#![no_std]
#![no_main]
#![feature(pointer_is_aligned)]

/*
 * This is the entry-point jumped to from OpenSBI. It needs to be at the very start of the ELF, so we put it in its
 * own section and then place it manually during linking. On entry, `a0` contains the current HART's ID, and `a1`
 * contains the address of the FDT - these match up with the ABI so we can pass these straight as parameters to
 * `kmain`.
 */
core::arch::global_asm!(
    "
    .section .text.entry
    .global _start
    _start:
        la sp, _stack_top
        mv fp, sp

        j kmain
"
);

#[no_mangle]
pub fn kmain(hart_id: usize, fdt: *const ()) -> ! {
    assert!(fdt.is_aligned_to(8));

    let uart = unsafe { &mut *(0x10000000 as *mut hal_riscv::hw::uart16550::Uart16550) };
    use core::fmt::Write;
    writeln!(uart, "Hello, World!").unwrap();
    writeln!(uart, "HART ID: {}", hart_id).unwrap();
    writeln!(uart, "FDT address: {:?}", fdt).unwrap();
    loop {}
}

#[panic_handler]
pub fn panic(_info: &core::panic::PanicInfo) -> ! {
    let uart = unsafe { &mut *(0x10000000 as *mut hal_riscv::hw::uart16550::Uart16550) };
    use core::fmt::Write;
    write!(uart, "Panic :(").unwrap();
    loop {}
}
