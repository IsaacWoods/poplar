[ORG 0x7c00]
bits 16

; Initialise DS to 0
xor ax, ax
mov ds, ax

mov si, msg
call print

hang:
    jmp hang

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

times 510-($-$$) db 0
db 0x55
db 0xAA
