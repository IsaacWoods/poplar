; Copyright (C) 2017, Pebble Developers.
; See LICENCE.md

[BITS 64]
section .text

_start:
    call print_message
    call print_message
    mov rax, 0xDEADBEEF
.loop:
    jmp .loop

print_message:
    mov rdi, 1
    mov rax, msg
    mov rbx, 20
    int 0x80
    ret

align 4096
section .rodata
msg: db "Hello from user mode"
