/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

ENTRY(kentry)
OUTPUT_FORMAT(elf64-x86-64)

KERNEL_VMA = 0xffffffff80000000;

PHDRS {
    text PT_LOAD;
    rodata PT_LOAD FLAGS(4);
    data PT_LOAD;
}

SECTIONS
{
    . = KERNEL_VMA;
    _kernel_start = .;

    .text :
    {
        *(.text .text.*)
    } :text

    . = ALIGN(4K);
    .rodata :
    {
        *(.rodata .rodata.*)
    } :rodata

    .got :
    {
        *(.got)
    } :rodata

    . = ALIGN(4K);
    .data :
    {
        *(.data .data.*)
    } :data

    .bss :
    {
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(4K);

        /*
         * We reserve a guard page that should be unmapped by the bootloader. This will cause a
         * page-fault if accessed and so will detect a stack overflow.
         * TODO: maybe manually allocate a stack in seed_uefi for each CPU?
         */
        _guard_page = .;
        . += 4K;
        _stack_bottom = .;
        . += 4M;
        _stack_top = .;
    } :data

    _kernel_end = .;

    /DISCARD/ : {
        *(.comment*)
        *(.gcc_except_table*)
        *(.eh_frame*)
        *(.note*)
    }
}
