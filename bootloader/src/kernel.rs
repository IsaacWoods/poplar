use crate::{
    memory::{BootFrameAllocator, MemoryType},
    uefi::Status,
};
use log::trace;
use x86_64::{
    hw::registers::{read_control_reg, read_msr, write_control_reg, write_msr},
    memory::{Mapper, Page, PageTable, Size4KiB, VirtualAddress},
};

/// Describes the loaded kernel image, including its entry point and where it expects the stack to
/// be.
pub struct KernelInfo {
    pub entry_point: VirtualAddress,
    pub stack_top: VirtualAddress,
}

pub fn jump_into_kernel(page_table: PageTable, info: KernelInfo) -> ! {
    setup_for_kernel();

    unsafe {
        /*
         * Switch to the kernel's page tables.
         */
        page_table.switch_to();

        /*
         * Because we change the stack pointer, we need to pre-load the kernel entry point into a
         * register, as local variables will no longer be available. We also disable interrupts
         * until the kernel has a chance to install its own IDT and configure the
         * interrupt controller.
         */
        trace!("Jumping into kernel\n\n");
        asm!("cli
          mov rsp, rax
          jmp rbx"
             :
             : "{rax}"(info.stack_top), "{rbx}"(info.entry_point)
             : "rax", "rbx", "rsp"
             : "intel"
        );
        unreachable!();
    }
}

/// Set up a common kernel environment. Some of this stuff will already be true for everything we'll
/// successfully boot on realistically, but it doesn't hurt to explicitly set it up.
fn setup_for_kernel() {
    let mut cr4 = read_control_reg!(CR4);
    cr4 |= 1 << 7; // Enable global pages
    cr4 |= 1 << 5; // Enable PAE
    cr4 |= 1 << 2; // Only allow use of the RDTSC instruction in ring 0
    unsafe {
        write_control_reg!(CR4, cr4);
    }

    let mut efer = read_msr(x86_64::hw::registers::EFER);
    efer |= 1 << 0; // Enable the syscall and sysret instructions
    efer |= 1 << 8; // Enable long mode
    efer |= 1 << 11; // Enable use of the NX bit in the page tables
    unsafe {
        write_msr(x86_64::hw::registers::EFER, efer);
    }

    /*
     * Until the kernel has a chance to install its own IDT, disable interrupts.
     */
    unsafe {
        asm!("cli");
    }
}

pub fn load_kernel(
    kernel_path: &str,
    mapper: &mut Mapper,
    allocator: &BootFrameAllocator,
) -> Result<KernelInfo, Status> {
    /*
     * Load the kernel ELF and map it into the page tables.
     */
    let file_data = crate::uefi::protocols::read_file(kernel_path, crate::uefi::image_handle())?;
    let elf = crate::elf::load_image(
        kernel_path,
        &file_data,
        MemoryType::PebbleKernelMemory,
        mapper,
        allocator,
        false,
    )?;

    /*
     * We now set up the kernel stack. As part of the `.bss` section, it has already had memory
     * allocated for it, and has been mapped into the page tables. However, we need to go back
     * and unmap the guard page, and extract the address of the top of the stack.
     */
    let guard_page_address =
        match elf.symbols().find(|symbol| symbol.name(&elf) == Some("_guard_page")) {
            Some(symbol) => VirtualAddress::new(symbol.value as usize).unwrap(),
            None => panic!("Kernel does not have a '_guard_page' symbol!"),
        };
    assert!(guard_page_address.is_page_aligned::<Size4KiB>());
    trace!("Unmapping guard page");
    mapper.unmap(Page::contains(guard_page_address));

    let stack_top = match elf.symbols().find(|symbol| symbol.name(&elf) == Some("_stack_top")) {
        Some(symbol) => VirtualAddress::new(symbol.value as usize).unwrap(),
        None => panic!("Kernel does not have a '_stack_top' symbol"),
    };
    assert!(stack_top.is_page_aligned::<Size4KiB>(), "Stack is not page aligned");

    Ok(KernelInfo { entry_point: VirtualAddress::new(elf.entry_point()).unwrap(), stack_top })
}
