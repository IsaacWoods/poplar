ENTRY(start)
OUTPUT_FORMAT(elf64-x86-64)

IMAGE_START = 0x10000;

SECTIONS {
    . = IMAGE_START;

    .text : AT(ADDR(.text) - IMAGE_START) {
        *(.text .text.*)
        . = ALIGN(4K);
    }

    .rodata : AT(ADDR(.rodata) - IMAGE_START) {
        *(.rodata .rodata.*)
        . = ALIGN(4K);
    }

    .data : AT(ADDR(.data) - IMAGE_START) {
        *(.data .data.*)
        . = ALIGN(4K);
    }

    .bss : AT(ADDR(.bss) - IMAGE_START) {
        *(.bss .bss.*)
        . = ALIGN(4K);
    }
}
