#![feature(never_type)]

use core::sync::atomic::{AtomicUsize, Ordering};
use log::info;
use platform_bus::{
    BusDriverMessage,
    DeviceDriverMessage,
    DeviceDriverRequest,
    DeviceInfo,
    Filter,
    HandoffInfo,
    HandoffProperty,
    Property,
};
use std::{
    collections::BTreeMap,
    mem::{self, MaybeUninit},
    poplar::{
        caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING, CAP_SERVICE_USER},
        channel::Channel,
        ddk::dma::DmaPool,
        early_logger::EarlyLogger,
        memory_object::{MappedMemoryObject, MemoryObject},
        syscall::{self, MemoryObjectFlags},
    },
};
use virtio::{
    gpu::{
        CreateResource2D,
        CtrlHeader,
        CtrlType,
        DisplayInfo,
        FlushResource,
        SetScanout,
        SimpleResourceAttachBacking,
        TransferToHost2D,
        VirtioGpuFormat,
    },
    pci::VirtioPciCommonCfg,
    virtqueue::Virtqueue,
    StatusFlags,
};

/*
 * TODO: these have to be extracted from custom PCI capabilities. Eventually, we'll want to
 * pass the config space out to userspace and parse the capability list as needed from the
 * driver, but for now we've just extracted this in the kernel and defined these constants here
 * to reflect the BAR layout. These represent offsets into BAR4, and each region is 0x1000
 * long.
 */
const COMMON_CFG_OFFSET: usize = 0;
const ISR_CFG_OFFSET: usize = 0x1000;
const DEVICE_CFG_OFFSET: usize = 0x2000;
const NOTIFY_CFG_OFFSET: usize = 0x3000;

pub type ResourceIndex = u32;
pub struct ScanoutInfo {
    width: u32,
    height: u32,
    scanout_id: u32,
}

pub struct VirtioGpu<'a> {
    mapped_bar: MappedMemoryObject,
    // TODO: This is located in `mapped_bar`, so we need to be very careful not to create aliasing
    // references! This might be safer if we created ad-hoc references to this as needed?
    common_cfg: &'a mut VirtioPciCommonCfg,
    queue: Virtqueue,
    request_pool: DmaPool,
    next_resource_id: ResourceIndex,
}

impl<'a> VirtioGpu<'a> {
    pub fn new(
        mapped_bar: MappedMemoryObject,
        common_cfg: &'a mut VirtioPciCommonCfg,
        queue: Virtqueue,
        request_pool: DmaPool,
    ) -> VirtioGpu<'a> {
        VirtioGpu { mapped_bar, common_cfg, queue, request_pool, next_resource_id: 1 }
    }

    pub fn get_scanout_info(&mut self) -> ScanoutInfo {
        let response: DisplayInfo = self.make_request(CtrlHeader::new(CtrlType::CmdGetDisplayInfo));
        assert!(response.header.typ == CtrlType::OkDisplayInfo);
        // XXX: we'll only support one display for now, so find the first enabled scanout
        let (scanout_id, mode) = response.modes.iter().enumerate().find(|(_, mode)| mode.enabled != 0).unwrap();
        info!("Display info: {:?}", mode);
        // TODO: we can actually just ignore this and set w/h to whatever we want which is nice too
        ScanoutInfo { width: mode.width, height: mode.height, scanout_id: scanout_id as u32 }
        // ScanoutInfo { width: 800, height: 600, scanout_id: scanout_id as u32 }
    }

    pub fn create_resource(&mut self, format: VirtioGpuFormat, width: u32, height: u32) -> ResourceIndex {
        let id = self.next_resource_id;
        self.next_resource_id += 1;
        let response: CtrlHeader = self.make_request(CreateResource2D::new(id, format, width, height));
        if response.typ != CtrlType::OkNoData {
            panic!("Error creating GPU resource: {:?}", response.typ);
        }
        id
    }

    pub fn attach_backing(&mut self, resource: ResourceIndex, address: u64, length: u32) {
        let response: CtrlHeader = self.make_request(SimpleResourceAttachBacking::new(resource, address, length));
        if response.typ != CtrlType::OkNoData {
            panic!("Error attaching backing for GPU resource: {:?}", response.typ);
        }
    }

    pub fn set_scanout(&mut self, scanout_info: &ScanoutInfo, framebuffer: ResourceIndex) {
        let response: CtrlHeader = self.make_request(SetScanout::new(
            scanout_info.width,
            scanout_info.height,
            scanout_info.scanout_id,
            framebuffer,
        ));
        if response.typ != CtrlType::OkNoData {
            panic!("Error setting scanout: {:?}", response.typ);
        }
    }

    pub fn transfer_to_host_2d(&mut self, resource: ResourceIndex, width: u32, height: u32) {
        let response: CtrlHeader = self.make_request(TransferToHost2D::new(width, height, 0, resource));
        if response.typ != CtrlType::OkNoData {
            panic!("Error transfering resource to host (2D): {:?}", response.typ);
        }
    }

    pub fn flush_resource(&mut self, resource: ResourceIndex, width: u32, height: u32) {
        let response: CtrlHeader = self.make_request(FlushResource::new(resource, width, height));
        if response.typ != CtrlType::OkNoData {
            panic!("Error flushing resource: {:?}", response.typ);
        }
    }

    fn make_request<T, R>(&mut self, request: T) -> R {
        use virtio::virtqueue::{Descriptor, DescriptorFlags};

        let request = self.request_pool.create(request).unwrap();
        let response = self.request_pool.create(MaybeUninit::<R>::uninit()).unwrap();

        let descriptor_0 = self.queue.alloc_descriptor().unwrap();
        let descriptor_1 = self.queue.alloc_descriptor().unwrap();

        self.queue.push_descriptor(
            descriptor_0,
            Descriptor {
                address: request.phys as u64,
                len: mem::size_of::<T>() as u32,
                flags: DescriptorFlags::NEXT,
                next: descriptor_1,
            },
        );
        self.queue.push_descriptor(
            descriptor_1,
            Descriptor {
                address: response.phys as u64,
                len: mem::size_of::<R>() as u32,
                flags: DescriptorFlags::WRITE,
                next: 0,
            },
        );

        self.queue.make_descriptor_available(descriptor_0);

        unsafe {
            core::arch::asm!("fence ow, ow");
        }

        /*
         * TODO: notifying the device of an available virtqueue is much harder via PCI than MMIO -
         * we need to
         * calculate an address in the BAR by reading a queue offset from the common cfg, then
         * multiply that by a multiplier from a PCI capability. For now, we're only using virtqueue
         * 0, so we'll cheat a little. Doing this properly will be needed to access the cursor
         * queue.
         */
        let notify_address = self.mapped_bar.mapped_at + NOTIFY_CFG_OFFSET + 0;
        unsafe {
            std::ptr::write_volatile(notify_address as *mut u16, 0);
        }

        for _ in 0..10 {
            // TODO: this is an immensely hacky way to hopefully give enough time for the device to
            // respond - this should obviously be changed to wait til the request has been
            // serviced.
            std::poplar::syscall::yield_to_kernel();
        }

        unsafe { std::ptr::read(response.ptr.as_ptr()).assume_init() }
    }
}

