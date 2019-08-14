#[inline(never)]
pub unsafe fn syscall0(number: usize) -> usize {
    let result: usize;
    asm!("syscall"
    : "={rax}"(result)
    : "{rax}"(number)
    :
    : "intel"
    );
    result
}

#[inline(never)]
pub unsafe fn syscall1(number: usize, a: usize) -> usize {
    let result: usize;
    asm!("syscall"
    : "={rax}"(result)
    : "{rax}"(number), "{rdi}"(a)
    :
    : "intel"
    );
    result
}

#[inline(never)]
pub unsafe fn syscall2(number: usize, a: usize, b: usize) -> usize {
    let result: usize;
    asm!("syscall"
    : "={rax}"(result)
    : "{rax}"(number), "{rdi}"(a), "{rsi}"(b)
    :
    : "intel"
    );
    result
}

#[inline(never)]
pub unsafe fn syscall3(number: usize, a: usize, b: usize, c: usize) -> usize {
    let result: usize;
    asm!("syscall"
    : "={rax}"(result)
    : "{rax}"(number), "{rdi}"(a), "{rsi}"(b), "{rdx}"(c)
    :
    : "intel"
    );
    result
}

#[inline(never)]
pub unsafe fn syscall4(number: usize, a: usize, b: usize, c: usize, d: usize) -> usize {
    let result: usize;
    asm!("syscall"
    : "={rax}"(result)
    : "{rax}"(number), "{rdi}"(a), "{rsi}"(b), "{rdx}"(c), "{r8}"(d)
    :
    : "intel"
    );
    result
}

#[inline(never)]
pub unsafe fn syscall5(number: usize, a: usize, b: usize, c: usize, d: usize, e: usize) -> usize {
    let result: usize;
    asm!("syscall"
    : "={rax}"(result)
    : "{rax}"(number), "{rdi}"(a), "{rsi}"(b), "{rdx}"(c), "{r8}"(d), "{r9}"(e)
    :
    : "intel"
    );
    result
}
