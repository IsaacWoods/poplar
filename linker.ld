ENTRY(Start)

SECTIONS
{
  . = 1M;

  .boot :
  {
    KEEP(*(.multiboot))
  }

  .text :
  {
    *(.text .text.*)
  }

  .rodata :
  {
    *(.rodata .rodata.*)
  }

  .data.rel.ro :
  {
    *(.data.rel.ro.local*)
    *(.data.rel.ro .data.rel.ro.*)
  }
}
