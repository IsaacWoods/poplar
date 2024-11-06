#![feature(never_type, exclusive_range_pattern)]
#![deny(unsafe_op_in_unsafe_fn)]

mod caps;
mod memory;
mod operational;
mod trb;

use caps::Capabilities;
use log::info;
use memory::MemoryArea;
use operational::OperationRegisters;
use platform_bus::{BusDriverMessage, DeviceDriverMessage, DeviceDriverRequest, Filter, Property};
use std::poplar::{
    channel::Channel,
    early_logger::EarlyLogger,
    memory_object::MemoryObject,
    syscall::{self, MemoryObjectFlags},
};

/*
 * TODO: this is currently broken from many updates to userspace and `platform_bus`. When we get
 * round to XHCI support (which I imagine will be in quite a bit as none of the hardware we're
 * interested in initially has support for it) this will need a thorough rework, probably based off
 * the EHCI driver.
 */

pub fn main() {
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

    let register_space_size =
        controller_device.properties.get("pci.bar0.size").unwrap().as_integer().unwrap() as usize;
    let register_space = MemoryObject {
        handle: controller_device.properties.get("pci.bar0.handle").as_ref().unwrap().as_memory_object().unwrap(),
        size: register_space_size,
        flags: MemoryObjectFlags::WRITABLE,
        phys_address: None,
    };
    const REGISTER_SPACE_ADDRESS: usize = 0x00000005_00000000;
    unsafe {
        register_space.map_at(REGISTER_SPACE_ADDRESS).unwrap();
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
        std::poplar::syscall::yield_to_kernel()
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
