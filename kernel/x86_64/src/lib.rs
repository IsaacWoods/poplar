/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#![no_std]

#![feature(asm)]
#![feature(const_fn)]
#![feature(naked_functions)]
#![feature(core_intrinsics)]
#![feature(alloc)]
#![feature(use_nested_groups)]

                extern crate volatile;
                extern crate spin;
#[macro_use]    extern crate alloc;
#[macro_use]    extern crate bitflags;
                extern crate bit_field;
                extern crate hole_tracking_allocator;
#[macro_use]    extern crate util;

#[macro_use]        mod control_reg;
#[macro_use]    pub mod vga_buffer;
#[macro_use]    pub mod serial;
                    mod memory;
                    mod interrupts;
                    mod gdt;
                    mod idt;
                    mod tlb;
                    mod tss;
                    mod i8259_pic;
                    mod port;
                    mod multiboot2;
                    mod acpi;

use memory::paging::PhysicalAddress;
use acpi::AcpiInfo;

#[derive(Copy,Clone,PartialEq,Eq)]
#[repr(u8)]
pub enum PrivilegeLevel
{
    Ring0 = 0,
    Ring1 = 1,
    Ring2 = 2,
    Ring3 = 3,
}

impl From<u16> for PrivilegeLevel
{
    fn from(value : u16) -> Self
    {
        match value
        {
            0 => PrivilegeLevel::Ring0,
            1 => PrivilegeLevel::Ring1,
            2 => PrivilegeLevel::Ring2,
            3 => PrivilegeLevel::Ring3,
            _ => panic!("Invalid privilege level used!"),
        }
    }
}

pub fn init_platform<T>(multiboot_address : T)
    where T : Into<PhysicalAddress>
{
    use multiboot2::BootInformation;

    serial::initialise();
    serial_println!("Kernel connected to COM1");

    vga_buffer::clear_screen();

    /*
     * We are passed the *physical* address of the Multiboot struct, so we offset it by the virtual
     * offset of the whole kernel.
     */
    let boot_info = unsafe { BootInformation::load(multiboot_address.into()) };
    let mut memory_controller = memory::init(&boot_info);
    interrupts::init(&mut memory_controller);

    let acpi_info = AcpiInfo::new(&boot_info, &mut memory_controller);

    /*
     * If the legacy 8259 PIC is active, we now disable it.
     */
    if acpi_info.legacy_pics_active
    {
        unsafe
        {
            i8259_pic::PIC_PAIR.lock().remap();
            i8259_pic::PIC_PAIR.lock().disable();
        }
    }

    for module_tag in boot_info.modules()
    {
        println!("Running module: {}", module_tag.name());
        let virtual_address = module_tag.start_address().into_kernel_space();
        let code : unsafe extern "C" fn() -> u32 = unsafe
                                                   {
                                                       core::mem::transmute(virtual_address.ptr() as *const ())
                                                   };
        let result : u32 = unsafe { (code)() };
        println!("Result was {:#x}", result);
    }

    unsafe { asm!("sti"); }
}
