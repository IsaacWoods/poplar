; Copyright (C) 2017, Isaac Woods. 
; See LICENCE.md

section .multiboot
multiboot_header:
  dd 0xe85250d6                                                           ; Multiboot-2 magic
  dd 0                                                                    ; Architecture=0 (P-mode i386)
  dd multiboot_end - multiboot_header                                     ; Header length
  dd 0x100000000 - (0xe85250d6 + 0 + (multiboot_end - multiboot_header))  ; Checksum

  ; Insert options here

  dw 0
  dw 0
  dd 8
multiboot_end:

section .text
global start
bits 32
start:
  mov dword [0xb8000], 0x2f4b2f4f
  hlt
