use crate::interrupts;
use alloc::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use bit_field::BitField;
use core::ptr;
use fdt::Fdt;
use hal::memory::PAddr;
use kernel::{object::event::Event, pci::PciInterruptConfigurator};
use pci_types::{capability::MsiCapability, ConfigRegionAccess, PciAddress};
use poplar_util::InitGuard;
use spinning_top::Spinlock;
use tracing::info;

pub struct PciAccess {
    start: *const u8,
    size: usize,
}

impl PciAccess {
    pub fn new(fdt: &Fdt) -> Option<PciAccess> {
        let pci_node = fdt
            .all_nodes()
            .filter(|node| {
                node.compatible().map_or(false, |c| {
                    c.all().any(|c| ["pci-host-ecam-generic", "pci-host-cam-generic"].contains(&c))
                })
            })
            .next()?;
        let ecam_window = pci_node.reg().expect("PCI entry doesn't have a reg property").next().unwrap();
        let ecam_address = hal_riscv::platform::kernel_map::physical_to_virtual(
            PAddr::new(ecam_window.starting_address as usize).unwrap(),
        );

        PCI_EVENTS.initialize(Spinlock::new(BTreeMap::new()));

        Some(PciAccess { start: ecam_address.ptr(), size: ecam_window.size.unwrap() })
    }

    fn address_for(&self, pci_address: PciAddress) -> *const u8 {
        unsafe {
            self.start.add(
                usize::from(pci_address.bus()) << 20
                    | usize::from(pci_address.device()) << 15
                    | usize::from(pci_address.function()) << 12,
            )
        }
    }
}

unsafe impl Send for PciAccess {}

impl ConfigRegionAccess for PciAccess {
    fn function_exists(&self, _address: PciAddress) -> bool {
        // TODO
        true
    }

    unsafe fn read(&self, address: PciAddress, offset: u16) -> u32 {
        ptr::read_volatile(self.address_for(address).add(offset as usize) as *const u32)
    }

    unsafe fn write(&self, address: PciAddress, offset: u16, value: u32) {
        ptr::write_volatile(self.address_for(address).add(offset as usize) as *mut u32, value);
    }
}

impl PciInterruptConfigurator for PciAccess {
    fn configure_interrupt(&self, function: PciAddress, msi: &mut MsiCapability) -> Arc<Event> {
        let event = Event::new();

        // TODO: allocate numbers from somewhere???
        let message_number = 2;

        PCI_EVENTS.get().lock().insert(message_number, event.clone());

        interrupts::handle_interrupt(message_number, pci_interrupt_handler);

        // TODO: get out of the device tree
        msi.set_message_info(0x28000000, message_number as u8, pci_types::capability::TriggerMode::Edge, self);
        msi.set_enabled(true, self);

        event
    }
}

// TODO: interrupt guard
static PCI_EVENTS: InitGuard<Spinlock<BTreeMap<u16, Arc<Event>>>> = InitGuard::uninit();

fn pci_interrupt_handler(number: u16) {
    info!("PCI interrupt: {}", number);
    if let Some(event) = PCI_EVENTS.get().lock().get(&number) {
        event.signal();
    }
}
