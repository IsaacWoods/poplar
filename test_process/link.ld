ENTRY(start)
OUTPUT_FORMAT(elf64-x86-64)

IMAGE_START = 0x10000;

PHDRS {
    text PT_LOAD;
    rodata PT_LOAD;
    data PT_LOAD;
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
}
