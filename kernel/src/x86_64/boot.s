; This creates some initial page tables to map the kernel, before we set real ones up from Rust
; It maps a GiB with 512 2MiB pages, starting at KERNEL_OFFSET
;extern KERNEL_OFFSET
;KERNEL_OFFSET   equ 0xffffff0000100000
;KERNEL_P4_INDEX equ ((KERNEL_OFFSET >> 27) & 0o777)
;KERNEL_P3_INDEX equ ((KERNEL_OFFSET >> 18) & 0o777)

; TODO: We probably need to identity-map the code we're currently executing (which can then be unmapped after the jump)
%if 0
SetupPageTables:
  ; Recursively map the 511th entry of the P4 to itself
  mov eax, p4_table
  or eax, 0b11  ; Present + Writable
  mov [p4_table+511*8], eax

  ; Map the correct P4 entry to the P3 table
  mov eax, p3_table
  or eax, 0b11 ; Present + Writable
  mov [p4_table+KERNEL_P4_INDEX*8], eax

  ; Map the correct P3 entry to the P2 table
  mov eax, p2_table
  or eax, 0b11  ; Present + Writable
  mov [p3_table+KERNEL_P3_INDEX*8], eax

  ; Match each entry in P2 to a huge page (2MiB) (where ecx=index of P2 entry)
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
  mov eax, (p4_table - KERNEL_OFFSET)
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

  ; Start fetching instructions in the new address space by far jumping to an absolute address
  lea ecx, [StartInHigherHalf]
  jmp ecx

global Start
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
  jmp EnablePaging

extern InLongMode
StartInHigherHalf:
  ; Load the stack again
  mov esp, stack_top

  ; We're now technically in Long Mode, but we still can't execute 64-bit instructions, because we've
  ; been put into a 32-bit compatiblity submode. We now need to replace GRUB's GDT with a proper one
  ; and far-jump to the new code segment
  lgdt [gdt64.pointer]
  jmp gdt64.kernel_code:InLongMode

  ; We should never get here
  mov al, 'R'
  call PrintError
  hlt
%endif

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
