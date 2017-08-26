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
    *(.text)
  }
}
