ENTRY(kmain)
OUTPUT_FORMAT(elf64-x86-64)

KERNEL_VMA = 0xffffffff80000000;

PHDRS {
    text PT_LOAD;
    rodata PT_LOAD;
    data PT_LOAD;
}

SECTIONS
{
    . = KERNEL_VMA;
    _kernel_start = .;

    .text :
    {
        *(.text .text.*)
        . = ALIGN(4K);
    } :text

    .data :
    {
        *(.data .data.*)
        . = ALIGN(4K);
    } :data

    .bss :
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
        . += 64K;
        _stack_top = .;
        /* No need to add more alignment here - it will already be page-aligned */
    } :data

    .rodata :
    {
        *(.rodata .rodata.*)
        . = ALIGN(4K);
    } :rodata

    _kernel_end = .;

    /DISCARD/ :
    {
        *(.comment*)
        *(.gcc_except_table*)
        *(.eh_frame*)
        *(.note*)
    }
}
