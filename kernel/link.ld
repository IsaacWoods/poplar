ENTRY(kentry)
OUTPUT_FORMAT(elf64-x86-64)

KERNEL_VMA = 0xffffffff80000000;

PHDRS {
    text PT_LOAD;
    rodata PT_LOAD FLAGS(4);
    data PT_LOAD;
}

SECTIONS
{
    . = KERNEL_VMA;
    _kernel_start = .;

    .text :
    {
        *(.text .text.*)
    } :text

    . = ALIGN(4K);
    .rodata :
    {
        *(.rodata .rodata.*)
    } :rodata

    .got :
    {
        *(.got)
    } :rodata

    . = ALIGN(4K);
    .data :
    {
        *(.data .data.*)
    } :data

    .bss :
    {
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(4K);
    } :data

    _kernel_end = .;

    /DISCARD/ : {
        *(.comment*)
        *(.gcc_except_table*)
        *(.eh_frame*)
        *(.note*)
    }
}
