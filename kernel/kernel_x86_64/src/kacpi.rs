use crate::{clocksource::TscClocksource, interrupts::InterruptController, pci::EcamAccess};
use acpi::{
    aml::{AmlError, Interpreter},
    platform::{AcpiPlatform, PciConfigRegions},
    AcpiTables,
    PhysicalMapping,
};
use alloc::sync::Arc;
use bit_field::BitField;
use core::ptr::NonNull;
use hal::memory::{Flags, Frame, FrameSize, PAddr, Size4KiB};
use hal_x86_64::hw::{idt::InterruptStackFrame, port::Port};
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

pub fn find_tables(boot_info: &BootInfo) -> AcpiTables<PoplarHandler<EcamAccess>> {
    let Some(rsdp_addr) = boot_info.rsdp_addr() else {
        panic!("Bootloader did not pass RSDP address. Booting without ACPI is not supported.");
    };
    let tables = unsafe { AcpiTables::from_rsdp(PoplarHandler::new(), rsdp_addr as usize).unwrap() };

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
    pub platform: AcpiPlatform<PoplarHandler<EcamAccess>>,
    pub interpreter: Interpreter<PoplarHandler<EcamAccess>>,
}

impl AcpiManager {
    pub fn initialize(tables: AcpiTables<PoplarHandler<EcamAccess>>) -> (Arc<AcpiManager>, EcamAccess) {
        let pci_access = crate::pci::EcamAccess::new(PciConfigRegions::new(&tables).unwrap());
        let mut handler = PoplarHandler::new();
        handler.init_pci_access(pci_access.clone());
        let platform = AcpiPlatform::new(tables, handler.clone()).unwrap();

        platform.initialize_events().unwrap();

        let interpreter = Interpreter::new_from_platform(&platform).unwrap();
        interpreter.initialize_namespace();
        info!("ACPI namespace: {}", interpreter.namespace.lock());

        (Arc::new(AcpiManager { platform, interpreter }), pci_access)
    }

    pub fn enter_acpi_mode(&self, interrupt_controller: &mut InterruptController) {
        use hal_x86_64::hw::ioapic::{PinPolarity, TriggerMode};

        interrupt_controller
            .configure_gsi(
                self.platform.sci_interrupt as u32,
                PinPolarity::Low,
                TriggerMode::Level,
                sci_handler,
                false,
            )
            .unwrap();
        self.platform.enter_acpi_mode().unwrap();

        // TODO: sort out GPE event handling
    }
}

pub fn sci_handler(_stack_frame: &InterruptStackFrame, _num: u8) {
    /*
     * TODO: obviously this doesn't actually handle the SCI event. This should probably be a better
     * broken out kernel driver for the different events. At a minimum we need to detect what the
     * event is here and clear it or it'll keep firing.
     */
    info!("SCI interrupt occurred!!");
}

#[derive(Clone)]
pub struct PoplarHandler<A>
where
    A: ConfigRegionAccess,
{
    pci_access: Option<A>,
}

impl<A> PoplarHandler<A>
where
    A: ConfigRegionAccess,
{
    pub fn new() -> PoplarHandler<A> {
        PoplarHandler { pci_access: None }
    }

    pub fn init_pci_access(&mut self, pci_access: A) {
        self.pci_access = Some(pci_access);
    }
}

