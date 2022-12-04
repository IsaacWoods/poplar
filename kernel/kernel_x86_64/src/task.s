/*
 * Used to enter a task for the first time.
 * 
 * For sysret, we need:
 *     - `rcx` to contain the instruction pointer to enter userspace at
 *     - `r11` to contain the flags we want to enter with
 *
 * Neither of these are saved by the context switch, so we instead use registers that are:
 *     - `r15` is moved into `rcx`
 *     - `r14` is moved into `r11`
 *
 * We also need to switch to the task's user stack, which we access through the per-CPU data.
 */
.global task_entry_trampoline
task_entry_trampoline:
    // Disable interrupts while we're messing around with stacks. Re-enabled on `sysretq`.
    cli

    mov gs:0x8, rsp     // Save the task's kernel stack
    mov rsp, gs:0x10

    mov rcx, r15
    xor r15, r15
    mov r11, r14
    xor r14, r14

    // Zero all registers not zerod as part of the context load, to avoid leaking kernel data into userspace
    // XXX: leave `rcx` and `r11` alone as they're needed for `sysret`
    xor rax, rax
    xor rdx, rdx
    xor rsi, rsi
    xor rdi, rdi
    xor r8, r8
    xor r9, r9
    xor r10, r10

    sysretq

// fn do_drop_into_usermode() -> !
.global do_drop_to_usermode
do_drop_to_usermode:
    // Disable interrupts while we're messing around with stacks. Re-enabled on `sysretq`.
    cli

    // Switch to the task's kernel stack
    mov rsp, gs:0x8

    // Pop the context-saved registers. We pop a few of them into the "wrong" registers as we need to move some
    // into registers not saved in the context. See documentation of `task_entry_trampoline` for details.
    popfq
    pop rcx
    pop r11
    pop r13
    pop r12
    pop rbp
    pop rbx

    // Switch to the task's user stack
    mov gs:0x8, rsp
    mov rsp, gs:0x10

    /*
     * Zero all registers that weren't zerod as part of the context load, except rcx and r11, as they're needed by
     * `sysret`. We also zero `r14` and `r15`, which would normally be loaded from the saved context but weren't
     * because we use them to populate `r11` and `rcx` instead.
     */
    xor rax, rax
    xor rdx, rdx
    xor rsi, rsi
    xor rdi, rdi
    xor r8, r8
    xor r9, r9
    xor r10, r10
    xor r14, r14
    xor r15, r15

    // Leap of faith!
    sysretq

// fn do_context_switch(current_kernel_rsp: *mut VAddr, new_kernel_rsp: VAddr)
.global do_context_switch
do_context_switch:
    // Save current task's context
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    pushfq

    // Change kernel stacks
    mov [rdi], rsp
    mov rsp, rsi

    // Restore state of new task
    popfq
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx

    /*
     * Return, either back up through the kernel and to the syscall handler, or in the case of a task that
     * hasn't been run before, into the kernel-space usermode trampoline
     */
    ret
