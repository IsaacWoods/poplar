ENTRY(kstart)
OUTPUT_FORMAT(elf64-x86-64)

KERNEL_VMA = 0xffffffff80000000;

SECTIONS
{
    . += KERNEL_VMA;
    _kernel_start = .;

    .rodata : AT(ADDR(.rodata) - KERNEL_VMA)
    {
        *(.rodata .rodata.*)
        . = ALIGN(4K);
    }

    .text : AT(ADDR(.text) - KERNEL_VMA)
    {
        *(.text .text.*)
        . = ALIGN(4K);
    }

    .data : AT(ADDR(.data) - KERNEL_VMA)
    {
        *(.data .data.*)
        . = ALIGN(4K);
    }

    .bss : AT(ADDR(.bss) - KERNEL_VMA)
    {
        *(.bss .bss.*)
        . = ALIGN(4K);

        /*
         * We reserve a guard page that should be unmapped by the bootloader. This will cause a
         * page-fault if accessed and so will detect a stack overflow.
         */
        _guard_page = .;
        . += 4K;
        _stack_bottom = .;
        . += 16K;
        _stack_top = .;
        /* No need to add more alignment here - it will already be page-aligned */
    }

    .got : AT(ADDR(.got) - KERNEL_VMA)
    {
        *(.got)
        . = ALIGN(4K);
    }

    .got.plt : AT(ADDR(.got.plt) - KERNEL_VMA)
    {
        *(.got.plt)
        . = ALIGN(4K);
    }

    .data.rel.ro : AT(ADDR(.data.rel.ro) - KERNEL_VMA) ALIGN(4K)
    {
        *(.data.rel.ro.local*)
        *(.data.rel.ro .data.rel.ro.*)
        . = ALIGN(4K);
    }

    _kernel_end = .;

    /DISCARD/ :
    {
        *(.comment*)
        *(.gcc_except_table*)
        *(.eh_frame*)
        *(.note*)
    }
}
