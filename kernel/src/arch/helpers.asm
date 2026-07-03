/// Load a new GDT.
///    rdi = virtual address of the GDT pointer structure
load_gdt:
    lgdt [rdi]

    // Load new kernel data selectors
    xor eax, eax
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    // For completeness, make sure we're not using an LDT
    lldt ax

    // Far-jump into the new code segment
    lea rdi, [rip + 1f]
    push 8      // This is derived from our GDT layout
    push rdi
    retfq
1:
    
    mov ax, 0x30    // Derived from our GDT layout
    ltr ax

    ret
