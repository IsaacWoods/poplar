use fdt::Fdt;
use hal::memory::PAddr;
use hal_riscv::hw::plic::Plic;
use poplar_util::InitGuard;

pub static PLIC: InitGuard<&'static Plic> = InitGuard::uninit();

pub static UART_PRODUCER: InitGuard<kernel::tasklets::queue::QueueProducer> = InitGuard::uninit();

pub fn init(fdt: &Fdt) {
    if let Some(plic_node) = fdt.find_compatible(&["riscv,plic0"]) {
        let reg = plic_node.reg().unwrap().next().unwrap();
        let address = hal_riscv::platform::kernel_map::physical_to_virtual(
            PAddr::new(reg.starting_address as usize).unwrap(),
        );
        let num_interrupts = plic_node.property("riscv,ndev").unwrap().as_usize().unwrap();
        tracing::info!("Found PLIC at {:#x} with {} interrupts", reg.starting_address as usize, num_interrupts);

        PLIC.initialize(unsafe { &*(address.ptr() as *const Plic) });
        PLIC.get().init(num_interrupts);
        // TODO: do this better
        PLIC.get().enable_interrupt(1, 0xa);
        PLIC.get().set_context_threshold(1, 0);
        PLIC.get().set_source_priority(0xa, 7);
    }
}

pub fn handle_external_interrupt() {
    let interrupt = PLIC.get().claim_interrupt(1);
    // TODO: better way of registering and dispatching ISRs
    match interrupt {
        0xa => {
            // It's the UART
            let serial = crate::logger::SERIAL.get();
            while let Some(byte) = serial.read() {
                if let Some(producer) = UART_PRODUCER.try_get() {
                    // TODO: with more stuff running and higher baud we might end up with multiple
                    // chars - would be more efficient to use a bigger grant.
                    let mut write = producer.grant_sync(1).unwrap();
                    write[0] = byte;
                    write.commit(1);
                }
            }
        }
        _ => (),
    }
    PLIC.get().complete_interrupt(1, interrupt);
}
