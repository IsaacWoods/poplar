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
        . = ALIGN(4K);
    } :text

    .rodata :
    {
        *(.rodata .rodata.*)
        /* We don't need to align to 4K here because the rodata segment is aligned by .got below */
    } :rodata

    .got :
    {
        *(.got)
        . = ALIGN(4K);
    } :rodata

    .data :
    {
        *(.data .data.*)
        /* We don't need to align to 4K here because it's done by .bss below */
    } :data

    .bss :
    {
        *(.bss .bss.*)
        . = ALIGN(4K);

        /*
         * We reserve a guard page that should be unmapped by the bootloader. This will cause a
         * page-fault if accessed and so will detect a stack overflow.
         * TODO: maybe manually allocate a stack in the efiloader for each CPU?
         */
        _guard_page = .;
        . += 4K;
        _stack_bottom = .;
        . += 4M;
        _stack_top = .;
        /* No need to add more alignment here - it will already be page-aligned */
    } :data

    _kernel_end = .;

    /DISCARD/ : {
        *(.comment*)
        *(.gcc_except_table*)
        *(.eh_frame*)
        *(.note*)
    }
}
