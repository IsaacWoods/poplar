; Copyright (C) 2017, Pebble Developers.
; See LICENCE.md

[BITS 64]
section .text

_start:
    mov rdi, 1
    mov rax, msg
    mov rbx, 20
    int 0x80
    mov rax, 0xDEADBEEF
.loop:
    jmp .loop

align 4096
section .rodata
msg: db "Hello from user mode"
