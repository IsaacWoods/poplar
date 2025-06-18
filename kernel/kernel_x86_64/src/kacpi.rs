use crate::{clocksource::TscClocksource, kernel_map, pci::EcamAccess};
use acpi::{
    aml::{AmlError, Interpreter},
    platform::{AcpiPlatform, PciConfigRegions},
    AcpiHandler,
    AcpiTables,
    PhysicalMapping,
};
use alloc::sync::Arc;
use bit_field::BitField;
use core::ptr::NonNull;
use hal::memory::PAddr;
use hal_x86_64::hw::port::Port;
use kernel::{bootinfo::BootInfo, clocksource::Clocksource};
use mulch::math::align_down;
use pci_types::{ConfigRegionAccess, PciAddress};
use tracing::{debug, info};

/*
 * TODO: allowing the computer to shut down
 *
 * - Provide a nice view of the PM1 register block (read/write support)
 * - Register a handler for the SCI interrupt (respecting overrides etc.)
 * - Configure events and enter ACPI mode
 * - Detect a fixed power button press from the PM1 registers on an SCI
 * - Do the actual shutdown bit: _PTS and _S5 and all that
 *
 * Long term:
 * - Provide a power button device as a kernel device on the PlatformBus
 * - Provide a power control device as a kernel device on the PlatformBus
 * - Decide which bit of userspace is responsible for sticking the two together
 *
 * I think we'll need some clever system of scheduling work from the interrupt handler to execute
 * later, because executing AML from an interrupt context is a crime. We could just utilise the
 * `tasklet` Maitake async system for this - bonus points if we can make the interpreter itself
 * async.
 *
 * Eventually, we'll want a kernel EC driver too I think - we can have a userspace aspect by making
 * it a kernel device on the PlatformBus maybe.
 */

pub fn find_tables(boot_info: &BootInfo) -> AcpiTables<PoplarAcpiHandler> {
    let Some(rsdp_addr) = boot_info.rsdp_addr() else {
        panic!("Bootloader did not pass RSDP address. Booting without ACPI is not supported.");
    };
    let tables = unsafe { AcpiTables::from_rsdp(PoplarAcpiHandler, rsdp_addr as usize).unwrap() };

    for (addr, table) in tables.table_headers() {
        info!(
            "{} {:8x} {:4x} {:2x} {:6} {:8} {:2x} {:4} {:8x}",
            table.signature,
            addr,
            table.length(),
            table.revision(),
            table.oem_id().unwrap_or("??????"),
            table.oem_table_id().unwrap_or("????????"),
            table.oem_revision(),
            table.creator_id().unwrap_or("????"),
            table.creator_revision(),
        );
    }

    tables
}

pub struct AcpiManager {
    pub platform: AcpiPlatform<PoplarAcpiHandler>,
    pub interpreter: acpi::aml::Interpreter<AmlHandler<EcamAccess>>,
}

impl AcpiManager {
    pub fn initialize(tables: AcpiTables<PoplarAcpiHandler>) -> (Arc<AcpiManager>, EcamAccess) {
        let platform = AcpiPlatform::new(tables).unwrap();
        let pci_access = crate::pci::EcamAccess::new(PciConfigRegions::new(&platform.tables).unwrap());
        let aml_handler = AmlHandler { pci_access: pci_access.clone() };

        let interpreter = Interpreter::new_from_tables(PoplarAcpiHandler, aml_handler, &platform.tables).unwrap();
        interpreter.initialize_namespace();
        info!("ACPI namespace: {}", interpreter.namespace.lock());

        (Arc::new(AcpiManager { platform, interpreter }), pci_access)
    }
}

#[derive(Clone)]
pub struct PoplarAcpiHandler;

