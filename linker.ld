ENTRY(start)

SECTIONS
{
  . = 1M;

  .boot :
  {
    *(.multiboot)
  }

  .text :
  {
    *(.text)
  }
}
