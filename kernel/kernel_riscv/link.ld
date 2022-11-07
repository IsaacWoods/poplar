/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

OUTPUT_ARCH("riscv")
OUTPUT_FORMAT("elf64-littleriscv")
ENTRY(kentry)

KERNEL_VMA = 0xffffffff80000000;

PHDRS {
    text PT_LOAD;
    rodata PT_LOAD FLAGS(4);
    data PT_LOAD;
}

SECTIONS {
    . = KERNEL_VMA;

    .text : ALIGN(16) {
        *(.text.start)
        *(.text .text.*)
        . = ALIGN(4K);
    } :text

    .srodata : ALIGN(16) {
        *(.srodata .srodata.*)
    } :rodata

    .rodata : ALIGN(16) {
        *(.rodata .rodata.*)
        . = ALIGN(4K);
    } :rodata

    .sdata : ALIGN(16) {
        *(.sdata .sdata.*)
    } :data

    PROVIDE(_global_pointer$ = .);
    PROVIDE(_bss_start = .);

    .sbss : ALIGN(16) {
        *(.sbss .sbss.*)
    } :data

    .bss : ALIGN(16) {
        *(.bss .bss.*)
        . = ALIGN(4K);

        _guard_page = .;
        . += 4K;
        PROVIDE(_stack_bottom = .);
        . += 64K;
        _stack_top = .;
    } :data

    PROVIDE(_bss_end = .);

    .data : ALIGN(16) {
        *(.data .data.*)
        . = ALIGN(4K);
    } :data

    /DISCARD/ : { *(.eh_frame_hdr .eh_frame) }
}
