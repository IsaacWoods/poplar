/*
 * TEMP:
 * This shouldn't be needed, but compiler_builtins is currently emitting the __rust_probestack symbol
 * incorrectly, so including it ourselves for now fixes the issue. This should be removed when compiler_builtins
 * is fixed.
 */
// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern "C" {
    pub fn __rust_probestack();
}

global_asm!(
    "
    .globl __rust_probestack
    __rust_probestack:
        .cfi_startproc
        pushq  %rbp
        .cfi_adjust_cfa_offset 8
        .cfi_offset %rbp, -16
        movq   %rsp, %rbp
        .cfi_def_cfa_register %rbp
        mov    %rax,%r11        // duplicate %rax as we're clobbering %r11
        // Main loop, taken in one page increments. We're decrementing rsp by
        // a page each time until there's less than a page remaining. We're
        // guaranteed that this function isn't called unless there's more than a
        // page needed.
        //
        // Note that we're also testing against `8(%rsp)` to account for the 8
        // bytes pushed on the stack orginally with our return address. Using
        // `8(%rsp)` simulates us testing the stack pointer in the caller's
        // context.
        // It's usually called when %rax >= 0x1000, but that's not always true.
        // Dynamic stack allocation, which is needed to implement unsized
        // rvalues, triggers stackprobe even if %rax < 0x1000.
        // Thus we have to check %r11 first to avoid segfault.
        cmp    $0x1000,%r11
        jna    3f
    2:
        sub    $0x1000,%rsp
        test   %rsp,8(%rsp)
        sub    $0x1000,%r11
        cmp    $0x1000,%r11
        ja     2b
    3:
        // Finish up the last remaining stack space requested, getting the last
        // bits out of r11
        sub    %r11,%rsp
        test   %rsp,8(%rsp)
        // Restore the stack pointer to what it previously was when entering
        // this function. The caller will readjust the stack pointer after we
        // return.
        add    %rax,%rsp
        leave
        .cfi_def_cfa_register %rsp
        .cfi_adjust_cfa_offset -8
        ret
        .cfi_endproc
"
);
