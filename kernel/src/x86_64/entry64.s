; Copyright (C) 2017, Isaac Woods.
; See LICENCE.md

bits 64
section .text

global Entry64
Entry64:
  ; Load a higher-half stack
  mov rsp, stack_top

  ; Reload the GDT

  ; Print OKAY
  mov rax, 0x2f592f412f4b2f4f
  mov qword [0xb8000], rax  ; TODO: use proper virtual address
  hlt

section .bss
align 4096
stack_bottom:
  resb 4096*4   ; 4 pages = 16kB
stack_top:
