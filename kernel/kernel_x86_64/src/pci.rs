use crate::{interrupts::INTERRUPT_CONTROLLER, kacpi::AcpiManager};
use acpi::{
    aml::{
        namespace::AmlName,
        pci_routing::{PciRoutingTable, Pin},
    },
    platform::PciConfigRegions,
};
use alloc::{alloc::Global, collections::btree_map::BTreeMap, sync::Arc, vec, vec::Vec};
use bit_field::BitField;
use core::{ptr, str::FromStr};
use hal::memory::{Flags, FrameSize, PAddr, Size4KiB, VAddr};
use hal_x86_64::hw::{
    idt::InterruptStackFrame,
    ioapic::{PinPolarity, TriggerMode},
};
use kernel::{object::interrupt::Interrupt, pci::PciInterruptConfigurator};
use pci_types::{
    capability::{MsiCapability, MsixCapability},
    Bar,
    ConfigRegionAccess,
    PciAddress,
};
use spinning_top::Spinlock;
use tracing::info;

// TODO: this should have an interrupt guard as well
/// Maps platform interrupt numbers to sets of PCI events
static INTERRUPT_ROUTING: Spinlock<BTreeMap<u8, Vec<Arc<Interrupt>>>> = Spinlock::new(BTreeMap::new());

#[derive(Clone, Copy)]
struct BusMapping {
    starts_at: PAddr,
    mapped_at: VAddr,
}

struct EcamAccessInner {
    regions: PciConfigRegions,
    bus_mappings: Spinlock<BTreeMap<u8, BusMapping>>,
}

/// Allows access to PCI configuration space via the ECAM.
#[derive(Clone)]
pub struct EcamAccess(Arc<EcamAccessInner>);

impl EcamAccess {
    pub fn new(regions: PciConfigRegions<Global>) -> EcamAccess {
        let inner = EcamAccessInner { regions, bus_mappings: Spinlock::new(BTreeMap::new()) };
        EcamAccess(Arc::new(inner))
    }

    fn map_bus(&self, segment: u16, bus: u8) -> BusMapping {
        let mut mappings = self.0.bus_mappings.lock();

        if let Some(mapping) = mappings.get(&bus) {
            return *mapping;
        }

        /*
         * We create mappings for each bus on-demand as they are first enumerated. This makes it
         * easy to size the mappings as each bus covers 2^20 bytes of total configuration space.
         */
        const BUS_MAPPING_SIZE: usize = 1 << 20;
        let phys = PAddr::new(self.0.regions.physical_address(segment, bus, 0, 0).unwrap() as usize).unwrap();
        let virt = crate::VMM
            .get()
            .map_kernel(phys, BUS_MAPPING_SIZE, Flags { writable: true, cached: false, ..Default::default() })
            .unwrap();
        mappings.insert(bus, BusMapping { starts_at: phys, mapped_at: virt });
        *mappings.get(&bus).unwrap()
    }
}

impl ConfigRegionAccess for EcamAccess {
    unsafe fn read(&self, address: PciAddress, offset: u16) -> u32 {
        let physical_address = self
            .0
            .regions
            .physical_address(address.segment(), address.bus(), address.device(), address.function())
            .unwrap();
        let mapping = self.map_bus(address.segment(), address.bus());
        let ptr = {
            let function_offset = (physical_address as usize) - usize::from(mapping.starts_at);
            assert!(function_offset < (1 << 20));
            (mapping.mapped_at + function_offset + offset as usize).ptr()
        };
        // TODO: ECAM accesses should be done into/out of EAX only. Do this in asm
        ptr::read_volatile(ptr)
    }

    unsafe fn write(&self, address: PciAddress, offset: u16, value: u32) {
        let physical_address = self
            .0
            .regions
            .physical_address(address.segment(), address.bus(), address.device(), address.function())
            .unwrap();
        let mapping = self.map_bus(address.segment(), address.bus());
        let ptr = {
            let function_offset = (physical_address as usize) - usize::from(mapping.starts_at);
            assert!(function_offset < (1 << 20));
            (mapping.mapped_at + function_offset + offset as usize).mut_ptr()
        };
        // TODO: ECAM accesses should be done into/out of EAX only. Do this in asm
        ptr::write_volatile(ptr, value)
    }
}

/// `PciConfigurator` is the full configuration system for PCI devices on x64. It takes over access
/// to the configuration space via `EcamAccess`, but also allows configuration of interrupts via
/// the legacy interrupt mechanical or MSIs.
///
/// An instance of this type is given to the common kernel to help it configure PCI devices.
pub struct PciConfigurator {
    access: EcamAccess,
    legacy_routing_table: PciRoutingTable,
    /// Maps from GSIs allocated to legacy PCI interrupts to platform interrupt number
    legacy_platform_interrupts: Spinlock<BTreeMap<u32, u8>>,
    acpi: Arc<AcpiManager>,
}

impl PciConfigurator {
    pub fn new(access: EcamAccess, acpi: Arc<AcpiManager>) -> PciConfigurator {
        let legacy_routing_table =
            PciRoutingTable::from_prt_path(AmlName::from_str("\\_SB.PCI0._PRT").unwrap(), &acpi.interpreter)
                .expect("Failed to parse _PRT");

        PciConfigurator {
            access,
            legacy_routing_table,
            legacy_platform_interrupts: Spinlock::new(BTreeMap::new()),
            acpi,
        }
    }
}

