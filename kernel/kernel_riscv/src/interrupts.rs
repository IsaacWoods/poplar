use alloc::vec::Vec;
use core::{
    mem,
    ptr,
    sync::atomic::{AtomicPtr, Ordering},
};
use fdt::Fdt;
use hal::memory::PAddr;
use hal_riscv::hw::plic::Plic;
use poplar_util::InitGuard;

pub static PLIC: InitGuard<&'static Plic> = InitGuard::uninit();
pub static HANDLERS: InitGuard<Vec<AtomicPtr<()>>> = InitGuard::uninit();

pub fn init(fdt: &Fdt) {
    if let Some(plic_node) = fdt.find_compatible(&["riscv,plic0"]) {
        let reg = plic_node.reg().unwrap().next().unwrap();
        let address = hal_riscv::platform::kernel_map::physical_to_virtual(
            PAddr::new(reg.starting_address as usize).unwrap(),
        );
        let num_interrupts = plic_node.property("riscv,ndev").unwrap().as_usize().unwrap();
        tracing::info!("Found PLIC at {:#x} with {} interrupts", reg.starting_address as usize, num_interrupts);

        // Create a handler entry for each interrupt
        // TODO: this assumes interrupt numbers are contiguous. Is that always the case?
        let handlers = {
            let mut handlers = Vec::with_capacity(num_interrupts);
            for _ in 0..num_interrupts {
                handlers.push(AtomicPtr::new(0x0 as *mut _));
            }
            handlers
        };
        HANDLERS.initialize(handlers);

        PLIC.initialize(unsafe { &*(address.ptr() as *const Plic) });
        PLIC.get().init(num_interrupts);
        PLIC.get().set_context_threshold(1, 0);

        // TODO: gnarly shit to see if PCI interrupts work lmao
        PLIC.get().enable_interrupt(1, 32);
        PLIC.get().set_source_priority(32, 7);
        PLIC.get().enable_interrupt(1, 33);
        PLIC.get().set_source_priority(33, 7);
        PLIC.get().enable_interrupt(1, 34);
        PLIC.get().set_source_priority(34, 7);
        PLIC.get().enable_interrupt(1, 35);
        PLIC.get().set_source_priority(35, 7);
    }
}

/// Register a handler for the specified interrupt
///
/// ### Panics
/// - If the supplied interrupt number does not exist
pub fn register_handler(interrupt: usize, handler: fn()) {
    HANDLERS.get().get(interrupt).unwrap().store(handler as *mut _, Ordering::SeqCst);
}

pub fn enable_interrupt(interrupt: usize) {
    let plic = PLIC.get();
    // TODO: don't just assume all interrupts should go to the first context
    plic.enable_interrupt(1, interrupt);
    // TODO: do priorities correctly at some point
    plic.set_source_priority(interrupt, 7);
}

pub fn handle_external_interrupt() {
    let interrupt = PLIC.get().claim_interrupt(1);
    let handler = HANDLERS.get().get(interrupt as usize).expect("Received interrupt with no handler slot!");
    let ptr = handler.load(Ordering::SeqCst);
    if ptr != ptr::null_mut() {
        unsafe {
            let ptr: fn() = mem::transmute(ptr);
            (ptr)();
        }
    } else {
        info!("Unhandled interrupt {}", interrupt);
    }
    PLIC.get().complete_interrupt(1, interrupt);
}
