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

/*
 * XXX: Macros have to be defined before they can be used, so define them before the module defs.
 */
macro_rules! read_control_reg
{
    ($reg : ident) =>
    {
        {
            let result : u64;
            unsafe
            {
                asm!(concat!("mov %", stringify!($reg), ", $0") : "=r"(result));
            }
            result
        }
    };
}

/*
 * Because the asm! macro is not wrapped, a call to this macro will need to be inside an unsafe
 * block, which is intended because writing to control registers is probably kinda dangerous.
 */
macro_rules! write_control_reg
{
    ($reg : ident, $value : expr) =>
    {
        asm!(concat!("mov $0, %", stringify!($reg)) :: "r"($value) : "memory");
    };
}

#[macro_use]    pub mod vga_buffer;
#[macro_use]    pub mod serial;
                pub mod memory;
                    mod interrupts;
                pub mod gdt;
                pub mod idt;
                pub mod tlb;
                pub mod tss;
                pub mod pic;
                pub mod port;
                pub mod multiboot2;

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
