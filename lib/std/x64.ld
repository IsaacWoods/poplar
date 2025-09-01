ENTRY(_start)
OUTPUT_FORMAT(elf64-x86-64)

IMAGE_START = 0x10000;

PHDRS {
    text PT_LOAD;
    rodata PT_LOAD FLAGS(4);
    data PT_LOAD;
    tls PT_TLS;
}

SECTIONS {
    . = IMAGE_START;

    .text : {
        *(.text .text.*)
    } :text

    . = ALIGN(4K);
    .rodata : {
        *(.rodata .rodata.*)
    } :rodata

    .got : {
        *(.got)
    } :rodata

    . = ALIGN(4K);
    .data : {
        *(.data .data.*)
    } :data

    .bss : {
        *(.bss .bss.*)
        *(COMMON)
    } :data

    . = ALIGN(4K);
    .tdata : {
        *(.tdata .tdata.*)
    } :tls
    .tbss : {
        *(.tbss .tbss.*)
    } :tls

    /DISCARD/ : {
        *(.eh_frame*)
    }
}
