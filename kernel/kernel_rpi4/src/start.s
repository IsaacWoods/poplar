.section ".text.entry"
.global _start
_start:
    mrs x1, mpidr_el1
    and x1, x1, #3
    # TODO: can't we just jump past the jump to kmain if we're an AP instead of 2 labels?
    cbz x1, 2f
1:
    wfe
    b 1b
2:
    ldr x1, =_start
    mov sp, x1
    bl kmain
    b 1b
