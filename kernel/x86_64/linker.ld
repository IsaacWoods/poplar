ENTRY(kstart)
OUTPUT_FORMAT(elf64-x86-64)

KERNEL_VMA = 0xffffffff80000000;

SECTIONS
{
    . += KERNEL_VMA;
    _higher_start = .;

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

    _end = .;

    /DISCARD/ :
    {
        *(.comment*)
        *(.gcc_except_table*)
        *(.eh_frame*)
        *(.note*)
    }
}
