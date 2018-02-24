/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#![no_std]

#![feature(lang_items)]
#![feature(asm)]
#![feature(const_fn)]
#![feature(naked_functions)]
#![feature(core_intrinsics)]
#![feature(alloc)]
#![feature(use_nested_groups)]

/*
 * `rlibc` just provides intrinsics that are linked against, and so the compiler doesn't pick up
 * that it's actually used, so we suppress the warning.
 */
#[allow(unused_extern_crates)] extern crate rlibc;

                extern crate volatile;
                extern crate spin;
#[macro_use]    extern crate alloc;
#[macro_use]    extern crate bitflags;
                extern crate bit_field;
                extern crate hole_tracking_allocator;
#[macro_use]    extern crate log;
#[macro_use]    extern crate arch;
                extern crate kernel;

#[macro_use]    mod control_reg;
#[macro_use]    mod vga_buffer;
#[macro_use]    mod serial;
                mod panic;
                mod memory;
                mod interrupts;
                mod gdt;
                mod idt;
                mod tlb;
                mod tss;
                mod i8259_pic;
                mod apic;
                mod port;
                mod multiboot2;
                mod acpi;

pub use panic::panic_fmt;

use memory::paging::PhysicalAddress;
use acpi::AcpiInfo;
use arch::Architecture;

struct X86_64
{
}

impl Architecture for X86_64
{
    type MemoryAddress = memory::paging::VirtualAddress;

    fn clear_screen(&self)
    {
        vga_buffer::WRITER.lock().clear_buffer();
    }
}

#[no_mangle]
pub extern fn kstart(multiboot_address : PhysicalAddress)
{
    use multiboot2::BootInformation;

    serial::initialise();
    log::set_logger(&serial::SERIAL_LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("Kernel connected to COM1");

    /*
     * We are passed the *physical* address of the Multiboot struct, so we offset it by the virtual
     * offset of the whole kernel.
     */
    let boot_info = unsafe { BootInformation::load(multiboot_address.into()) };
    let mut memory_controller = memory::init(&boot_info);

    /*
     * We want to use the APIC, so we remap and disable the legacy PIC.
     * XXX: We do this regardless of whether ACPI tells us we need to.
     */
    unsafe
    {
        let mut legacy_pic = i8259_pic::PIC_PAIR.lock();
        legacy_pic.remap();
        legacy_pic.disable();
    }

    /*
     * We write 0 to CR8 (the Task Priority Register) to say that we want to recieve all
     * interrupts.
     */
    unsafe { write_control_reg!(cr8, 0u64); }

    /*
     * We now find and parse the ACPI tables. This also initialises the local APIC and IOAPIC, as
     * they are detailed by the MADT.
     */
    let acpi_info = AcpiInfo::new(&boot_info, &mut memory_controller);
    interrupts::init(&mut memory_controller);
    apic::LOCAL_APIC.lock().enable_timer(6);

    info!("Framebuffer: {:#?}", boot_info.framebuffer_info());

    for module_tag in boot_info.modules()
    {
        info!("Running module: {}", module_tag.name());
        let virtual_address = module_tag.start_address().into_kernel_space();
        let code : unsafe extern "C" fn() -> u32 = unsafe
                                                   {
                                                       core::mem::transmute(virtual_address.ptr() as *const ())
                                                   };
        let result : u32 = unsafe { (code)() };
        info!("Result was {:#x}", result);
    }

    unsafe { asm!("sti"); }

    /*
     * Pass control to the kernel proper.
     */
    let arch = X86_64 { };
    kernel::kernel_main(arch);
}

#[lang = "eh_personality"]
extern fn eh_personality()
{
}
