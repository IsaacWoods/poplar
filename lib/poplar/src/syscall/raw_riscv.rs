use core::arch::asm;

pub unsafe fn syscall0(number: usize) -> usize {
    let result: usize;
    unsafe {
        asm!("ecall", inlateout("a0") number => result);
    }
    result
}

pub unsafe fn syscall1(number: usize, a: usize) -> usize {
    let result: usize;
    unsafe {
        asm!("ecall", inlateout("a0") number => result, in("a1") a);
    }
    result
}

pub unsafe fn syscall2(number: usize, a: usize, b: usize) -> usize {
    let result: usize;
    unsafe {
        asm!("ecall", inlateout("a0") number => result, in("a1") a, in("a2") b);
    }
    result
}

pub unsafe fn syscall3(number: usize, a: usize, b: usize, c: usize) -> usize {
    let result: usize;
    unsafe {
        asm!("ecall", inlateout("a0") number => result, in("a1") a, in("a2") b, in("a3") c);
    }
    result
}

pub unsafe fn syscall4(number: usize, a: usize, b: usize, c: usize, d: usize) -> usize {
    let result: usize;
    unsafe {
        asm!("ecall", inlateout("a0") number => result, in("a1") a, in("a2") b, in("a3") c, in("a4") d);
    }
    result
}

pub unsafe fn syscall5(number: usize, a: usize, b: usize, c: usize, d: usize, e: usize) -> usize {
    let result: usize;
    unsafe {
        asm!("ecall", inlateout("a0") number => result, in("a1") a, in("a2") b, in("a3") c, in("a4") d, in("a5") e);
    }
    result
}
