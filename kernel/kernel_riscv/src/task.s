.global task_entry_trampoline
task_entry_trampoline:
    // Clear SPP in `sstatus` - this makes `sret` return to U-mode
    li t6, 1<<8
    csrc sstatus, t6
    // Set SPIE in `sstatus` - this enables S-mode interrupts upon return to U-mode
    li t6, 1<<5
    csrs sstatus, t6

    // Load registers off the stack
    ld ra, 0(sp)
    // Skip `sp` til we've loaded the rest of the registers
    // TODO: for some reason I'm confused lol - work this out later

    // Load `sepc` to userspace's entry point
    csrw sepc, s0

    // 

    sret

.global do_drop_to_userspace
do_drop_to_userspace:
    // Clear SPP in `sstatus` - this makes `sret` return to U-mode
    li t6, 1<<8
    csrc sstatus, t6
    // Set SPIE in `sstatus` - this enables S-mode interrupts upon return to U-mode
    li t6, 1<<5
    csrs sstatus, t6

    // Load registers from context-switch frame
    ld ra, 0(a0)
    ld sp, 8(a0)
    ld s0, 16(a0)
    ld s1, 24(a0)
    ld s2, 32(a0)
    ld s3, 48(a0)
    ld s4, 56(a0)
    ld s5, 64(a0)
    ld s6, 72(a0)
    ld s7, 80(a0)
    ld s8, 88(a0)
    ld s9, 96(a0)
    ld s10, 104(a0)
    ld s11, 112(a0)

    // TODO: load other registers with zero/known-values to avoid leaking stuff to userspace?

    // Load `sepc` to userspace's entry point
    csrw sepc, s0

    // Switch to the user's stack
    mv sp, s1

    sret
