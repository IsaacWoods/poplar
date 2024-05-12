use crate::{
    block::{BlockDevice, ReadToken},
    memory::MemoryManager,
};
use alloc::collections::VecDeque;
use core::ptr;
use fdt::Fdt;
use tracing::error;
use virtio::{
    block::BlockDeviceConfig,
    virtqueue::{Mapped, Virtqueue},
    DeviceType,
    mmio::VirtioMmioHeader,
};

pub struct VirtioBlockDevice<'a> {
    device: &'a mut BlockDeviceConfig,
    queue: Virtqueue,
    request_buffer: Mapped<[virtio::block::Request]>,
    free_request_slots: VecDeque<u16>,
    data_buffer: Mapped<[[u8; 512]]>,
    free_data_slots: VecDeque<u16>,
}

impl<'a> VirtioBlockDevice<'a> {
    /// Find a Virtio block device, if present, and initialize it.
    pub fn init(fdt: &Fdt, memory_manager: &MemoryManager) -> Option<VirtioBlockDevice<'a>> {
        /*
         * Find the Virtio block device's config space and interrupt from the FDT
         * This shortcircuits if there isn't a Virtio block device present.
         */
        // XXX: this assumes that there is only ever one Virtio block device
        let (config_ptr, _interrupt) = fdt
            .all_nodes()
            .filter(|node| node.compatible().map_or(false, |c| c.all().any(|c| c == "virtio,mmio")))
            .find_map(|node| {
                let reg = node.reg().unwrap().next().unwrap();
                let header = unsafe { &*(reg.starting_address as *const VirtioMmioHeader) };

                if !header.is_magic_valid() {
                    return None;
                }

                match header.device_type() {
                    Ok(DeviceType::BlockDevice) => {
                        // XXX: not sure how brittle this is, but each device seems to only have one interrupt
                        let interrupt = node.interrupts().unwrap().next().unwrap();
                        Some((reg.starting_address as *mut BlockDeviceConfig, interrupt))
                    }
                    _ => None,
                }
            })?;

        let device = unsafe { &mut *config_ptr };

        // TODO: deal with freeing this memory somehow once we're done with the device (probably
        // need to reset it too)
        let queue = Virtqueue::new(64, memory_manager);
        let request_buffer = unsafe { Mapped::<[virtio::block::Request]>::new(64, memory_manager) };
        let free_request_slots = (0..64).collect();
        let data_buffer = unsafe { Mapped::<[[u8; 512]]>::new(512, memory_manager) };
        let free_data_slots = (0..512).collect();

        device.header.reset();
        device.header.set_status_flag(virtio::StatusFlags::Acknowledge);
        device.header.set_status_flag(virtio::StatusFlags::Driver);

        // TODO: actually negotiate needed features
        device.header.set_status_flag(virtio::StatusFlags::FeaturesOk);
        assert!(device.header.is_status_flag_set(virtio::StatusFlags::FeaturesOk));

        device.header.queue_select.write(0);
        device.header.queue_size.write(64);
        device.header.set_queue_descriptor(queue.descriptor_table.physical as u64);
        device.header.set_queue_driver(queue.available_ring.physical as u64);
        device.header.set_queue_device(queue.used_ring.physical as u64);
        device.header.mark_queue_ready();

        device.header.set_status_flag(virtio::StatusFlags::DriverOk);

        if device.header.is_status_flag_set(virtio::StatusFlags::Failed) {
            error!("Virtio device initialization failed");
        }

        Some(VirtioBlockDevice { device, queue, request_buffer, free_request_slots, data_buffer, free_data_slots })
    }

    fn alloc_request_slot(&mut self) -> u16 {
        self.free_request_slots.pop_back().expect("Too many requests in-flight!")
    }

    fn free_request_slot(&mut self, slot: u16) {
        self.free_request_slots.push_back(slot);
    }

    fn alloc_data_slot(&mut self) -> u16 {
        self.free_data_slots.pop_back().expect("Too many data sectors loaded atm!")
    }

    fn free_data_slot(&mut self, slot: u16) {
        self.free_data_slots.push_back(slot);
    }
}

pub struct ReadTokenMeta {
    data_slot: u16,
    request_slot: u16,
}

impl<'a> BlockDevice for VirtioBlockDevice<'a> {
    type ReadTokenMetadata = ReadTokenMeta;

    fn read(&mut self, block: u64) -> ReadToken<Self::ReadTokenMetadata> {
        let request_slot = self.alloc_request_slot();
        let (request_phys, request_virt) = self.request_buffer.get(request_slot as usize).unwrap();
        unsafe {
            ptr::write_volatile(request_virt.as_ptr(), virtio::block::Request::read(block));
        }

        let data_slot = self.alloc_data_slot();
        tracing::trace!("Request_slot: {}, data_slot: {}", request_slot, data_slot);
        let (data_phys, data_virt) = self.data_buffer.get(data_slot as usize).unwrap();

        use virtio::virtqueue::{Descriptor, DescriptorFlags};
        let descriptor_0 = self.queue.alloc_descriptor().unwrap();
        let descriptor_1 = self.queue.alloc_descriptor().unwrap();
        let descriptor_2 = self.queue.alloc_descriptor().unwrap();

        self.queue.push_descriptor(
            descriptor_0,
            Descriptor { address: request_phys as u64, len: 16, flags: DescriptorFlags::NEXT, next: descriptor_1 },
        );
        self.queue.push_descriptor(
            descriptor_1,
            Descriptor {
                address: data_phys as u64,
                len: 512,
                flags: DescriptorFlags::NEXT | DescriptorFlags::WRITE,
                next: descriptor_2,
            },
        );
        self.queue.push_descriptor(
            descriptor_2,
            Descriptor { address: request_phys as u64 + 16, len: 1, flags: DescriptorFlags::WRITE, next: 0 },
        );

        self.queue.make_descriptor_available(descriptor_0);

        unsafe {
            core::arch::asm!("fence ow, ow");
        }
        self.device.header.queue_notify.write(0);

        // XXX: this is a load-bearing print. We don't actually check that the request has been serviced before
        // returning the data - this uses enough cycles that it's probably done before we return.
        tracing::info!("Read data: {:?}", unsafe { data_virt.as_ref() });
        // TODO: read the status of the request before assuming data is good
        ReadToken { data: data_virt, meta: ReadTokenMeta { data_slot, request_slot } }
    }

    fn free_read_block(&mut self, token: ReadToken<Self::ReadTokenMetadata>) {
        self.free_data_slot(token.meta.data_slot);
        self.free_request_slot(token.meta.request_slot);
    }
}
