ENTRY(start)
OUTPUT_FORMAT(elf64-x86-64)

SECTIONS {
    . = 0x10000;

    .text : {
        *(.text)
        . = ALIGN(4K);
    }

    .rodata : {
        *(.rodata .rodata.*)
        . = ALIGN(4K);
    }

    .data : {
        *(.data .data.*)
        . = ALIGN(4K);
    }

    .bss : {
        *(.bss .bss.*)
        . = ALIGN(4K);
    }
}