fn main() {
    log::set_logger(&EarlyLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("Virtio GPU driver is running!");

    // We act as a bus driver to create the framebuffer device
    let platform_bus_bus_channel: Channel<BusDriverMessage, !> =
        Channel::subscribe_to_service("platform_bus.bus_driver").unwrap();
    // And also as a device driver to find Virtio GPU devices
    let platform_bus_device_channel: Channel<DeviceDriverMessage, DeviceDriverRequest> =
        Channel::subscribe_to_service("platform_bus.device_driver").unwrap();
    platform_bus_device_channel
        .send(&DeviceDriverMessage::RegisterInterest(vec![
            Filter::Matches(String::from("pci.vendor_id"), Property::Integer(0x1af4)),
            Filter::Matches(String::from("pci.device_id"), Property::Integer(0x1050)),
        ]))
        .unwrap();

    let (device_info, handoff_info) = loop {
        match platform_bus_device_channel.try_receive().unwrap() {
            Some(DeviceDriverRequest::QuerySupport(name, _)) => {
                platform_bus_device_channel.send(&DeviceDriverMessage::CanSupport(name, true)).unwrap();
            }
            Some(DeviceDriverRequest::HandoffDevice(name, device_info, handoff_info)) => {
                info!("Started driving device: {}", name);
                break (device_info, handoff_info);
            }
            None => syscall::yield_to_kernel(),
        }
    };

    let mapped_bar = {
        // TODO: let the kernel choose the address when it can - we don't care
        // TODO: maybe confirm info from platform_bus
        let bar = MemoryObject {
            handle: handoff_info.get_as_memory_object("pci.bar4.handle").unwrap(),
            size: handoff_info.get_as_integer("pci.bar4.size").unwrap() as usize,
            flags: MemoryObjectFlags::WRITABLE,
            phys_address: None,
        };
        const BAR_SPACE_ADDRESS: usize = 0x00000005_00000000;
        unsafe { bar.map_at(BAR_SPACE_ADDRESS).unwrap() }
    };

    let memory_manager = VirtioMemoryManager::new();
    let queue = Virtqueue::new(64, &memory_manager);
    let request_pool = {
        let memory_object = unsafe { MemoryObject::create_physical(0x1000, MemoryObjectFlags::WRITABLE).unwrap() };
        const REQUEST_POOL_ADDRESS: usize = 0x00000005_20000000;
        let memory_object = unsafe { memory_object.map_at(REQUEST_POOL_ADDRESS).unwrap() };
        DmaPool::new(memory_object)
    };

    let common_cfg = unsafe { &mut *(mapped_bar.ptr().byte_add(COMMON_CFG_OFFSET) as *mut VirtioPciCommonCfg) };
    common_cfg.reset();
    common_cfg.set_status_flag(StatusFlags::Acknowledge);
    common_cfg.set_status_flag(StatusFlags::Driver);

    common_cfg.set_status_flag(virtio::StatusFlags::FeaturesOk);
    assert!(common_cfg.is_status_flag_set(virtio::StatusFlags::FeaturesOk));

    common_cfg.select_queue(0);
    common_cfg.set_queue_size(64);
    common_cfg.set_queue_descriptor(queue.descriptor_table.physical as u64);
    common_cfg.set_queue_driver(queue.available_ring.physical as u64);
    common_cfg.set_queue_device(queue.used_ring.physical as u64);
    common_cfg.mark_queue_ready();

    common_cfg.set_status_flag(virtio::StatusFlags::DriverOk);

    if common_cfg.is_status_flag_set(virtio::StatusFlags::Failed) {
        panic!("Virtio device initialization failed");
    }
    assert!(common_cfg.num_queues.read() == 2);

    let mut gpu = VirtioGpu::new(mapped_bar, common_cfg, queue, request_pool);
    let scanout_info = gpu.get_scanout_info();
    let framebuffer_resource =
        gpu.create_resource(VirtioGpuFormat::R8G8B8X8Unorm, scanout_info.width, scanout_info.height);

    // Allocate guest memory for the framebuffer
    let framebuffer_size = scanout_info.width * scanout_info.height * 4;
    let framebuffer = {
        let memory_object = unsafe {
            MemoryObject::create_physical(framebuffer_size as usize, MemoryObjectFlags::WRITABLE).unwrap()
        };
        const FRAMEBUFFER_ADDDRESS: usize = 0x00000005_30000000;
        unsafe { memory_object.map_at(FRAMEBUFFER_ADDDRESS).unwrap() }
    };
    gpu.attach_backing(framebuffer_resource, framebuffer.inner.phys_address.unwrap() as u64, framebuffer_size);
    gpu.set_scanout(&scanout_info, framebuffer_resource);

    let framebuffer_base = framebuffer.ptr() as *mut u32;
    for y in 0..scanout_info.height {
        for x in 0..scanout_info.width {
            unsafe {
                std::ptr::write_volatile(framebuffer_base.add((y * scanout_info.width + x) as usize), 0xffff00ff);
            }
        }
    }

    // Flush the framebuffer to the host for the first time
    gpu.transfer_to_host_2d(framebuffer_resource, scanout_info.width, scanout_info.height);
    gpu.flush_resource(framebuffer_resource, scanout_info.width, scanout_info.height);

    // Add the framebuffer as a device to the Platform Bus
    let channel = {
        let device_info = {
            let mut properties = BTreeMap::new();
            properties.insert("type".to_string(), Property::String("framebuffer".to_string()));
            properties.insert("width".to_string(), Property::Integer(scanout_info.width as u64));
            properties.insert("height".to_string(), Property::Integer(scanout_info.height as u64));
            DeviceInfo(properties)
        };
        let (control_channel, control_channel_handle) = Channel::<(), ()>::create().unwrap();
        let handoff_info = {
            let mut properties = BTreeMap::new();
            properties.insert("framebuffer".to_string(), HandoffProperty::MemoryObject(framebuffer.inner.handle));
            properties.insert("channel".to_string(), HandoffProperty::Channel(control_channel_handle));
            HandoffInfo(properties)
        };
        platform_bus_bus_channel
            .send(&BusDriverMessage::RegisterDevice("virtio-fb".to_string(), device_info, handoff_info))
            .unwrap();
        control_channel
    };

    loop {
        match channel.try_receive() {
            Ok(Some(message)) => {
                // Flush the entire framebuffer to the host
                gpu.transfer_to_host_2d(framebuffer_resource, scanout_info.width, scanout_info.height);
                gpu.flush_resource(framebuffer_resource, scanout_info.width, scanout_info.height);
            }
            Ok(None) => std::poplar::syscall::yield_to_kernel(),
            Err(err) => panic!("Error receiving message from control channel: {:?}", err),
        }
    }
}

pub struct VirtioMemoryManager {
    area: MappedMemoryObject,
    offset: AtomicUsize,
}

impl VirtioMemoryManager {
    pub fn new() -> VirtioMemoryManager {
        let memory_object = unsafe { MemoryObject::create_physical(0x1000, MemoryObjectFlags::WRITABLE).unwrap() };
        const QUEUE_AREA_ADDRESS: usize = 0x00000005_10000000;
        let memory_object = unsafe { memory_object.map_at(QUEUE_AREA_ADDRESS).unwrap() };

        VirtioMemoryManager { area: memory_object, offset: AtomicUsize::new(0) }
    }
}

impl virtio::virtqueue::Mapper for VirtioMemoryManager {
    fn alloc(&self, size: usize) -> (usize, usize) {
        let virt = self.area.mapped_at + self.offset.fetch_add(size, Ordering::Relaxed);
        (self.area.virt_to_phys(virt).unwrap(), virt)
    }
}

#[used]
#[link_section = ".caps"]
pub static mut CAPS: CapabilitiesRepr<4> =
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_SERVICE_USER, CAP_PADDING, CAP_PADDING]);
