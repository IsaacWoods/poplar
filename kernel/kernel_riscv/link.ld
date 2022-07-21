OUTPUT_ARCH("riscv")
ENTRY(_start)

SECTIONS {
    . = 0x80200000;
    _kernel_start = .;

    .text : {
        *(.text.entry)
        *(.text .text.*)
        . = ALIGN(4K);
    }

    .rodata : {
        *(.rodata .rodata.*)
        . = ALIGN(4K);
    }

    .data : {
        *(.data .data.*)
    }

    . = ALIGN(8);

    .sdata : {
        *(.sdata .sdata.*)
        . = ALIGN(4K);
    }

    .sbss : {
        *(.sbss .sbss.*)
    }

    .bss : {
        *(.bss .bss.*)
        . = ALIGN(4K);

        _stack_bottom = .;
        . += 4K;
        _stack_top = .;
    }

    _kernel_end = .;

    /DISCARD/ : { *(.eh_frame_hdr .eh_frame) }
}
