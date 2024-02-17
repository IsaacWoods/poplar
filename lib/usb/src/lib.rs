#![no_std]

pub mod descriptor;
pub mod hid;
pub mod setup;

use core::mem;
use descriptor::{ConfigurationDescriptor, DescriptorBase, EndpointDescriptor, InterfaceDescriptor};

/// Used to 'visit' each descriptor within a configuration hierachy. Used with
/// [`walk_configuration`].
pub trait ConfigurationVisitor {
    fn visit_configuration(&mut self, descriptor: &ConfigurationDescriptor) {}
    fn visit_interface(&mut self, descriptor: &InterfaceDescriptor) {}
    fn visit_endpoint(&mut self, descriptor: &EndpointDescriptor) {}
    fn visit_other(&mut self, descriptor_typ: u8, bytes: &[u8]) {}
}

/// Walk a configuration descriptor hierachy for a device. This should be passed the bytes received
/// after requesting a full Configuration descriptor from a device, and will call the passed
/// [`ConfigurationVisitor`] for each contained descriptor.
///
/// A conformant device will have a Configuration Descriptor, followed by Interface Descriptors,
/// which may potentially each have one or more Endpoint Descriptors. Endpoint descriptors pertain
/// to the Interface descriptor that they follow. Class and vendor-specific descriptors can be
/// interleaved with the standard descriptors - [`ConfigurationVisitor::visit_other`] will be called
/// for each of them.
pub fn walk_configuration(bytes: &[u8], visitor: &mut impl ConfigurationVisitor) {
    fn at<T>(bytes: &[u8], offset: usize) -> &T {
        assert!(offset + mem::size_of::<T>() <= bytes.len());
        unsafe { &*(bytes.as_ptr().byte_add(offset) as *const T) }
    }

    // Start with the configuration descriptor at the start
    let configuration_descriptor = at::<ConfigurationDescriptor>(bytes, 0);
    visitor.visit_configuration(configuration_descriptor);

    // Then any subsequent descriptors
    let mut offset = configuration_descriptor.length as usize;
    while offset < configuration_descriptor.total_length as usize {
        let base = at::<DescriptorBase>(bytes, offset);

        match base.typ {
            4 => {
                let interface_descriptor = at::<InterfaceDescriptor>(bytes, offset);
                visitor.visit_interface(interface_descriptor);
            }
            5 => {
                let endpoint_descriptor = at::<EndpointDescriptor>(bytes, offset);
                visitor.visit_endpoint(endpoint_descriptor);
            }
            other => {
                let descriptor_bytes = &bytes[offset..(offset + base.length as usize)];
                visitor.visit_other(other, descriptor_bytes);
            }
        }
        offset += base.length as usize;
    }
}
