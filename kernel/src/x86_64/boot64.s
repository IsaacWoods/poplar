; Copyright (C) 2017, Isaac Woods
; See LICENCE.md

section .text
bits 64
global InLongMode
extern kmain
InLongMode:
  ; Point all segments to the zero entry
  mov ax, 0
  mov ss, ax
  mov ds, ax
  mov es, ax
  mov fs, ax
  mov gs, ax

  ; Call into Rust
  call kmain
  hlt
