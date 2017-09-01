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

; This identity-maps the virtual memory space to the physical one
SetupPageTables:
  ; Recursively map the 511th entry of P4 to the P4 table itself
  mov eax, p4_table
  or eax, 0b11  ; Present + Writable
  mov [p4_table+511*8], eax

  ; Map the first P4 entry to the P3 table
  mov eax, p3_table
  or eax, 0b11 ; Present + Writable
  mov [p4_table], eax

  ; Map the first P3 entry to the P2 table
  mov eax, p2_table
  or eax, 0b11  ; Present + Writable
  mov [p3_table], eax

  ; Match each P2 entry to a huge page (2MiB) (where ecx=index of P2 entry)
  mov ecx, 0
.map_p2:
  mov eax, 0x200000 ; Make the page 2MiB
  mul ecx
  or eax, 0b10000011  ; Present + Writable + Huge
  mov [p2_table + ecx * 8], eax

  inc ecx
  cmp ecx, 512
  jne .map_p2

  ret

EnablePaging:
  ; Load our P4 into CR3
  mov eax, p4_table
  mov cr3, eax

  ; Enable Physical Address Extension
  mov eax, cr4
  or eax, 1<<5
  mov cr4, eax

  ; Set the Long Mode Bit in the EFER MSR
  mov ecx, 0xC0000080
  rdmsr
  or eax, 1<<8
  wrmsr

  ; Enable paging
  mov eax, cr0
  or eax, 1<<31
  mov cr0, eax

  ret

extern InLongMode
Start:
  mov esp, stack_top
  mov edi, ebx        ; Move the pointer to the Multiboot struct into EDI

  ; Check that the multiboot magic GRUB returns is correct
  cmp eax, 0x36d76289
  je .multiboot_fine
  mov al, 'M'
  call PrintError
.multiboot_fine:

  call CheckCpuidSupported
  call CheckLongModeSupported

  call SetupPageTables
  call EnablePaging

  ; We're now technically in Long Mode, but we still can't execute 64-bit instructions, because we've been put into
  ; a 32-bit compatiblity submode. We now need to replace GRUB's crappy GDT with a proper one and far-jump to the
  ; new code segment
  lgdt [gdt64.pointer]
  jmp gdt64.kernel_code:InLongMode

  hlt

section .rodata
gdt64:
.zeroEntry: equ $-gdt64
  dq 0
.kernel_code: equ $-gdt64
  dq (1<<43)|(1<<44)|(1<<47)|(1<<53)
.pointer:
  dw $-gdt64-1
  dq gdt64

section .bss
align 4096  ; Make sure the page-tables are page aligned
p4_table:
  resb 4096
p3_table:
  resb 4096
p2_table:
  resb 4096

stack_bottom:
  resb 4096*4   ; 4 pages = 16kB
stack_top:
