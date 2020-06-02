#![no_std]
#![no_main]
#![feature(asm, global_asm)]

global_asm!(include_str!("start.s"));

use core::panic::PanicInfo;

extern "C" {
    static __bss_start: usize;
    static __bss_end: usize;
}

#[no_mangle]
pub fn kmain() -> ! {
    // Zero BSS
    // TODO: do this in assembly before we reach Rust
    let mut ptr = unsafe { &__bss_start as *const _ as *mut usize };
    let end = unsafe { &__bss_end as *const _ as *mut usize };

    while ptr < end {
        unsafe {
            core::ptr::write_volatile(ptr, 0x0);
            ptr = ptr.offset(1);
        }
    }

    panic!("Did stuff!");
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe {
            asm!("wfe");
        }
    }
}