impl ConfigRegionAccess for PciConfigurator {
    unsafe fn read(&self, address: PciAddress, offset: u16) -> u32 {
        self.access.read(address, offset)
    }

    unsafe fn write(&self, address: PciAddress, offset: u16, value: u32) {
        self.access.write(address, offset, value);
    }
}

impl PciInterruptConfigurator for PciConfigurator {
    fn configure_legacy(&self, function: PciAddress, pin: u8) -> Arc<Interrupt> {
        let pin = match pin {
            1 => Pin::IntA,
            2 => Pin::IntB,
            3 => Pin::IntC,
            4 => Pin::IntD,
            _ => panic!(),
        };
        let routed_gsi = self
            .legacy_routing_table
            .route(function.device() as u16, function.function() as u16, pin, &self.acpi.interpreter)
            .unwrap();

        let interrupt = Interrupt::new(Some(routed_gsi.irq as usize));

        let mut legacy_platform_interrupts = self.legacy_platform_interrupts.lock();
        if let Some(platform_interrupt) = legacy_platform_interrupts.get(&routed_gsi.irq) {
            INTERRUPT_ROUTING.lock().get_mut(platform_interrupt).unwrap().push(interrupt.clone());
        } else {
            let platform_interrupt = INTERRUPT_CONTROLLER
                .get()
                .lock()
                .configure_gsi(routed_gsi.irq, PinPolarity::Low, TriggerMode::Level, handle_pci_interrupt, true)
                .unwrap();
            legacy_platform_interrupts.insert(routed_gsi.irq, platform_interrupt);
            INTERRUPT_ROUTING.lock().insert(platform_interrupt, vec![interrupt.clone()]);
        }

        interrupt
    }

    fn configure_msi(&self, _function: PciAddress, msi: &mut MsiCapability) -> Arc<Interrupt> {
        let interrupt = Interrupt::new(None);

        let platform_interrupt =
            INTERRUPT_CONTROLLER.get().lock().allocate_platform_interrupt(handle_pci_interrupt, None);
        INTERRUPT_ROUTING.lock().insert(platform_interrupt, vec![interrupt.clone()]);

        let msi_address = {
            let mut address = 0;
            address.set_bits(20..32, 0x0fee);
            address.set_bits(12..20, 0);
            address.set_bit(2, false);
            address.set_bit(3, false);
            address
        };
        let msi_data = {
            let mut data = 0u32;
            data.set_bits(0..8, platform_interrupt as u32); // Vector
            data
        };
        msi.set_message_info(msi_address, msi_data, self);
        msi.set_enabled(true, self);

        interrupt
    }

    fn configure_msix(&self, function: PciAddress, bar: Bar, msix: &mut MsixCapability) -> Arc<Interrupt> {
        let interrupt = Interrupt::new(None);
        info!("Configuring PCI device to use MSI-X interrupts: {:?}", function);

        let platform_interrupt =
            INTERRUPT_CONTROLLER.get().lock().allocate_platform_interrupt(handle_pci_interrupt, None);
        INTERRUPT_ROUTING.lock().insert(platform_interrupt, vec![interrupt.clone()]);

        msix.set_enabled(true, self);

        let table_base_phys = match bar {
            Bar::Memory32 { address, .. } => (address + msix.table_offset()) as usize,
            Bar::Memory64 { address, .. } => address as usize + msix.table_offset() as usize,
            _ => panic!(),
        };
        let table_base_virt = crate::VMM
            .get()
            .map_kernel(
                PAddr::new(table_base_phys).unwrap(),
                Size4KiB::SIZE,
                Flags { writable: true, cached: false, ..Default::default() },
            )
            .unwrap();
        // TODO: offset into the table if we ever need an entry that isn't the first
        let entry_ptr = table_base_virt.mut_ptr() as *mut u32;

        let msi_address = {
            let mut address = 0;
            address.set_bits(20..32, 0x0fee);
            address.set_bits(12..20, 0);
            address.set_bit(2, false);
            address.set_bit(3, false);
            address
        };

        let msi_data = {
            let mut data = 0u32;
            data.set_bits(0..8, platform_interrupt as u32);
            data.set_bits(8..11, 0b000); // Fixed delivery mode
            data.set_bit(14, false); // Level for trigger mode = doesn't matter
            data.set_bit(15, false); // Trigger mode = edge
            data
        };

        /*
         * Each entry of the MSI-X table is laid out as:
         *    0x00 => Message Address
         *    0x04 => Message Upper Address
         *    0x08 => Message Data
         *    0x0c => Vector Control
         */
        unsafe {
            ptr::write_volatile(entry_ptr.byte_add(0x00), msi_address);
            ptr::write_volatile(entry_ptr.byte_add(0x04), 0);
            ptr::write_volatile(entry_ptr.byte_add(0x08), msi_data);
            ptr::write_volatile(entry_ptr.byte_add(0x0c), 0);
        }

        interrupt
    }
}

pub fn handle_pci_interrupt(_: &InterruptStackFrame, platform_interrupt: u8) {
    let routing = INTERRUPT_ROUTING.lock();
    if let Some(interrupts) = routing.get(&platform_interrupt) {
        for interrupt in interrupts {
            interrupt.trigger();
        }
    } else {
        panic!("Unhandled PCI interrupt: {}", platform_interrupt);
    }
}
