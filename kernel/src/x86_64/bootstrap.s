; Copyright (C) 2017, Isaac Woods. 
; See LICENCE.md

section .multiboot
multiboot_header:
  dd 0xe85250d6                                                         ; Multiboot-2 magic
  dd 0                                                                  ; Architecture=0 (i386)
  dd multiboot_end - multiboot_header                                   ; Header length
  dd 0x100000000-(0xe85250d6 + 0 + (multiboot_end - multiboot_header))  ; Checksum

  ; More options go here

  dw 0
  dw 0
  dd 8
multiboot_end:

; The bootstrap is a small physically-mapped bit of code for entering Long Mode and jumping into the
; actual kernel.

; We place the kernel at -2GB because this allows compilers to use R_X86_64_32S relocations to
; address kernel space
KERNEL_VMA equ 0xFFFFFFFF80000000
extern _start
extern _end

; Constants for defining page tables
PAGE_SIZE     equ 0x1000
PAGE_PRESENT  equ 0x1
PAGE_WRITABLE equ 0x2
PAGE_USER     equ 0x4
PAGE_HUGE     equ 0x80
PAGE_NO_EXEC  equ 0x8000000000000000

section .bootstrap_data
align 4096
bootstrap_stack_bottom:
  times 4096 db 0   ; This should really be `resb`d in a BSS section, but that was effort
bootstrap_stack_top:

boot_pml4:
  dq (boot_pml3l + PAGE_PRESENT + PAGE_WRITABLE)
  times (512 - 4) dq 0
  dq (identity_pml3 + PAGE_PRESENT + PAGE_WRITABLE)
  dq (boot_pml4 + PAGE_PRESENT + PAGE_WRITABLE + PAGE_NO_EXEC)
  dq (boot_pml3h + PAGE_PRESENT + PAGE_WRITABLE)

boot_pml3l:
  dq (boot_pml2 + PAGE_PRESENT + PAGE_WRITABLE)
  dq 0
  times (512 - 2) dq 0

boot_pml3h:
  times (512 - 2) dq 0
  dq (boot_pml2 + PAGE_PRESENT + PAGE_WRITABLE)
  dq 0

boot_pml2:
  dq (0x0 + PAGE_PRESENT + PAGE_WRITABLE + PAGE_HUGE)
  times (512 - 1) dq 0

identity_pml3:
  times (512 - 5) dq 0
  dq (pmm_stack_pml2 + PAGE_PRESENT + PAGE_WRITABLE)
  dq (identity_pml2a + PAGE_PRESENT + PAGE_WRITABLE)
  dq (identity_pml2b + PAGE_PRESENT + PAGE_WRITABLE)
  dq (identity_pml2c + PAGE_PRESENT + PAGE_WRITABLE)
  dq (identity_pml2d + PAGE_PRESENT + PAGE_WRITABLE)

pmm_stack_pml2:
  times (512 - 1) dq 0
  dq (pmm_stack_pml1 + PAGE_PRESENT + PAGE_WRITABLE)

pmm_stack_pml1:
  times 512 dq 0

identity_pml2a:
  %assign pg 0
  %rep 512
    dq (pg + PAGE_PRESENT + PAGE_WRITABLE)
    %assign pg pg+PAGE_SIZE*512
  %endrep

identity_pml2b:
  %assign pg 0
  %rep 512
    dq (pg + PAGE_PRESENT + PAGE_WRITABLE)
    %assign pg pg+PAGE_SIZE*512
  %endrep

identity_pml2c:
  %assign pg 0
  %rep 512
    dq (pg + PAGE_PRESENT + PAGE_WRITABLE)
    %assign pg pg+PAGE_SIZE*512
  %endrep

identity_pml2d:
  %assign pg 0
  %rep 512
    dq (pg + PAGE_PRESENT + PAGE_WRITABLE)
    %assign pg pg+PAGE_SIZE*512
  %endrep

gdt64:
  dq 0                                ; Null selector
  dq 0x00AF98000000FFFF               ; CS
  dq 0x00CF92000000FFFF               ; DS TODO
.end:
  dq 0  ; Pad out so .pointer is 16-aligned
