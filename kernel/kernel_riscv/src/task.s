.global task_entry_trampoline
task_entry_trampoline:
    // Clear SPP in `sstatus` - this makes `sret` return to U-mode
    li t6, 1<<8
    csrc sstatus, t6
    // Set SPIE in `sstatus` - this enables S-mode interrupts upon return to U-mode
    li t6, 1<<5
    csrs sstatus, t6

    // Load `sepc` to userspace's entry point
    csrw sepc, s0

    // Switch to the user's stack
    mv sp, s1

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
    ld s3, 40(a0)
    ld s4, 48(a0)
    ld s5, 56(a0)
    ld s6, 64(a0)
    ld s7, 72(a0)
    ld s8, 80(a0)
    ld s9, 88(a0)
    ld s10, 96(a0)
    ld s11, 104(a0)

    // TODO: load other registers with zero/known-values to avoid leaking stuff to userspace?

    // Load `sepc` to userspace's entry point
    csrw sepc, s0

    // Switch to the user's stack
    mv sp, s1

    sret

.global do_context_switch
do_context_switch:
    sd ra, 0(a0)
    sd sp, 8(a0)
    sd s0, 16(a0)
    sd s1, 24(a0)
    sd s2, 32(a0)
    sd s3, 40(a0)
    sd s4, 48(a0)
    sd s5, 56(a0)
    sd s6, 64(a0)
    sd s7, 72(a0)
    sd s8, 80(a0)
    sd s9, 88(a0)
    sd s10, 96(a0)
    sd s11, 104(a0)

    ld ra, 0(a1)
    ld sp, 8(a1)
    ld s0, 16(a1)
    ld s1, 24(a1)
    ld s2, 32(a1)
    ld s3, 40(a1)
    ld s4, 48(a1)
    ld s5, 56(a1)
    ld s6, 64(a1)
    ld s7, 72(a1)
    ld s8, 80(a1)
    ld s9, 88(a1)
    ld s10, 96(a1)
    ld s11, 104(a1)

    ret
