use core::arch::asm;

#[inline(never)]
pub unsafe fn syscall0(number: usize) -> usize {
    let result: usize;
    unsafe {
        asm!("syscall",
            out("rax") result,
            inlateout("rdi") number => _,
            out("rcx") _,
            out("r11") _,

            // out("rdx") _,
            // out("rsi") _,
            // out("r8") _,
            // out("r9") _,
            // out("r10") _,
        );
    }
    result
}

#[inline(never)]
pub unsafe fn syscall1(number: usize, a: usize) -> usize {
    let result: usize;
    unsafe {
        asm!("syscall",
            out("rax") result,
            inlateout("rdi") number => _,
            inlateout("rsi") a => _,
            out("rcx") _,
            out("r11") _,

            // out("rdx") _,
            // out("r8") _,
            // out("r9") _,
            // out("r10") _,
        );
    }
    result
}

#[inline(never)]
pub unsafe fn syscall2(number: usize, a: usize, b: usize) -> usize {
    let result: usize;
    unsafe {
        asm!("syscall",
            out("rax") result,
            inlateout("rdi") number => _,
            inlateout("rsi") a => _,
            inlateout("rdx") b => _,
            out("rcx") _,
            out("r11") _,

            // out("r8") _,
            // out("r9") _,
            // out("r10") _,
        );
    }
    result
}

#[inline(never)]
pub unsafe fn syscall3(number: usize, a: usize, b: usize, c: usize) -> usize {
    let result: usize;
    unsafe {
        asm!("syscall",
            out("rax") result,
            inlateout("rdi") number => _,
            inlateout("rsi") a => _,
            inlateout("rdx") b => _,
            inlateout("r10") c => _,
            out("rcx") _,
            out("r11") _,

            // out("r8") _,
            // out("r9") _,
        );
    }
    result
}

#[inline(never)]
pub unsafe fn syscall4(number: usize, a: usize, b: usize, c: usize, d: usize) -> usize {
    let result: usize;
    unsafe {
        asm!("syscall",
            out("rax") result,
            inlateout("rdi") number => _,
            inlateout("rsi") a => _,
            inlateout("rdx") b => _,
            inlateout("r10") c => _,
            inlateout("r8") d => _,
            out("rcx") _,
            out("r11") _,

            // out("r9") _,
        );
    }
    result
}

#[inline(never)]
pub unsafe fn syscall5(number: usize, a: usize, b: usize, c: usize, d: usize, e: usize) -> usize {
    let result: usize;
    unsafe {
        asm!("syscall",
            out("rax") result,
            inlateout("rdi") number => _,
            inlateout("rsi") a => _,
            inlateout("rdx") b => _,
            inlateout("r10") c => _,
            inlateout("r8") d => _,
            inlateout("r9") e => _,
            out("rcx") _,
            out("r11") _,
        );
    }
    result
}