impl<A> acpi::Handler for PoplarHandler<A>
where
    A: ConfigRegionAccess + Send + Sync + Clone,
{
    unsafe fn map_physical_region<T>(&self, physical_address: usize, size: usize) -> PhysicalMapping<Self, T> {
        let virtual_address = crate::VMM.get().physical_to_virtual(PAddr::new(physical_address).unwrap());

        PhysicalMapping {
            physical_start: usize::from(physical_address),
            virtual_start: NonNull::new(virtual_address.mut_ptr()).unwrap(),
            region_length: size,
            mapped_length: size,
            handler: self.clone(),
        }
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {}

    fn read_u8(&self, address: usize) -> u8 {
        debug!("AML: Reading byte from {:#x}", address);

        let addr_to_map = Frame::<Size4KiB>::contains(PAddr::new(address).unwrap()).start;
        let mapped_virt = crate::VMM
            .get()
            .map_kernel(addr_to_map, Size4KiB::SIZE, Flags { writable: true, ..Default::default() })
            .unwrap();
        let virt = mapped_virt + (address % Size4KiB::SIZE);

        assert!(virt.is_aligned(1));
        unsafe { core::ptr::read_volatile(virt.ptr()) }
    }

    fn read_u16(&self, address: usize) -> u16 {
        debug!("AML: Reading word from {:#x}", address);
        let addr_to_map = Frame::<Size4KiB>::contains(PAddr::new(address).unwrap()).start;
        let mapped_virt = crate::VMM
            .get()
            .map_kernel(addr_to_map, Size4KiB::SIZE, Flags { writable: true, ..Default::default() })
            .unwrap();
        let virt = mapped_virt + (address % Size4KiB::SIZE);
        assert!(virt.is_aligned(2));
        unsafe { core::ptr::read_volatile(virt.ptr()) }
    }

    fn read_u32(&self, address: usize) -> u32 {
        debug!("AML: Reading dword from {:#x}", address);
        let addr_to_map = Frame::<Size4KiB>::contains(PAddr::new(address).unwrap()).start;
        let mapped_virt = crate::VMM
            .get()
            .map_kernel(addr_to_map, Size4KiB::SIZE, Flags { writable: true, ..Default::default() })
            .unwrap();
        let virt = mapped_virt + (address % Size4KiB::SIZE);
        assert!(virt.is_aligned(4));
        unsafe { core::ptr::read_volatile(virt.ptr()) }
    }

    fn read_u64(&self, address: usize) -> u64 {
        debug!("AML: Reading qword from {:#x}", address);
        let addr_to_map = Frame::<Size4KiB>::contains(PAddr::new(address).unwrap()).start;
        let mapped_virt = crate::VMM
            .get()
            .map_kernel(addr_to_map, Size4KiB::SIZE, Flags { writable: true, ..Default::default() })
            .unwrap();
        let virt = mapped_virt + (address % Size4KiB::SIZE);
        assert!(virt.is_aligned(8));
        unsafe { core::ptr::read_volatile(virt.ptr()) }
    }

    fn write_u8(&self, address: usize, value: u8) {
        debug!("AML: Writing byte to {:#x}: {:#x}", address, value);
        let addr_to_map = Frame::<Size4KiB>::contains(PAddr::new(address).unwrap()).start;
        let mapped_virt = crate::VMM
            .get()
            .map_kernel(addr_to_map, Size4KiB::SIZE, Flags { writable: true, ..Default::default() })
            .unwrap();
        let virt = mapped_virt + (address % Size4KiB::SIZE);
        assert!(virt.is_aligned(1));
        unsafe { core::ptr::write_volatile(virt.mut_ptr(), value) }
    }

    fn write_u16(&self, address: usize, value: u16) {
        debug!("AML: Writing word to {:#x}: {:#x}", address, value);
        let addr_to_map = Frame::<Size4KiB>::contains(PAddr::new(address).unwrap()).start;
        let mapped_virt = crate::VMM
            .get()
            .map_kernel(addr_to_map, Size4KiB::SIZE, Flags { writable: true, ..Default::default() })
            .unwrap();
        let virt = mapped_virt + (address % Size4KiB::SIZE);
        assert!(virt.is_aligned(2));
        unsafe { core::ptr::write_volatile(virt.mut_ptr(), value) }
    }

    fn write_u32(&self, address: usize, value: u32) {
        debug!("AML: Writing dword to {:#x}: {:#x}", address, value);
        let addr_to_map = Frame::<Size4KiB>::contains(PAddr::new(address).unwrap()).start;
        let mapped_virt = crate::VMM
            .get()
            .map_kernel(addr_to_map, Size4KiB::SIZE, Flags { writable: true, ..Default::default() })
            .unwrap();
        let virt = mapped_virt + (address % Size4KiB::SIZE);
        assert!(virt.is_aligned(4));
        unsafe { core::ptr::write_volatile(virt.mut_ptr(), value) }
    }

    fn write_u64(&self, address: usize, value: u64) {
        debug!("AML: Writing qword to {:#x}: {:#x}", address, value);
        let addr_to_map = Frame::<Size4KiB>::contains(PAddr::new(address).unwrap()).start;
        let mapped_virt = crate::VMM
            .get()
            .map_kernel(addr_to_map, Size4KiB::SIZE, Flags { writable: true, ..Default::default() })
            .unwrap();
        let virt = mapped_virt + (address % Size4KiB::SIZE);
        assert!(virt.is_aligned(8));
        unsafe { core::ptr::write_volatile(virt.mut_ptr(), value) }
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
        let dword_read = unsafe { self.pci_access.as_ref().unwrap().read(address, align_down(offset, 0x20)) };
        let start_bit = (offset % 0x20) as usize;
        dword_read.get_bits(start_bit..(start_bit + 8)) as u8
    }

    fn read_pci_u16(&self, address: PciAddress, offset: u16) -> u16 {
        debug!("AML: Reading word from PCI config space {:?}(offset={:#x})", address, offset);
        let dword_read = unsafe { self.pci_access.as_ref().unwrap().read(address, align_down(offset, 0x20)) };
        let start_bit = (offset % 0x20) as usize;
        dword_read.get_bits(start_bit..(start_bit + 16)) as u16
    }

    fn read_pci_u32(&self, address: PciAddress, offset: u16) -> u32 {
        debug!("AML: Reading dword from PCI config space {:?}(offset={:#x})", address, offset);
        unsafe { self.pci_access.as_ref().unwrap().read(address, offset) }
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
        unsafe { self.pci_access.as_ref().unwrap().write(address, offset, value) }
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

    fn create_mutex(&self) -> acpi::Handle {
        // TODO: keep track of mutexes
        acpi::Handle(0)
    }

    fn acquire(&self, mutex: acpi::Handle, timeout: u16) -> Result<(), AmlError> {
        // TODO
        Ok(())
    }

    fn release(&self, mutex: acpi::Handle) {
        // TODO
    }
}