.pointer:
  dw .end-gdt64-1   ; Limit
  dq gdt64          ; Base

section .bootstrap
bits 32

; Prints "ERR: " followed by the ASCII character in AL. The last thing on the stack should be the
; address that called this function.
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

EnablePaging:
  ; Load the P4 pointer into CR3
  mov eax, boot_pml4
  mov cr3, eax

  ; Enable PAE
  mov eax, cr4
  or eax, 1 << 5
  mov cr4, eax

  ; Enable Long-Mode in the EFER MSR
  mov ecx, 0xC0000080
  rdmsr
  or eax, 1 << 8
  wrmsr

  ; Enable paging
  mov eax, cr0
  or eax, 1 << 31
  mov cr0, eax

  ret

global Start
Start:
  mov esp, bootstrap_stack_top
  mov edi, ebx  ; Move the pointer to the Multiboot structure into EDI

  ; Check that GRUB passed us the correct magic number
  cmp eax, 0x36d76289
  je .multiboot_fine
  mov al, 'M'
  call PrintError
.multiboot_fine:

  call CheckCpuidSupported
  call CheckLongModeSupported
  call EnablePaging

  mov dword [0xb8064], 0x2f4b2f4f

  ; We're now technically in Long-Mode, but we've been put in 32-bit compatibility submode until we
  ; install a valid GDT. We can then far-jump into the new code segment (in real Long-Mode :P).
  lgdt [gdt64.pointer]

  ; Reload segment selectors
  mov ax, 0x10
  mov ss, ax

  mov ax, 0
  mov ds, ax
  mov es, ax
  mov fs, ax
  mov gs, ax

  jmp 0x8:Trampoline

bits 64
Trampoline:
  mov rax, qword InHigherHalf
  jmp rax

section .text
bits 64
extern kmain
InHigherHalf:
  ; Reload the GDT pointer with the correct virtual address
  mov rax, [gdt64.pointer + 2]
  mov rbx, KERNEL_VMA
  add rax, rbx
  mov [gdt64.pointer + 2], rax
  mov rax, gdt64.pointer + KERNEL_VMA
  lgdt [rax]

  ; Map the kernel
  ; Calculate page number of the PHYSICAL start of the kernel
  mov rax, _start - KERNEL_VMA
  shr rax, 21 ; Divide by 0x200000

  ; Calculate page number of the PHYSICAL end of the kernel
  mov rbx, _end - KERNEL_VMA
  shr rbx, 21 ; Divide by 0x200000

  mov rcx, boot_pml2 + KERNEL_VMA ; Virtual address of the pointer into PML2
  .map_page:
    ; Calculate the address to put in the table entry
        ; rdx = physical address of kernel memory
        ; r8  = virtual address to map kernel page into
    mov rdx, rax
    shl rdx, 21 ; Multiply by 0x200000
    mov r8, rdx
    mov r9, KERNEL_VMA
    add r8, r9
    or rdx, PAGE_PRESENT + PAGE_WRITABLE + PAGE_HUGE

    ; Write the page entry and invalidate the TLB entry
    mov [rcx], rdx
    invlpg [r8]
    
    ; Increment table pointer
    add rcx, 8

    ; Terminate loop if we've mapped the whole kernel
    cmp rax, rbx
    je .mapped_kernel

    ; Increment page number and map next page
    inc rax
    jmp .map_page
  .mapped_kernel:

  ; Set up the real stack
  mov rbp, 0  ; Terminate stack-traces in the higher-half (going lower leads to a clusterfuck)
  mov rsp, stack_top

  ; Unmap the identity-map and invalidate its TLB entries
  mov qword [boot_pml4], 0x0
  invlpg [0x0]

  ; Clear RFLAGS (TODO: why do we need this?)
  push 0x0
  popf

  ; Print OKAY
  mov rax, 0x2f592f412f4b2f4f
  mov qword [0xFFFFFFFF800b8000], rax

  ; Correct the address of the Multiboot structure
  mov rcx, qword KERNEL_VMA
  add rdi, rcx

  ; Call into the kernel
  call kmain
  hlt

section .bss
align 4096
stack_bottom:
  resb 4096*4   ; 4 pages = 16kB
stack_top:
