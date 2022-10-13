#![no_std]
#![no_main]
#![feature(alloc_error_handler, never_type, exclusive_range_pattern)]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

mod caps;
mod memory;
mod operational;
mod trb;

use alloc::{string::String, vec};
use caps::Capabilities;
use core::panic::PanicInfo;
use linked_list_allocator::LockedHeap;
use log::info;
use memory::MemoryArea;
use operational::OperationRegisters;
use platform_bus::{BusDriverMessage, DeviceDriverMessage, DeviceDriverRequest, Filter, Property};
use poplar::{
    caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING, CAP_SERVICE_USER},
    channel::Channel,
    early_logger::EarlyLogger,
    syscall,
};

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[no_mangle]
pub extern "C" fn _start() -> ! {
    syscall::early_log("Hello from usb_bus_xhci!").unwrap();
    // Initialise the heap
    const HEAP_START: usize = 0x600000000;
    const HEAP_SIZE: usize = 0x4000;
    let heap_memory_object =
        syscall::create_memory_object(HEAP_START, HEAP_SIZE, true, false, 0x0 as *mut usize).unwrap();
    unsafe {
        syscall::map_memory_object(&heap_memory_object, &poplar::ZERO_HANDLE, None, 0x0 as *mut usize).unwrap();
        ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
    }

    log::set_logger(&EarlyLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("XHCI USB Bus Driver is running!");

    // This allows us to talk to the PlatformBus as a bus driver (to register USB devices).
    let platform_bus_bus_channel: Channel<BusDriverMessage, !> =
        Channel::from_handle(syscall::subscribe_to_service("platform_bus.bus_driver").unwrap());
    // This allows us to talk to the PlatformBus as a device driver (to find controllers we can manage).
    let platform_bus_device_channel: Channel<DeviceDriverMessage, DeviceDriverRequest> =
        Channel::from_handle(syscall::subscribe_to_service("platform_bus.device_driver").unwrap());

    // Tell PlatformBus that we're interested in XHCI controllers.
    platform_bus_device_channel
        .send(&DeviceDriverMessage::RegisterInterest(vec![
            Filter::Matches(String::from("pci.class"), Property::Integer(0x0c)),
            Filter::Matches(String::from("pci.sub_class"), Property::Integer(0x03)),
            Filter::Matches(String::from("pci.interface"), Property::Integer(0x30)),
        ]))
        .unwrap();

    // TODO: we currently only support one controller, and just stop listening after we find the first one
    // TODO: probably don't bother changing this until we have a futures-based message interface
    let mut controller_device = loop {
        match platform_bus_device_channel.try_receive().unwrap() {
            Some(DeviceDriverRequest::HandoffDevice(device_name, device)) => {
                info!("Started driving a XHCI controller: {}", device_name);
                break device;
            }
            None => syscall::yield_to_kernel(),
        }
    };

    let register_space_size = controller_device.properties.get("pci.bar0.size").unwrap().as_integer().unwrap();
    const REGISTER_SPACE_ADDRESS: usize = 0x00000005_00000000;
    unsafe {
        syscall::map_memory_object(
            controller_device.properties.get("pci.bar0.handle").as_ref().unwrap().as_memory_object().unwrap(),
            &poplar::ZERO_HANDLE,
            Some(REGISTER_SPACE_ADDRESS),
            0x0 as *mut usize,
        )
        .unwrap();
    }

    let capabilities = unsafe { Capabilities::read_from_registers(REGISTER_SPACE_ADDRESS) };
    info!("Capabilites: {:#?}", capabilities);

    let mut operational = unsafe {
        OperationRegisters::new(
            REGISTER_SPACE_ADDRESS + usize::from(capabilities.operation_registers_offset),
            capabilities.max_ports,
        )
    };

    for i in 0..capabilities.max_ports {
        info!("Port {}: {:?}", i, operational.port(i).port_link_state());
    }

    let memory_area = MemoryArea::new(capabilities.max_ports);
    initialize_controller(&mut operational, &capabilities, &memory_area);

    loop {
        syscall::yield_to_kernel()
    }
}

fn initialize_controller(
    operational: &mut OperationRegisters,
    capabilities: &Capabilities,
    memory_area: &MemoryArea,
) {
    // Wait until the controller clears the Controller Not Ready bit
    while operational.usb_status().controller_not_ready() {
        // TODO: is this enough to stop it from getting optimized out?
    }

    // Set the number of device slots that are enabled
    operational.update_config(|mut config| {
        // TODO: should we always enable all of the ports?
        config.set_device_slots_enabled(capabilities.max_ports);
        config
    });

    // Set the physical address of the Device Context Base Address Pointer Register
    operational.set_device_context_base_address_array_pointer(
        memory_area.physical_address_of_device_context_base_address_array() as u64,
    );

    // todo!()
}

#[panic_handler]
pub fn handle_panic(info: &PanicInfo) -> ! {
    log::error!("PANIC: {}", info);
    loop {}
}

#[alloc_error_handler]
fn alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Alloc error: {:?}", layout);
}

#[used]
#[link_section = ".caps"]
pub static mut CAPS: CapabilitiesRepr<4> =
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_SERVICE_USER, CAP_PADDING, CAP_PADDING]);
