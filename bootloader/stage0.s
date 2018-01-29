[ORG 0x7c00]
bits 16

; Upon calling our bootsector, the BIOS will put the drive number we're booting from in DL
start:
    cli

    ; Initalise segment registers to 0; they may start with uninitialised values
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    ; Set stack to 2000h past entry point
    mov esp, 0x9c00

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

.hang:
    cli
    hlt
    jmp .hang

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
