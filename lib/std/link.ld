ENTRY(_start)
OUTPUT_FORMAT(elf64-x86-64)

IMAGE_START = 0x10000;

PHDRS {
    text PT_LOAD;
    rodata PT_LOAD FLAGS(4);
    data PT_LOAD;
    tls PT_TLS;
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
        /* No need to align, because .got is aligned below */
    } :rodata

    .got : {
        *(.got)
        . = ALIGN(4K);
    } :rodata

    .data : {
        *(.data .data.*)
        /* No need to align, because .bss is aligned below */
    } :data

    .bss : {
        *(.bss .bss.*)
        . = ALIGN(4K);
    } :data

    .tdata : {
        *(.tdata .tdata.*)
    } :tls
    .tbss : {
        *(.tbss .tbss.*)
    } :tls

    .caps : {
        KEEP(*(.caps))
    } :caps

    /DISCARD/ : {
        *(.eh_frame*)
    }
}
