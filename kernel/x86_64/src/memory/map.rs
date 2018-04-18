/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use super::paging::{VirtualAddress,Page,PAGE_SIZE};

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
 * The kernel is mapped from -2GB at 0xffffffff8000000 + its physical address. This is located at
 * [P4=511, P3=510, P2=0,P1=0] onwards. Obviously, this means we can't use the last PML4 entry for
 * recursive mapping, so we instead use the 510th. The lower part of the virtual address space
 * (0x0 - 0xffffffff7fffffff) can be used by user-mode processes and other stuff.
 */

pub const KERNEL_START_P4 : usize = 511;
pub const KERNEL_START_P3 : usize = 510;
pub const KERNEL_START_P2 : usize = 0;
pub const KERNEL_START_P1 : usize = 0;

/*
 * This is the address for addressing into the P4 table directly (through the 510th P4 entry).
 * We achieve this by recursively addressing this entry 4 times, so P4 looks like a normal memory
 * page.
 */
pub const RECURSIVE_ENTRY  : usize = 510;
pub const P4_TABLE_ADDRESS : VirtualAddress = VirtualAddress::from_page_table_offsets(RECURSIVE_ENTRY,
                                                                                      RECURSIVE_ENTRY,
                                                                                      RECURSIVE_ENTRY,
                                                                                      RECURSIVE_ENTRY,
                                                                                      0);

/* 0xffffffff80000000 */
pub const KERNEL_VMA : VirtualAddress = VirtualAddress::new(0xffff_ffff_8000_0000);

/*
 * The kernel is mapped here. We don't know how much space it'll use, so we leave plenty of space.
 */

/* 0xffffffffc0000000 */
pub const HEAP_START : VirtualAddress = VirtualAddress::new(0xffff_ffff_c000_0000);
pub const HEAP_SIZE  : usize = 200 * 1024;  // 200 KiB

/* 0xffffffffd0000000 */
pub const STACK_SPACE_TOP       : VirtualAddress = VirtualAddress::new(0xffff_ffff_d000_0000);
pub const STACK_SPACE_BOTTOM    : VirtualAddress = VirtualAddress::new(0xffff_ffff_d006_4000 - 1);

/* 0xffffffffd0064000 */

/* 0xfffffffff0000000 */
pub const TEMP_PAGE : Page = Page { number : 0xffff_ffff_f000_0000 / PAGE_SIZE };

/* 0xffffffffffffffff */
pub const KERNEL_SPACE_END : VirtualAddress = VirtualAddress::new(0xffff_ffff_ffff_ffff);
