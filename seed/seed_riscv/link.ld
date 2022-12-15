/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

OUTPUT_ARCH("riscv")
OUTPUT_FORMAT("elf64-littleriscv")
ENTRY(_start)

SECTIONS {
    . = 0x80200000;
    PROVIDE(_seed_start = .);

    .text : ALIGN(16) {
        *(.text.start)
        *(.text .text.*)
    }

    .srodata : ALIGN(16) {
        *(.srodata .srodata.*)
    }

    .sdata : ALIGN(16) {
        *(.sdata .sdata.*)
    }

    PROVIDE(_global_pointer$ = .);
    PROVIDE(_bss_start = .);

    .sbss : ALIGN(16) {
        *(.sbss .sbss.*)
    }

    .bss : ALIGN(16) {
        *(.bss .bss.*)

        PROVIDE(_stack_bottom = .);
        . += 256K;
        PROVIDE(_stack_top = .);
    }

    PROVIDE(_bss_end = .);

    .data : ALIGN(16) {
        *(.data .data.*)
    }

    .rodata : ALIGN(16) {
        *(.rodata .rodata.*)
    }

    .eh_frame : ALIGN(16) {
        *(.eh_frame)
    }
    PROVIDE(_seed_end = .);

    /DISCARD/ : { *(.eh_frame_hdr .eh_frame) }
}
