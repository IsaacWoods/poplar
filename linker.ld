ENTRY(Start)

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
