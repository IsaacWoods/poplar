/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#![no_std]

#![feature(asm)]
#![feature(const_fn)]
#![feature(naked_functions)]
#![feature(core_intrinsics)]

                extern crate volatile;
                extern crate spin;
#[macro_use]    extern crate bitflags;
                extern crate bit_field;
                extern crate hole_tracking_allocator;

#[macro_use]        mod control_reg;
#[macro_use]    pub mod vga_buffer;
#[macro_use]    pub mod serial;
                    mod memory;
                    mod interrupts;
                    mod gdt;
                    mod idt;
                    mod tlb;
                    mod tss;
                    mod pic;
                    mod port;
                    mod multiboot2;

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

pub fn init_platform(multiboot_ptr : usize)
{
    use multiboot2::BootInformation;
    use memory::map::KERNEL_VMA;

    serial::initialise();
    serial_println!("Kernel connected to COM1");

    vga_buffer::clear_screen();

    /*
     * We are passed the *physical* address of the Multiboot struct, so we offset it by the virtual
     * offset of the whole kernel.
     */
    let boot_info = unsafe { BootInformation::load(multiboot_ptr, KERNEL_VMA.into()) };
    let mut memory_controller = memory::init(&boot_info);

    interrupts::init(&mut memory_controller);

/*    for module_tag in boot_info.modules()
    {
        println!("Loading and running {}", module_tag.name());
        println!("  Start address: {:#x}, End address: {:#x}", module_tag.start_address(), module_tag.end_address());
        let virtual_address = module_tag.start_address();
        let code : unsafe extern "C" fn() -> u32 = unsafe
                                                   {
                                                       core::mem::transmute(virtual_address as *const ())
                                                   };
        let result : u32 = unsafe { (code)() };
        println!("Result was {:#x}", result);
    }*/

    unsafe { asm!("sti"); }
}
