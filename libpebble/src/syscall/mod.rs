cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        pub mod raw_x86_64;
        pub use raw_x86_64 as raw;
    } else {
        compile_error!("libpebble does not support this target architecture!");
    }
}

pub const SYSCALL_YIELD: usize = 0;

pub fn yield_to_kernel() {
    unsafe {
        raw::syscall0(SYSCALL_YIELD);
    }
}
