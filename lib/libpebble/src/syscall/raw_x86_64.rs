#[inline(never)]
pub unsafe fn syscall0(number: usize) -> usize {
    let result: usize;
    llvm_asm!("syscall"
    : "={rax}"(result)
    : "{rdi}"(number)
    :
    : "intel"
    );
    result
}

#[inline(never)]
pub unsafe fn syscall1(number: usize, a: usize) -> usize {
    let result: usize;
    llvm_asm!("syscall"
    : "={rax}"(result)
    : "{rdi}"(number), "{rsi}"(a)
    :
    : "intel"
    );
    result
}

#[inline(never)]
pub unsafe fn syscall2(number: usize, a: usize, b: usize) -> usize {
    let result: usize;
    llvm_asm!("syscall"
    : "={rax}"(result)
    : "{rdi}"(number), "{rsi}"(a), "{rdx}"(b)
    :
    : "intel"
    );
    result
}

#[inline(never)]
pub unsafe fn syscall3(number: usize, a: usize, b: usize, c: usize) -> usize {
    let result: usize;
    llvm_asm!("syscall"
    : "={rax}"(result)
    : "{rdi}"(number), "{rsi}"(a), "{rdx}"(b), "{r10}"(c)
    :
    : "intel"
    );
    result
}

#[inline(never)]
pub unsafe fn syscall4(number: usize, a: usize, b: usize, c: usize, d: usize) -> usize {
    let result: usize;
    llvm_asm!("syscall"
    : "={rax}"(result)
    : "{rdi}"(number), "{rsi}"(a), "{rdx}"(b), "{r10}"(c), "{r8}"(d)
    :
    : "intel"
    );
    result
}

#[inline(never)]
pub unsafe fn syscall5(number: usize, a: usize, b: usize, c: usize, d: usize, e: usize) -> usize {
    let result: usize;
    llvm_asm!("syscall"
    : "={rax}"(result)
    : "{rdi}"(number), "{rsi}"(a), "{rdx}"(b), "{r10}"(c), "{r8}"(d), "{r9}"(e)
    :
    : "intel"
    );
    result
}
