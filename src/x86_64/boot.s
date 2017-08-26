; Copyright (C) 2017, Isaac Woods. 
; See LICENCE.md

section .multiboot
multiboot_header:
  dd 0xe85250d6                                                         ; Multiboot-2 magic
  dd 0                                                                  ; Architecture=0 (P-mode i386)
  dd multiboot_end - multiboot_header                                   ; Header length
  dd 0x100000000-(0xe85250d6 + 0 + (multiboot_end - multiboot_header))  ; Checksum

  ; More options can be inserted here

  dw 0
  dw 0
  dd 8
multiboot_end:

section .text
global Start
bits 32

; Prints "ERR: " followed by the ASCII character in AL. The last thing on the stack should be the address that called this function.
;   'M' = Incorrect Multiboot magic
;   'C' = CPUID instruction is not supported
;   'L' = Long mode not available
PrintError:
  mov dword [0xb8000], 0x4f524f45
  mov dword [0xb8004], 0x4f3a4f52
  mov dword [0xb8008], 0x4f204f20
  mov byte  [0xb800a], al
  hlt

CheckCpuidSupported:
  pushfd          ; Copy EFLAGS into EAX
  pop eax
  mov ecx, eax    ; Make a copy in ECX to compare later on
  xor eax, 1<<21  ; Flip the ID bit
  push eax        ; Copy EAX back into EFLAGS
  popfd
  pushfd          ; Read EFLAGS again (with the ID bit flipped or not)
  pop eax
  push ecx        ; Restore EFLAGS back to the old version
  popfd

  ; Compare the (potentially) flipped version to the first one
  cmp eax, ecx
  je .no_cpuid
  ret
.no_cpuid:
  mov al, 'C'
  call PrintError

CheckLongModeSupported:
  ; Test if we can access the Extended Processor Info
  mov eax, 0x80000000
  cpuid
  cmp eax, 0x80000001
  jb .no_long_mode

  ; Check the EPI to see if long mode is available on this CPU
  mov eax, 0x80000001
  cpuid
  test edx, 1<<29
  jz .no_long_mode
  ret
.no_long_mode:
  mov al, 'L'
  call PrintError

Start:
  mov esp, stack_top

  ; Check that the multiboot magic GRUB returns is correct
  cmp eax, 0x36d76289
  je .multiboot_fine
  mov al, 'M'
  call PrintError
.multiboot_fine:

  call CheckCpuidSupported
  call CheckLongModeSupported

  ; Print OK
  mov dword [0xb8000], 0x2f4b2f4f
  hlt

section .bss
stack_bottom:
  resb 64
stack_top:
