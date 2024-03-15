use acpi::PciConfigRegions;
use alloc::{alloc::Global, sync::Arc};
use core::ptr;
use hal::memory::PAddr;
use hal_x86_64::kernel_map;
use kernel::{object::event::Event, pci::PciInterruptConfigurator};
use pci_types::{capability::MsiCapability, ConfigRegionAccess, PciAddress};
use tracing::warn;

#[derive(Clone)]
pub struct EcamAccess<'a>(Arc<PciConfigRegions<'a, Global>>);

impl<'a> EcamAccess<'a> {
    pub fn new(regions: PciConfigRegions<'a, Global>) -> EcamAccess<'a> {
        EcamAccess(Arc::new(regions))
    }
}

impl<'a> ConfigRegionAccess for EcamAccess<'a> {
    fn function_exists(&self, address: PciAddress) -> bool {
        self.0.physical_address(address.segment(), address.bus(), address.device(), address.function()).is_some()
    }

    unsafe fn read(&self, address: PciAddress, offset: u16) -> u32 {
        let physical_address = self
            .0
            .physical_address(address.segment(), address.bus(), address.device(), address.function())
            .unwrap();
        let ptr = (kernel_map::physical_to_virtual(PAddr::new(physical_address as usize).unwrap())
            + offset as usize)
            .ptr();
        ptr::read_volatile(ptr)
    }

    unsafe fn write(&self, address: PciAddress, offset: u16, value: u32) {
        let physical_address = self
            .0
            .physical_address(address.segment(), address.bus(), address.device(), address.function())
            .unwrap();
        let ptr = (kernel_map::physical_to_virtual(PAddr::new(physical_address as usize).unwrap())
            + offset as usize)
            .mut_ptr();
        ptr::write_volatile(ptr, value)
    }
}

impl<'a> PciInterruptConfigurator for EcamAccess<'a> {
    fn configure_interrupt(&self, _function: PciAddress, _msi: &mut MsiCapability) -> Arc<Event> {
        let event = Event::new();
        warn!("MSI support is incomplete on x86_64! PCI interrupts will not trigger delegated `Event` objects!");
        event
    }
}
