/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#![no_std]

#![feature(lang_items)]
#![feature(const_fn)]
#![feature(const_unique_new)]
#![feature(unique)]
#![feature(alloc)]
#![feature(asm)]
#![feature(abi_x86_interrupt)]

/*
 * `rlibc` just provides intrinsics that are linked against, and so the compiler doesn't pick up
 * that it's actually used, so we suppress the warning.
 */
#[allow(unused_extern_crates)]
                extern crate rlibc;
                extern crate volatile;
                extern crate spin;
//#[macro_use]    extern crate lazy_static;
                extern crate multiboot2;
#[macro_use]    extern crate bitflags;
                extern crate bit_field;
#[macro_use]    extern crate alloc;
#[macro_use]    extern crate rustos_common;
                extern crate hole_tracking_allocator;

#[macro_use]    mod vga_buffer;
#[macro_use]    mod serial;
#[macro_use]    mod x86_64;
                mod memory;
                mod interrupts;

use multiboot2::BootInformation;
use memory::map::KERNEL_VMA;

#[no_mangle]
pub extern fn kmain(multiboot_ptr : usize)
{
    serial::initialise();
    serial_println!("Hello, World!");

    vga_buffer::clear_screen();
    println!("Hello, World!");

    /*
     * We are passed the *physical* address of the Multiboot struct, so we offset it by the virtual
     * offset of the whole kernel.
     */
    let boot_info = unsafe { BootInformation::load(multiboot_ptr, KERNEL_VMA) };
    let mut memory_controller = memory::init(&boot_info);
    interrupts::init(&mut memory_controller);
    println!("Going to interrupt now!");
    unsafe { asm!("int $$3"); }

/*
    for module_tag in boot_info.module_tags()
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
    }

    unsafe { asm!("sti"); }*/
    println!("FINISHED ALL KERNEL SHITE");
    loop { }
}

#[lang = "eh_personality"]
extern fn eh_personality() { }

#[lang = "panic_fmt"]
#[no_mangle]
pub extern fn panic_fmt(fmt : core::fmt::Arguments, file : &'static str, line : u32) -> !
{
    println!("\n\nPANIC in {} at line {}:", file, line);
    println!("    {}", fmt);
    loop {}
}
