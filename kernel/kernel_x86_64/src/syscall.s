.extern rust_syscall_entry

/*
 * This is the code that is run when a task executes the `syscall` instruction. The `syscall`
 * instruction:
 *    - has put the `rip` to return to in `rcx`
 *    - has put the `rflags` to return with in `r11`, masked with `IA32_FMASK`
 *    - does not save `rsp`. It is our responsibility to deal with the stack(s).
 *
 * Register summary:
 *    rax => system call result
 *    rbx - MUST BE PRESERVED
 *    rcx - users' rip
 *    rdx - b
 *    rdi - system call number
 *    rsi - a
 *    rbp - MUST BE PRESERVED
 *    r8  - d
 *    r9  - e
 *    r10 - c
 *    r11 - users' rflags
 *    r12 - MUST BE PRESERVED
 *    r13 - MUST BE PRESERVED
 *    r14 - MUST BE PRESERVED
 *    r15 - MUST BE PRESERVED
 *
 * This is only different from the Sys-V ABI in that `c` is in `r10` and not `rcx` (because `rcx` is being
 * used by syscall). To call into the Rust function (as long as it is using the C ABI), we only need to
 * move that one parameter.
 */
.global syscall_handler
syscall_handler:
    // Save the task's user rsp in the per-cpu data
    mov gs:0x10, rsp
    // Move to the task's kernel stack
    mov rsp, gs:0x8

    // We're now on the kernel stack, so interrupts are okay now
    sti

    // The `syscall` instruction puts important stuff in `rcx` and `r11`, so we save them and restore them
    // before calling `sysretq`.
    push rcx
    push r11

    // Save registers
    push rbp
    push rbx
    push rdx
    push rdi
    push rsi
    push r8
    push r9
    push r10
    push r12
    push r13
    push r14
    push r15

    // Move `c` into the right register. This is fine now because we've saved syscall's expected `rcx` on the
    // stack.
    mov rcx, r10

    // Call the Rust handler. From this point, `rax` contains the return value, so musn't be trashed!
    call rust_syscall_entry

    // Restore registers
    pop r15
    pop r14
    pop r13
    pop r12
    pop r10
    pop r9
    pop r8
    pop rsi
    pop rdi
    pop rdx
    pop rbx
    pop rbp

    // Restore state needed for `sysretq`
    pop r11
    pop rcx

    // Disable interrupts again while we mess around with the stacks
    cli

    // Save the kernel's stack back into per-cpu data
    mov gs:0x8, rsp
    // Move back to the task's user stack
    mov rsp, gs:0x10

    sysretq
