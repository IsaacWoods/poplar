#![feature(never_type)]

use log::info;
use pci_types::device_type::{DeviceType, UsbType};
use platform_bus::{BusDriverMessage, DeviceInfo, HandoffInfo, HandoffProperty, Property};
use std::{
    collections::BTreeMap,
    poplar::{
        caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING, CAP_PCI_BUS_DRIVER, CAP_SERVICE_USER},
        channel::Channel,
        ddk::pci::Bar,
        early_logger::EarlyLogger,
        syscall,
    },
};

pub fn main() {
    log::set_logger(&EarlyLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("PCI bus driver is running!");

    let platform_bus_channel: Channel<BusDriverMessage, !> =
        Channel::subscribe_to_service("platform_bus.bus_driver")
            .expect("Couldn't subscribe to platform_bus.bus_driver service!");

    let mut descriptors = std::poplar::ddk::pci::pci_get_info_vec().expect("Failed to get PCI descriptors");
    for descriptor in descriptors.drain(..) {
        let device_type = DeviceType::from((descriptor.class, descriptor.sub_class));
        info!(
            "PCI device at {}: {:04x}:{:04x} (class = {}, sub = {}, interface = {}) => {:?}",
            descriptor.address,
            descriptor.vendor_id,
            descriptor.device_id,
            descriptor.class,
            descriptor.sub_class,
            descriptor.interface,
            device_type,
        );
        if device_type == DeviceType::UsbController {
            info!("USB controller type: {:?}", UsbType::try_from(descriptor.interface).unwrap());
        }

        /*
         * Register the device with the Platform Bus.
         */
        let name = "pci-".to_string() + &descriptor.address.to_string();
        let device_info = {
            let mut properties = BTreeMap::new();
            properties.insert("pci.vendor_id".to_string(), Property::Integer(descriptor.vendor_id as u64));
            properties.insert("pci.device_id".to_string(), Property::Integer(descriptor.device_id as u64));
            properties.insert("pci.class".to_string(), Property::Integer(descriptor.class as u64));
            properties.insert("pci.sub_class".to_string(), Property::Integer(descriptor.sub_class as u64));
            properties.insert("pci.interface".to_string(), Property::Integer(descriptor.interface as u64));
            DeviceInfo(properties)
        };
        let handoff_info = {
            let mut properties = BTreeMap::new();

            if let Some(interrupt) = descriptor.interrupt {
                properties.insert("pci.interrupt".to_string(), HandoffProperty::Event(interrupt));
            }

            for (i, bar) in descriptor.bars.into_iter().enumerate() {
                if let Some(bar) = bar {
                    match bar {
                        Bar::Memory32 { memory_object, size } => {
                            properties.insert(
                                format!("pci.bar{}.handle", i),
                                HandoffProperty::MemoryObject(memory_object),
                            );
                            properties.insert(format!("pci.bar{}.size", i), HandoffProperty::Integer(size as u64));
                        }
                        Bar::Memory64 { memory_object, size } => {
                            properties.insert(
                                format!("pci.bar{}.handle", i),
                                HandoffProperty::MemoryObject(memory_object),
                            );
                            properties.insert(format!("pci.bar{}.size", i), HandoffProperty::Integer(size));
                        }
                    }
                }
            }

            HandoffInfo(properties)
        };
        platform_bus_channel.send(&BusDriverMessage::RegisterDevice(name, device_info, handoff_info)).unwrap();
    }

    loop {
        syscall::yield_to_kernel();
    }
}

#[used]
#[link_section = ".caps"]
pub static mut CAPS: CapabilitiesRepr<4> =
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_PCI_BUS_DRIVER, CAP_SERVICE_USER, CAP_PADDING]);
