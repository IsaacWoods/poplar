use crate::Device;
use log::info;
use pci_types::device_type::{DeviceType, UsbType};
use platform_bus::{DeviceInfo, HandoffInfo, HandoffProperty, Property};
use std::{collections::BTreeMap, poplar::ddk::pci::Bar};

pub fn enumerate_pci_devices() -> BTreeMap<String, Device> {
    let mut devices = BTreeMap::new();
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
                properties.insert("pci.interrupt".to_string(), HandoffProperty::Interrupt(interrupt));
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

        devices.insert(name, Device::Unclaimed { bus_driver: crate::KERNEL_DEVICE, device_info, handoff_info });
    }

    devices
}
