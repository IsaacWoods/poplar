#![no_std]
#![feature(lang_items)]
#![feature(asm)]
#![feature(const_fn)]
#![feature(naked_functions)]
#![feature(core_intrinsics)]
#![feature(alloc)]
#![feature(type_ascription)]
#![feature(allocator_api)]
#![feature(panic_implementation)]
#![feature(panic_info_message)]
#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]
#![allow(identity_op)]
#![allow(new_without_default)]

extern crate alloc;
extern crate spin;
extern crate volatile;
#[macro_use]
extern crate bitflags;
extern crate bit_field;
#[macro_use]
extern crate log;
#[macro_use]
extern crate common;
#[macro_use]
extern crate kernel;
extern crate libmessage;
extern crate xmas_elf;
extern crate acpi;

mod multiboot2;
#[macro_use]
mod registers;
#[macro_use]
mod serial;
mod apic;
mod cpu;
mod gdt;
mod i8259_pic;
mod idt;
mod interrupts;
mod memory;
mod panic;
mod pit;
mod port;
mod process;
mod tlb;
mod tss;

pub use panic::{_Unwind_Resume, panic, rust_eh_personality};

use acpi::AcpiInfo;
use alloc::boxed::Box;
use gdt::{Gdt, GdtSelectors};
use kernel::arch::{Architecture, MemoryAddress, ModuleMapping};
use kernel::node::Node;
use kernel::process::ProcessMessage;
use memory::paging::PhysicalAddress;
use memory::MemoryController;
use process::{Process, ProcessImage};
use tss::Tss;

pub static mut PLATFORM: Platform = Platform::placeholder();

pub struct Platform {
    pub memory_controller: Option<MemoryController>,
    pub gdt_selectors: Option<GdtSelectors>,
    pub tss: Tss,
}

impl Platform {
    const fn placeholder() -> Platform {
        Platform {
            memory_controller: None,
            gdt_selectors: None,
            tss: Tss::new(),
        }
    }
}

impl Architecture for Platform {
    fn get_module_mapping(&self, module_name: &str) -> Option<ModuleMapping> {
        self.memory_controller
            .as_ref()
            .unwrap()
            .loaded_modules
            .get(module_name)
            .map(|mapping| ModuleMapping {
                physical_start: usize::from(mapping.start),
                physical_end: usize::from(mapping.end),
                virtual_start: mapping.ptr as usize,
                virtual_end: mapping.ptr as usize + mapping.size,
            })
    }

    fn create_process(
        &mut self,
        image_start: MemoryAddress,
        image_end: MemoryAddress,
    ) -> Box<Node<MessageType = ProcessMessage>> {
        Box::new(Process::new(
            ProcessImage::from_elf(
                PhysicalAddress::new(image_start),
                PhysicalAddress::new(image_end),
                self.memory_controller.as_mut().unwrap(),
            ),
            &mut self.memory_controller.as_mut().unwrap(),
        ))
    }
}

#[no_mangle]
pub extern "C" fn kstart(multiboot_address: PhysicalAddress) -> ! {
    use multiboot2::BootInformation;

    serial::initialise();
    log::set_logger(&serial::SERIAL_LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("Kernel connected to COM1");

    /*
     * We are passed the *physical* address of the Multiboot struct, so we offset it by the virtual
     * offset of the whole kernel.
     */
    let boot_info = unsafe { BootInformation::load(multiboot_address) };
    unsafe {
        PLATFORM.memory_controller = Some(memory::init(&boot_info));
    }

    /*
     * We can now create and install a TSS and new GDT.
     *
     * Allocate a 4KiB stack for the double-fault handler. Using a separate stack for double-faults
     * avoids a triple fault happening when the guard page of the normal stack is hit (after a stack
     * overflow), which would otherwise:
     *      Page Fault -> Page Fault -> Double Fault -> Page Fault -> Triple Fault
     */
    let double_fault_stack = unsafe {
        PLATFORM
            .memory_controller
            .as_mut()
            .unwrap()
            .alloc_stack(1)
            .expect("Failed to allocate stack")
    };
    unsafe {
        PLATFORM.tss.interrupt_stack_table[tss::DOUBLE_FAULT_IST_INDEX] = double_fault_stack.top();
        PLATFORM
            .tss
            .set_kernel_stack(memory::get_kernel_stack_top());
    }
    let gdt_selectors = Gdt::install(unsafe { &mut PLATFORM.tss });
    interrupts::init(&gdt_selectors);
    unsafe {
        PLATFORM.gdt_selectors = Some(gdt_selectors);
    }

    /*
     * We now find and parse the ACPI tables. This also initialises the local APIC and IOAPIC, as
     * they are described by the MADT. We then enable interrupts.
     */
    let rsdp_tag = boot_info.rsdp_v1_tag().expect("Failed to get RSDP V1 tag");
    rsdp_tag.validate().expect("Failed to validate RSDP tag");
    // let acpi_info = AcpiInfo::new(&boot_info, unsafe { PLATFORM.memory_controller.as_mut().unwrap() }).expect("Failed to parse ACPI tables");
    // interrupts::enable();

    // info!("BSP: {:?}", acpi_info.bootstrap_cpu);
    // for cpu in acpi_info.application_cpus
    // {
    //     info!("AP: {:?}", cpu);
    // }

    /*
     * We can now initialise the local APIC timer to interrupt every 10ms. This uses the PIT to
     * determine the frequency the timer is running at, so interrupts must be enabled at this point.
     * We also re-initialise the PIT to tick every 10ms.
     */
    // unsafe {
    //     apic::LOCAL_APIC.enable_timer(10);
    // }
    // unsafe {
    //     pit::PIT.init(10);
    // }

    /*
     * Finally, we can pass control to the kernel.
     */
    kernel::kernel_main(unsafe { &mut PLATFORM });
}

#[lang = "oom"]
#[no_mangle]
pub extern "C" fn rust_oom() -> ! {
    panic!("Kernel ran out of heap memory!");
}