impl AcpiHandler for PoplarAcpiHandler {
    unsafe fn map_physical_region<T>(&self, physical_address: usize, size: usize) -> PhysicalMapping<Self, T> {
        let virtual_address = kernel_map::physical_to_virtual(PAddr::new(physical_address).unwrap());

        PhysicalMapping::new(
            usize::from(physical_address),
            NonNull::new(virtual_address.mut_ptr()).unwrap(),
            size,
            size,
            PoplarAcpiHandler,
        )
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {}
}

pub struct AmlHandler<A>
where
    A: ConfigRegionAccess,
{
    pci_access: A,
}

impl<A> AmlHandler<A>
where
    A: ConfigRegionAccess,
{
    pub fn new(pci_access: A) -> AmlHandler<A> {
        AmlHandler { pci_access }
    }
}

impl<A> acpi::aml::Handler for AmlHandler<A>
where
    A: ConfigRegionAccess + Send + Sync,
{
    fn read_u8(&self, address: usize) -> u8 {
        debug!("AML: Reading byte from {:#x}", address);
        let address = hal_x86_64::kernel_map::physical_to_virtual(PAddr::new(address).unwrap());
        assert!(address.is_aligned(1));
        unsafe { core::ptr::read_volatile(address.ptr()) }
    }

    fn read_u16(&self, address: usize) -> u16 {
        debug!("AML: Reading word from {:#x}", address);
        let address = hal_x86_64::kernel_map::physical_to_virtual(PAddr::new(address).unwrap());
        assert!(address.is_aligned(2));
        unsafe { core::ptr::read_volatile(address.ptr()) }
    }

    fn read_u32(&self, address: usize) -> u32 {
        debug!("AML: Reading dword from {:#x}", address);
        let address = hal_x86_64::kernel_map::physical_to_virtual(PAddr::new(address).unwrap());
        assert!(address.is_aligned(4));
        unsafe { core::ptr::read_volatile(address.ptr()) }
    }

    fn read_u64(&self, address: usize) -> u64 {
        debug!("AML: Reading qword from {:#x}", address);
        let address = hal_x86_64::kernel_map::physical_to_virtual(PAddr::new(address).unwrap());
        assert!(address.is_aligned(8));
        unsafe { core::ptr::read_volatile(address.ptr()) }
    }

    fn write_u8(&self, address: usize, value: u8) {
        debug!("AML: Writing byte to {:#x}: {:#x}", address, value);
        let address = hal_x86_64::kernel_map::physical_to_virtual(PAddr::new(address).unwrap());
        assert!(address.is_aligned(1));
        unsafe { core::ptr::write_volatile(address.mut_ptr(), value) }
    }

    fn write_u16(&self, address: usize, value: u16) {
        debug!("AML: Writing word to {:#x}: {:#x}", address, value);
        let address = hal_x86_64::kernel_map::physical_to_virtual(PAddr::new(address).unwrap());
        assert!(address.is_aligned(2));
        unsafe { core::ptr::write_volatile(address.mut_ptr(), value) }
    }

    fn write_u32(&self, address: usize, value: u32) {
        debug!("AML: Writing dword to {:#x}: {:#x}", address, value);
        let address = hal_x86_64::kernel_map::physical_to_virtual(PAddr::new(address).unwrap());
        assert!(address.is_aligned(4));
        unsafe { core::ptr::write_volatile(address.mut_ptr(), value) }
    }

    fn write_u64(&self, address: usize, value: u64) {
        debug!("AML: Writing qword to {:#x}: {:#x}", address, value);
        let address = hal_x86_64::kernel_map::physical_to_virtual(PAddr::new(address).unwrap());
        assert!(address.is_aligned(8));
        unsafe { core::ptr::write_volatile(address.mut_ptr(), value) }
    }

    fn read_io_u8(&self, port: u16) -> u8 {
        debug!("AML: Reading IO byte from port {:#x}", port);
        unsafe { Port::new(port).read() }
    }

    fn read_io_u16(&self, port: u16) -> u16 {
        debug!("AML: Reading IO word from port {:#x}", port);
        unsafe { Port::new(port).read() }
    }

    fn read_io_u32(&self, port: u16) -> u32 {
        debug!("AML: Reading IO dword from port {:#x}", port);
        unsafe { Port::new(port).read() }
    }

    fn write_io_u8(&self, port: u16, value: u8) {
        debug!("AML: Writing IO byte to port {:#x}: {:#x}", port, value);
        unsafe { Port::new(port).write(value) }
    }

    fn write_io_u16(&self, port: u16, value: u16) {
        debug!("AML: Writing IO word to port {:#x}: {:#x}", port, value);
        unsafe { Port::new(port).write(value) }
    }

    fn write_io_u32(&self, port: u16, value: u32) {
        debug!("AML: Writing IO dword to port {:#x}: {:#x}", port, value);
        unsafe { Port::new(port).write(value) }
    }

    fn read_pci_u8(&self, address: PciAddress, offset: u16) -> u8 {
        debug!("AML: Reading byte from PCI config space {:?}(offset={:#x})", address, offset);
        let dword_read = unsafe { self.pci_access.read(address, align_down(offset, 0x20)) };
        let start_bit = (offset % 0x20) as usize;
        dword_read.get_bits(start_bit..(start_bit + 8)) as u8
    }

    fn read_pci_u16(&self, address: PciAddress, offset: u16) -> u16 {
        debug!("AML: Reading word from PCI config space {:?}(offset={:#x})", address, offset);
        let dword_read = unsafe { self.pci_access.read(address, align_down(offset, 0x20)) };
        let start_bit = (offset % 0x20) as usize;
        dword_read.get_bits(start_bit..(start_bit + 16)) as u16
    }

    fn read_pci_u32(&self, address: PciAddress, offset: u16) -> u32 {
        debug!("AML: Reading dword from PCI config space {:?}(offset={:#x})", address, offset);
        unsafe { self.pci_access.read(address, offset) }
    }

    fn write_pci_u8(&self, address: PciAddress, offset: u16, value: u8) {
        debug!("AML: Writing byte from PCI config space {:?}(offset={:#x}) <- {:#x}", address, offset, value);
        unimplemented!()
    }

    fn write_pci_u16(&self, address: PciAddress, offset: u16, value: u16) {
        debug!("AML: Writing word from PCI config space {:?}(offset={:#x}) <- {:#x}", address, offset, value);
        unimplemented!()
    }

    fn write_pci_u32(&self, address: PciAddress, offset: u16, value: u32) {
        debug!("AML: Writing dword from PCI config space {:?}(offset={:#x}) <- {:#x}", address, offset, value);
        unsafe { self.pci_access.write(address, offset, value) }
    }

    fn nanos_since_boot(&self) -> u64 {
        TscClocksource::nanos_since_boot()
    }

    fn stall(&self, _microseconds: u64) {
        todo!()
    }

    fn sleep(&self, _milliseconds: u64) {
        todo!()
    }

    fn create_mutex(&self) -> acpi::aml::Handle {
        // TODO: keep track of mutexes
        acpi::aml::Handle(0)
    }

    fn acquire(&self, mutex: acpi::aml::Handle, timeout: u16) -> Result<(), AmlError> {
        // TODO
        Ok(())
    }

    fn release(&self, mutex: acpi::aml::Handle) {
        // TODO
    }
}
