; Copyright (C) 2017, Isaac Woods
; See LICENCE.md

section .text
bits 64
global StartInLongMode
extern rust_main
StartInLongMode:
  ; Point all segments to the zero entry
  mov ax, 0
  mov ss, ax
  mov ds, ax
  mov es, ax
  mov fs, ax
  mov gs, ax

  ; Call into Rust
  call rust_main
  hlt

  ; Print 'OKAY'
;  mov rax, 0x2f592f412f4b2f4f
;  mov qword [0xb8000], rax
;  hlt
