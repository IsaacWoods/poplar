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
