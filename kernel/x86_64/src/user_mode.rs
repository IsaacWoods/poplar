/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use memory::paging::VirtualAddress;
use gdt::GdtSelectors;

pub unsafe fn enter_usermode(instruction_pointer : VirtualAddress, gdt_selectors : GdtSelectors) -> !
{
    /*
     * TODO:
     *   * Use a real user-mode stack
     *   * Make sure interrupts are enabled after we enter ring3
     */
    asm!("cli
          push r10      // Push selector for user data segment
          push rsp      // Push stack pointer TODO: user mode stack
          pushfq        // Push flags (TODO: enable interrupts again (1<<9)? or just remove cli?)
          push r11      // Push selector for user code segment
          push r12      // Push new instruction pointer
          iretq"
          :
          : "{r10}"(gdt_selectors.user_data.0),
            "{r11}"(gdt_selectors.user_code.0),
            "{r12}"(instruction_pointer)
          : // We technically don't clobber anything because this never returns
          : "intel", "volatile");
    unreachable!();
}
