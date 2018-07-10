use core::panic::PanicInfo;
use cpu;

#[lang = "eh_personality"]
#[no_mangle]
pub extern "C" fn rust_eh_personality() {}

#[panic_implementation]
#[no_mangle]
pub extern "C" fn panic(info: &PanicInfo) -> ! {
    error!("KERNEL PANIC: {}", info);

    loop {
        unsafe {
            cpu::halt();
        }
    }
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn _Unwind_Resume() {
    loop {
        unsafe {
            cpu::halt();
        }
    }
}
