/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use memory::paging::PAGE_SIZE;

/*
 * On kernel entry, the page tables that the bootstrap set up are still active. It maps the kernel
 * into the higher-half at 0xffffffff80000000+{kernel physical base}. In total, it maps one page
 * directory (P2) of Huge Pages, so maps 1GiB in the range 0xffffffff80000000-0xffffffffc0000000.
 *
 * --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- ---
 *
 * These are all the constants used to define the memory mapping, so we can see it all in one go
 * for when we screw it up.
 *
 * The kernel is mapped roughly from -2GB at 0xffffffff8000000 + its physical address. This is
 * located at [P4=511, P3=510, P2=0,P1=0] onwards. Obviously, this means we can't use the last PML4
 * entry for recursive mapping, so we instead use the 510th. The lower part of the virtual address
 * space (0x0 - 0xffffffff7fffffff) can be used by user-mode processes and other stuff.
 */

/* 0xffffffff80000000 */
pub const KERNEL_VMA : usize = 0xffffffff80000000;

pub const RECURSIVE_ENTRY : usize = 510;

/*
 * This is the address for addressing into the P4 table directly (through the 510th P4 entry).
 * We achieve this by recursively addressing this entry 4 times, so P4 looks like a normal memory
 * page.
 */
                                          /* sign extension */
pub const P4_TABLE_ADDRESS : usize = 0o177777_000_000_000_000_0000 + (RECURSIVE_ENTRY<<39) // P4 slot
                                                                   + (RECURSIVE_ENTRY<<30) // P3 slot
                                                                   + (RECURSIVE_ENTRY<<21) // P2 slot
                                                                   + (RECURSIVE_ENTRY<<12) // P1 slot
                                                                   + (0<<0);               // Offset

/*
 * This is where the kernel will be mapped into. We obviously don't know exactly how much memory
 * this will use.
 *
 * TODO: Can we validate the memory map by comparing _end and known areas with this memory map?
 */

/* 0xffffffff90000000 */
pub const TEMP_PAGE_A : usize = 0xffffffff90000000;
pub const TEMP_PAGE_B : usize = TEMP_PAGE_A + PAGE_SIZE;

/* 0xffffffffc0000000 */
pub const HEAP_START : usize = KERNEL_VMA + 0o000_001_000_000_0000;
pub const HEAP_SIZE  : usize = 100 * 1024;  // 100 KiB

/* 0xffffffffc0019000 */
