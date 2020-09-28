/*
 * This is the linker script that all userland tasks should be compiled with at the moment. In the future,
 * this should not be needed, and will instead be build directly into a custom Rust target.
 *
 * This (and the JSON target file) are the exact same for every userspace process, but have to be duplicated
 * in a few places due to a path issue in the target file. Not sure how we should fix this in the future, but
 * for now this works at a pinch.
 */

ENTRY(start)
OUTPUT_FORMAT(elf64-x86-64)

IMAGE_START = 0x10000;

PHDRS {
    text PT_LOAD;
    rodata PT_LOAD;
    data PT_LOAD;
    caps PT_NOTE;
}

SECTIONS {
    . = IMAGE_START;

    .text : {
        *(.text .text.*)
        . = ALIGN(4K);
    } :text

    .rodata : {
        *(.rodata .rodata.*)
        . = ALIGN(4K);
    } :rodata

    .data : {
        *(.data .data.*)
        . = ALIGN(4K);
    } :data

    .bss : {
        *(.bss .bss.*)
        . = ALIGN(4K);
    } :data

    .caps : {
        KEEP(*(.caps))
    } :caps
}
