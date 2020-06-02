#[inline(never)]
pub unsafe fn syscall0(number: usize) -> usize {
    let result: usize;
    asm!("syscall",
        out("rax") result,
        in("rdi") number,
    );
    result
}

#[inline(never)]
pub unsafe fn syscall1(number: usize, a: usize) -> usize {
    let result: usize;
    asm!("syscall",
        out("rax") result,
        in("rdi") number,
        in("rsi") a,
    );
    result
}

#[inline(never)]
pub unsafe fn syscall2(number: usize, a: usize, b: usize) -> usize {
    let result: usize;
    asm!("syscall",
        out("rax") result,
        in("rdi") number,
        in("rsi") a,
        in("rdx") b,
    );
    result
}

#[inline(never)]
pub unsafe fn syscall3(number: usize, a: usize, b: usize, c: usize) -> usize {
    let result: usize;
    asm!("syscall",
        out("rax") result,
        in("rdi") number,
        in("rsi") a,
        in("rdx") b,
        in("r10") c,
    );
    result
}

#[inline(never)]
pub unsafe fn syscall4(number: usize, a: usize, b: usize, c: usize, d: usize) -> usize {
    let result: usize;
    asm!("syscall",
        out("rax") result,
        in("rdi") number,
        in("rsi") a,
        in("rdx") b,
        in("r10") c,
        in("r8") d,
    );
    result
}

#[inline(never)]
pub unsafe fn syscall5(number: usize, a: usize, b: usize, c: usize, d: usize, e: usize) -> usize {
    let result: usize;
    asm!("syscall",
        out("rax") result,
        in("rdi") number,
        in("rsi") a,
        in("rdx") b,
        in("r10") c,
        in("r8") d,
        in("r9") e,
    );
    result
}
