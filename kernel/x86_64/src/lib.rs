#![no_std]
#![feature(
    lang_items,
    asm,
    const_fn,
    naked_functions,
    alloc,
    type_ascription,
    allocator_api,
    panic_info_message,
    alloc_error_handler,
    core_intrinsics
)]
#![allow(unknown_lints)]
#![allow(identity_op)]
#![allow(new_without_default)]

extern crate alloc;
extern crate spin;
extern crate volatile;
extern crate x86_64;
#[macro_use]
extern crate bitflags;
extern crate bit_field;
#[macro_use]
extern crate log;
#[macro_use]
extern crate common;
extern crate acpi;
extern crate kernel;
// extern crate libmessage;
extern crate xmas_elf;

#[macro_use]
mod registers;
#[macro_use]
mod serial;
// mod acpi_handler;
mod cpu;
// mod gdt;
// mod i8259_pic;
// mod idt;
// mod interrupts;
// mod io_apic;
// mod local_apic;
// mod memory;
mod panic;
// mod pci;
// mod pit;
mod port;
// mod process;
// mod tlb;
// mod tss;

pub use panic::{_Unwind_Resume, panic, rust_eh_personality};

use x86_64::boot::BootInfo;

// use acpi_handler::PebbleAcpiHandler;
// use alloc::boxed::Box;
// use alloc::collections::BTreeMap;
// use gdt::Gdt;
// use kernel::arch::{Architecture, ModuleMapping};
// use kernel::fs::File;
// use kernel::node::Node;
// use kernel::process::ProcessMessage;
// use kernel::process::ProcessId;
// use memory::paging::PhysicalAddress;
// use memory::MemoryController;
// use pci::Pci;
// use process::{Process, ProcessImage};

// pub struct Platform {
//     memory_controller: MemoryController,
//     process_map: BTreeMap<ProcessId, Process>,
// }

// impl Architecture for Platform {
//     fn get_module_mapping(&self, module_name: &str) -> Option<ModuleMapping> {
//         self.memory_controller
//             .loaded_modules
//             .get(module_name)
//             .map(|mapping| ModuleMapping {
//                 physical_start: usize::from(mapping.start),
//                 physical_end: usize::from(mapping.end),
//                 virtual_start: mapping.ptr as usize,
//                 virtual_end: mapping.ptr as usize + mapping.size,
//             })
//     }

//     fn create_process(&mut self, id: ProcessId, image: &File) {
//         self.process_map.insert(
//             id,
//             Process::new(
//                 ProcessImage::from_elf(image, &mut self.memory_controller),
//                 &mut self.memory_controller,
//             ),
//         );
//     }

//     // fn create_process(&mut self, image: &File) -> Box<Node<MessageType = ProcessMessage>> {
//     //     Box::new(Process::new(
//     //         ProcessImage::from_elf(image, &mut self.memory_controller),
//     //         &mut self.memory_controller,
//     //     ))
//     // }
// }

#[no_mangle]
pub extern "C" fn kstart(boot_info: &BootInfo) -> ! {
    // use tss::TSS;

    serial::initialise();
    log::set_logger(&serial::SERIAL_LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("Kernel connected to COM1");

    // TODO: when we can get the memory map for the bootloader, re-add everything
    loop {}

    /*
     * We are passed the *physical* address of the Multiboot struct, so we need to translate it
     * into the higher half.
     */
    // let boot_info = unsafe { multiboot2::load(usize::from(multiboot_address.in_kernel_space())) };
    // let mut memory_controller = memory::init(&boot_info);

    // /*
    //  * We now find and parse the ACPI tables.
    //  */
    // // TODO: actually handle both types of tag for systems with ACPI Version 2.0+
    // let rsdp_tag = boot_info.rsdp_v1_tag().expect("Failed to get RSDP V1 tag");
    // // TODO: validate the RSDP tag
    // // rsdp_tag.validate().expect("Failed to validate RSDP tag");
    // let acpi_info = PebbleAcpiHandler::parse_acpi(
    //     &mut memory_controller,
    //     PhysicalAddress::new(rsdp_tag.rsdt_address()),
    //     rsdp_tag.revision(),
    // )
    // .expect("Failed to parse ACPI tables");
    // info!("ACPI info: {:#?}", acpi_info);

    // /*
    //  * We can now create and install a TSS and new GDT.
    //  *
    //  * Allocate a 4KiB stack for the double-fault handler. Using a separate stack for double-faults
    //  * avoids a triple fault happening when the guard page of the normal stack is hit (after a stack
    //  * overflow), which would otherwise:
    //  *      Page Fault -> Page Fault -> Double Fault -> Page Fault -> Triple Fault
    //  */
    // let double_fault_stack = memory_controller
    //     .alloc_stack(1)
    //     .expect("Failed to allocate stack!");
    // unsafe {
    //     TSS.interrupt_stack_table[tss::DOUBLE_FAULT_IST_INDEX] = double_fault_stack.top();
    //     TSS.set_kernel_stack(memory::get_kernel_stack_top());
    // }
    // let gdt_selectors = Gdt::install(unsafe { &mut TSS });
    // interrupts::init(&acpi_info, &gdt_selectors, &mut memory_controller);
    // interrupts::enable();

    // /*
    //  * We can now initialise the local APIC timer to interrupt every 10ms. This uses the PIT to
    //  * determine the frequency the timer is running at, so interrupts must be enabled at this point.
    //  * We also re-initialise the PIT to tick every 10ms.
    //  */
    // // unsafe {
    // //     apic::LOCAL_APIC.enable_timer(10);
    // // }
    // unsafe {
    //     pit::PIT.init(10);
    // }

    // /*
    //  * Scan for PCI devices
    //  */
    // let mut pci = unsafe { Pci::new() };
    // pci.scan();

    // /*
    //  * Finally, we pass control to the kernel.
    //  */
    // let mut platform = Platform {
    //     memory_controller,
    //     process_map: BTreeMap::new(),
    // };
    // kernel::kernel_main(&mut platform);
}

#[alloc_error_handler]
#[no_mangle]
pub extern "C" fn rust_oom(_: core::alloc::Layout) -> ! {
    // TODO: handle this better
    panic!("Kernel ran out of heap memory!");
}
