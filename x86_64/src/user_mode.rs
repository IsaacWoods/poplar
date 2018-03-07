/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use memory::paging::VirtualAddress;
use gdt::GdtSelectors;

#[naked]
pub unsafe fn enter_usermode(instruction_pointer : VirtualAddress, gdt_selectors : GdtSelectors) -> !
{
    /*
     * TODO:
     *   * Use a real user-mode stack
     */
    asm!("cli
          push r10      // Push selector for user data segment
          push rsp      // Push stack pointer TODO: user mode stack
          push r11      // Push new RFLAGS
          push r12      // Push selector for user code segment
          push r13      // Push new instruction pointer

          xor rax, rax
          xor rbx, rbx
          xor rcx, rcx
          xor rdx, rdx
          xor rsi, rsi
          xor rdi, rdi
          xor r8, r8
          xor r9, r9
          xor r10, r10
          xor r11, r11
          xor r12, r12
          xor r13, r13
          xor r14, r14
          xor r15, r15

          iretq"
          :
          : "{r10}"(gdt_selectors.user_data.0),
            "{r11}"(1 << 9 | 1 << 2),   // We probably shouldn't leak flags out of kernel-space, so
                                        // we set them to the bare minimum:
                                        //     * Bit 2 must be 1
                                        //     * Enable interrupts by setting bit 9
            "{r12}"(gdt_selectors.user_code.0),
            "{r13}"(instruction_pointer)
          : // We technically don't clobber anything because this never returns
          : "intel", "volatile");
    unreachable!();
}
