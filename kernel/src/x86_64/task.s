.intel_syntax noprefix
.code64

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
 * We also need to switch to the task's user stack, which is expected to be in `r13`.
 */
.global task_entry_trampoline
task_entry_trampoline:
    mov gs:0x8, rsp     // Save the task's kernel stack
    mov rsp, r13

    mov rcx, r15
    xor r15, r15
    mov r11, r14
    xor r14, r14

    sysretq

// fn do_context_switch(old_kernel_rsp: *mut VirtualAddress, new_kernel_rsp: VirtualAddress)
.global do_context_switch
do_context_switch:
    // Save old task's context
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15

    // Change kernel stacks
    mov [rdi], rsp
    mov rsp, rsi

    // Restore state of new task
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
