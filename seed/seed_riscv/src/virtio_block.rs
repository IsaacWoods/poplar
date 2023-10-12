use crate::memory::MemoryManager;
use core::{ptr, ptr::NonNull};
use tracing::error;
use virtio::{
    block::BlockDeviceConfig,
    virtqueue::{Mapped, Virtqueue},
};

pub struct VirtioBlockDevice<'a> {
    device: &'a mut BlockDeviceConfig,
    queue: Virtqueue,
    request_buffer: Mapped<[virtio::block::Request]>,
    data_buffer: Mapped<[[u8; 512]]>,
}

impl<'a> VirtioBlockDevice<'a> {
    pub fn init(device: &'a mut BlockDeviceConfig, memory_manager: &MemoryManager) -> VirtioBlockDevice<'a> {
        // TODO: deal with freeing this memory somehow once we're done with the device (probably
        // need to reset it too)
        let queue = Virtqueue::new(64, memory_manager);
        let request_buffer = unsafe { Mapped::<[virtio::block::Request]>::new(64, memory_manager) };
        let data_buffer = unsafe { Mapped::<[[u8; 512]]>::new(512, memory_manager) };

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

        VirtioBlockDevice { device, queue, request_buffer, data_buffer }
    }

    pub fn read(&mut self, sector: u64) -> NonNull<[u8; 512]> {
        // TODO: actually manage free request slots etc. - this just overwrites the last-read sector atm lmao
        let (request_phys, request_virt) = self.request_buffer.get(0).unwrap();
        unsafe {
            ptr::write_volatile(request_virt.as_ptr(), virtio::block::Request::read(sector));
        }

        let (data_phys, data_virt) = self.data_buffer.get(0).unwrap();

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

        // TODO: read the status of the request before assuming data is good
        data_virt
    }
}
