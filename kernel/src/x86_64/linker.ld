ENTRY(BootstrapStart)

/*KERNEL_OFFSET = 0xffffff0000100000;*/
/*KERNEL_OFFSET = 0xC0100000;*/

KERNEL_LMA = 0x100000;
KERNEL_VMA = 0xC0000000;

SECTIONS
{
  /*
   * First, we physically-map a bootstrap section that will load the 64-bit code. This is required
   * because we haven't enabled paging yet, and so code that uses virtual addresses won't work.
   */
  . = KERNEL_LMA;

  .bootstrap.text :
  {
    KEEP(*(.multiboot))
    *bootstrap.o (.text .text.*)
    . = ALIGN(4K);
  }

  .bootstrap.bss :
  {
    *bootstrap.o (.bss)
    . = ALIGN(4K);
  }

  .bootstrap.rodata :
  {
    *bootstrap.o (.rodata .rodata.*)
    . = ALIGN(4K);
  }

  /*
   * Then, we place everything else at proper virtual addresses
   */
  . += KERNEL_VMA;

  .rodata : AT(ADDR(.rodata) - KERNEL_VMA)
  {
    *(.rodata .rodata.*)
    . = ALIGN(4K);
  }

  .text : AT(ADDR(.text) - KERNEL_VMA)
  {
    *(EXCLUDE_FILE(bootstrap.o) .text .text.*)
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

  /DISCARD/ :
  {
    *(.comment*)
    *(.gcc_except_table*)
    *(.eh_frame*)
    *(.note*)
  }
}
