use alloc::collections::BTreeMap;
use bit_field::BitField;
use core::{mem, ptr};
use fdt::{node::FdtNode, Fdt};
use hal::memory::PAddr;
use hal_riscv::hw::{
    aplic::{AplicDomain, SourceMode},
    imsic::Imsic,
    plic::Plic,
};
use mulch::InitGuard;
use spinning_top::Spinlock;
use tracing::{info, warn};

pub static INTERRUPT_CONTROLLER: InitGuard<InterruptController> = InitGuard::uninit();

pub fn init(fdt: &Fdt) {
    if let Some(plic_node) = fdt.find_compatible(&["riscv,plic0", "allwinner,sun20i-d1-plic"]) {
        InterruptController::init_plic(plic_node);
    } else if fdt.find_compatible(&["riscv,aplic"]).is_some() {
        InterruptController::init_aia(fdt);
    } else {
        panic!("No supported interrupt controller found!");
    }
}

pub struct InterruptHandler(pub *const ());
unsafe impl Send for InterruptHandler {}

impl InterruptHandler {
    pub unsafe fn call(&self, number: u16) {
        assert!(self.0 != ptr::null());
        unsafe {
            let ptr: fn(u16) = mem::transmute(self.0);
            (ptr)(number);
        }
    }
}

pub enum InterruptController {
    Plic {
        plic: &'static Plic,
        // TODO: wrap in a guard to disable interrupts
        handlers: Spinlock<BTreeMap<usize, InterruptHandler>>,
    },
    Aia {
        aplic: &'static AplicDomain,
        // TODO: wrap in a guard to disable interrupts
        handlers: Spinlock<BTreeMap<usize, InterruptHandler>>,
    },
}

impl InterruptController {
    pub fn init_plic(plic_node: FdtNode<'_, '_>) {
        let reg = plic_node.reg().unwrap().next().unwrap();
        let address = hal_riscv::platform::kernel_map::physical_to_virtual(
            PAddr::new(reg.starting_address as usize).unwrap(),
        );
        let num_interrupts = plic_node.property("riscv,ndev").unwrap().as_usize().unwrap();
        tracing::info!("Found PLIC at {:#x} with {} interrupts", reg.starting_address as usize, num_interrupts);

        let plic = unsafe { &*(address.ptr() as *const Plic) };
        plic.init(num_interrupts);
        plic.set_context_threshold(1, 0);

        INTERRUPT_CONTROLLER
            .initialize(InterruptController::Plic { plic, handlers: Spinlock::new(BTreeMap::new()) });
    }

    pub fn init_aia(fdt: &Fdt) {
        /*
         * This gets the physical address of the area of memory used to trigger messages on the
         * S-mode IMSIC.
         */
        let imsic_area = {
            // TODO: same problem as below re multiple entries
            let node = fdt.find_compatible(&["riscv,imsics"]).unwrap();
            PAddr::new(node.reg().unwrap().next().unwrap().starting_address as usize).unwrap()
        };

        let (aplic_phys, aplic) = {
            /*
             * TODO: there are actually multiple APLICs and IMSICs in the FDT - one for M-mode and
             * one for S-mode. We should instead find the one that is marked as enabled, but `fdt`
             * doesn't seem to have good support for this so this probs will work for now.
             */
            let node = fdt.find_compatible(&["riscv,aplic"]).unwrap();
            let aplic_address = node.reg().unwrap().next().unwrap().starting_address as usize;
            let address = hal_riscv::platform::kernel_map::physical_to_virtual(PAddr::new(aplic_address).unwrap());
            (aplic_address, unsafe { &*(address.ptr() as *const AplicDomain) })
        };

        info!(
            "Configuring Advanced Interrupt Architecture (IMSCI @ {:#x}, APLIC @ {:#x})",
            imsic_area, aplic_phys
        );

        Imsic::init();
        aplic.init();
        aplic.set_msi_address(usize::from(imsic_area));

        INTERRUPT_CONTROLLER
            .initialize(InterruptController::Aia { aplic, handlers: Spinlock::new(BTreeMap::new()) });
    }
}

pub fn handle_interrupt(number: u16, handler: fn(u16)) {
    match INTERRUPT_CONTROLLER.get() {
        InterruptController::Plic { plic, handlers } => {
            // TODO: don't just assume all interrupts should go to the first context
            plic.enable_interrupt(1, number as usize);
            // TODO: do priorities correctly at some point
            plic.set_source_priority(number as usize, 7);

            handlers.lock().insert(number as usize, InterruptHandler(handler as *const _));
        }
        InterruptController::Aia { handlers, .. } => {
            Imsic::enable(number as usize);
            handlers.lock().insert(number as usize, InterruptHandler(handler as *const _));
        }
    }
}

pub fn handle_wired_fdt_device_interrupt(node: FdtNode<'_, '_>, handler: fn(u16)) {
    handle_wired_device_interrupt(node.interrupts().unwrap().next().unwrap(), handler);
}

pub fn handle_wired_device_interrupt(interrupt: usize, handler: fn(u16)) {
    match INTERRUPT_CONTROLLER.get() {
        InterruptController::Plic { plic, handlers } => {
            // TODO: don't just assume all interrupts should go to the first context
            plic.enable_interrupt(1, interrupt);
            // TODO: do priorities correctly at some point
            plic.set_source_priority(interrupt, 7);

            assert!(handlers.lock().get(&(interrupt as usize)).is_none());
            handlers.lock().insert(interrupt as usize, InterruptHandler(handler as *const _));
        }
        InterruptController::Aia { aplic, handlers } => {
            /*
             * TODO:
             * I haven't worked out where this is documented yet, but the interrupt number is
             * in the top 32 bits of the `interrupt` property. I'm guessing the bottom 32 bits
             * is the phandle of the interrupt controller, which makes me think this should be
             * an `interrupt-extended` property, but it's not.
             */
            let interrupt = interrupt.get_bits(32..64);

            /*
             * Configure the APLIC to trigger an MSI with a message matching the interrupt number.
             */
            Imsic::enable(interrupt as usize);
            aplic.set_target_msi(interrupt as u32, interrupt as u32);
            // TODO: how are we supposed to know this in general?
            aplic.set_source_cfg(interrupt as u32, SourceMode::LevelHigh);
            aplic.enable_interrupt(interrupt as u32);

            handlers.lock().insert(interrupt as usize, InterruptHandler(handler as *const _));
        }
    }
}

pub fn handle_external_interrupt() {
    // TODO: it feels a little strange to do this on every interrupt. Maybe dynamically dispatch to
    // a specialised handler for PLIC vs AIA?
    match INTERRUPT_CONTROLLER.get() {
        InterruptController::Plic { plic, handlers } => {
            let interrupt = plic.claim_interrupt(1);

            let handlers = handlers.lock();
            match handlers.get(&(interrupt as usize)) {
                Some(handler) => unsafe {
                    handler.call(interrupt as u16);
                },
                None => warn!("Unhandled interrupt: {}", interrupt),
            }

            plic.complete_interrupt(1, interrupt);
        }
        InterruptController::Aia { handlers, .. } => {
            let interrupt = Imsic::pop() as usize;
            let handlers = handlers.lock();

            match handlers.get(&interrupt) {
                Some(handler) => unsafe {
                    handler.call(interrupt as u16);
                },
                None => warn!("Unhandled interrupt: {}", interrupt),
            }
        }
    }
}
