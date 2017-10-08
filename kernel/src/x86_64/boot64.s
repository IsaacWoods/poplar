; Copyright (C) 2017, Isaac Woods
; See LICENCE.md

section .text
bits 64

extern kmain

global InLongMode
InLongMode:
  ; Point all segments to the zero entry
  mov ax, 0
  mov ss, ax
  mov ds, ax
  mov es, ax
  mov fs, ax
  mov gs, ax

  ; Correct the address of the Multiboot structure
  add rdi, 0xC0000000

  ; Call into Rust
  call kmain
  hlt
