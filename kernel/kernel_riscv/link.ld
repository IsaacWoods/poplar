OUTPUT_ARCH("riscv")
ENTRY(_start)

SECTIONS {
    . = 0x80200000;
    _kernel_start = .;

    .text : {
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

    .sdata : {
        . = ALIGN(8);
        *(.sdata .sdata.*)
    }

    .bss : {
        *(.bss .bss.*)
        . = ALIGN(4K);

        _stack_bottom = .;
        . += 4K;
        _stack_top = .;
    }

    _kernel_end = .;
}
