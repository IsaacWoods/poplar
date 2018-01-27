[ORG 0x7c00]
bits 16

start:
    cli

    xor ax, ax
    mov ds, ax      ; Initialise DS to 0
    mov ss, ax      ; Initialise SS to 0
    mov esp, 0x9c00  ; Set stack to 2000h past entry point

    mov si, msg
    call print

    lgdt [gdt_ptr]

    ; Move to protected mode
    mov eax, cr0
    or al, 1
    mov cr0, eax

    ; Select new GDT segment
    mov bx, 0x08
    mov ds, bx

    hlt

print:
    lodsb
    or al, al   ; Test if al is 0 - marks end of the string
    jz done
    mov ah, 0x0E
    int 0x10
    jmp print
done:
    ret

msg db 'Hello World', 13, 10, 0

gdt_ptr:
    dw gdt_end-gdt - 1  ; Limit of the GDT
    dd gdt              ; Address of the GDT
gdt:
    dd 0,0
    flatdesc db 0xff, 0xff, 0, 0, 0, 10010010b, 11001111b, 0
gdt_end:

times 510-($-$$) db 0
db 0x55
db 0xAA
