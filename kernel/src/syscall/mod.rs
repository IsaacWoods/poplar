mod validation;

use crate::{
    object::{
        address_space::AddressSpace,
        channel::{ChannelEnd, Message},
        event::Event,
        interrupt::Interrupt,
        memory_object::MemoryObject,
        task::{Task, TaskState},
        KernelObject,
        KernelObjectType,
    },
    scheduler::Scheduler,
    vmm::Vmm,
    Platform,
};
use alloc::{string::ToString, sync::Arc};
use bit_field::BitField;
use core::{convert::TryFrom, sync::atomic::Ordering};
use hal::memory::{Flags, FrameSize, PAddr, PageTable, Size4KiB, VAddr};
use poplar::{
    syscall::{
        self,
        result::{handle_to_syscall_repr, status_to_syscall_repr, status_with_payload_to_syscall_repr},
        AckInterruptError,
        CreateAddressSpaceError,
        CreateChannelError,
        CreateMemoryObjectError,
        EarlyLogError,
        FramebufferInfo,
        GetFramebufferError,
        GetMessageError,
        MapMemoryObjectError,
        MemoryObjectFlags,
        PciGetInfoError,
        PollInterestError,
        ResizeMemoryObjectError,
        SendMessageError,
        SpawnTaskDetails,
        SpawnTaskError,
        WaitForEventError,
        WaitForInterruptError,
        CHANNEL_MAX_NUM_HANDLES,
    },
    Handle,
};
use tracing::{info, warn};
use validation::{UserPointer, UserSlice, UserString};

/// This is the architecture-independent syscall handler. It should be called by the handler that
/// receives the syscall (each architecture is free to do this however it wishes). The only
/// parameter that is guaranteed to be valid is `number`; the meaning of the rest may be undefined
/// depending on how many parameters the specific system call takes.
pub fn handle_syscall<P>(
    scheduler: &Scheduler<P>,
    vmm: &Vmm<P>,
    number: usize,
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    e: usize,
) -> usize
where
    P: Platform,
{
    // Clone the current task out of the scheduler as we can't hold a lock on the scheduler
    let task = {
        let cpu_scheduler = scheduler.for_this_cpu();
        cpu_scheduler.running_task.as_ref().unwrap().clone()
    };

    // info!(
    //     "[{}] Syscall! number = {}, a = {:#x}, b = {:#x}, c = {:#x}, d = {:#x}, e = {:#x}",
    //     task.name, number, a, b, c, d, e
    // );

    match number {
        syscall::SYSCALL_YIELD => yield_syscall(scheduler),
        syscall::SYSCALL_EARLY_LOG => status_to_syscall_repr(early_log(&task, a, b)),
        syscall::SYSCALL_GET_FRAMEBUFFER => handle_to_syscall_repr(get_framebuffer(&task, a)),
        syscall::SYSCALL_CREATE_MEMORY_OBJECT => handle_to_syscall_repr(create_memory_object(&task, a, b, c)),
        syscall::SYSCALL_MAP_MEMORY_OBJECT => status_to_syscall_repr(map_memory_object(&task, a, b, c, d)),
        syscall::SYSCALL_CREATE_CHANNEL => handle_to_syscall_repr(create_channel(&task, a)),
        syscall::SYSCALL_SEND_MESSAGE => status_to_syscall_repr(send_message(&task, a, b, c, d, e)),
        syscall::SYSCALL_GET_MESSAGE => status_with_payload_to_syscall_repr(get_message(&task, a, b, c, d, e)),
        syscall::SYSCALL_WAIT_FOR_MESSAGE => todo!(),
        syscall::SYSCALL_PCI_GET_INFO => status_with_payload_to_syscall_repr(pci_get_info(&task, a, b)),
        syscall::SYSCALL_WAIT_FOR_EVENT => status_to_syscall_repr(wait_for_event(scheduler, &task, a, b)),
        syscall::SYSCALL_POLL_INTEREST => status_with_payload_to_syscall_repr(poll_interest(&task, a)),
        syscall::SYSCALL_CREATE_ADDRESS_SPACE => handle_to_syscall_repr(create_address_space(&task)),
        syscall::SYSCALL_SPAWN_TASK => handle_to_syscall_repr(spawn_task(&task, a, scheduler, vmm)),
        syscall::SYSCALL_RESIZE_MEMORY_OBJECT => status_to_syscall_repr(resize_memory_object(&task, a, b)),
        syscall::SYSCALL_WAIT_FOR_INTERRUPT => status_to_syscall_repr(wait_for_interrupt(scheduler, &task, a, b)),
        syscall::SYSCALL_ACK_INTERRUPT => status_to_syscall_repr(ack_interrupt(&task, a)),

        _ => {
            warn!("Process made system call with invalid syscall number: {}", number);
            usize::MAX
        }
    }
}

fn yield_syscall<P>(scheduler: &Scheduler<P>) -> usize
where
    P: Platform,
{
    scheduler.schedule(TaskState::Ready);
    0
}

fn early_log<P>(task: &Arc<Task<P>>, str_length: usize, str_address: usize) -> Result<(), EarlyLogError>
where
    P: Platform,
{
    // Check if the message is too long
    if str_length > 8192 {
        return Err(EarlyLogError::MessageTooLong);
    }

    // Check the message is valid UTF-8
    let message = UserString::new(str_address as *mut u8, str_length)
        .validate()
        .map_err(|_| EarlyLogError::MessageNotValidUtf8)?;

    info!("[{}]: {}", task.name, message);
    Ok(())
}

fn get_framebuffer<P>(task: &Arc<Task<P>>, info_address: usize) -> Result<Handle, GetFramebufferError>
where
    P: Platform,
{
    let (info, memory_object) = crate::FRAMEBUFFER.try_get().ok_or(GetFramebufferError::NoFramebufferCreated)?;
    let handle = task.handles.add(memory_object.clone());

    UserPointer::new(info_address as *mut FramebufferInfo, true)
        .validate_write(*info)
        .map_err(|()| GetFramebufferError::InfoAddressIsInvalid)?;

    Ok(handle)
}

fn create_memory_object<P>(
    task: &Arc<Task<P>>,
    size: usize,
    flags: usize,
    physical_address_ptr: usize,
) -> Result<Handle, CreateMemoryObjectError>
where
    P: Platform,
{
    use mulch::math::align_up;

    // TODO: should we require that the size be multiple of the page size, or just up it here?
    let size = align_up(size, Size4KiB::SIZE);
    let flags = MemoryObjectFlags::from_bits_truncate(flags as u32);

    // TODO: do something more sensible with this when we have a concept of physical memory "ownership"
    assert!(size % Size4KiB::SIZE == 0);
    let physical_start = crate::PMM.get().alloc(size / Size4KiB::SIZE);

    let memory_object = MemoryObject::new(
        task.id(),
        physical_start,
        size,
        Flags {
            writable: flags.contains(MemoryObjectFlags::WRITABLE),
            executable: flags.contains(MemoryObjectFlags::EXECUTABLE),
            user_accessible: true,
            ..Default::default()
        },
    );

    if physical_address_ptr != 0x0 {
        UserPointer::new(physical_address_ptr as *mut PAddr, true)
            .validate_write(physical_start)
            .map_err(|()| CreateMemoryObjectError::InvalidPhysicalAddressPointer)?;
    }

    Ok(task.handles.add(memory_object))
}

fn map_memory_object<P>(
    task: &Arc<Task<P>>,
    memory_object_handle: usize,
    address_space_handle: usize,
    virtual_address: usize,
    address_ptr: usize,
) -> Result<(), MapMemoryObjectError>
where
    P: Platform,
{
    let memory_object_handle =
        Handle::try_from(memory_object_handle).map_err(|_| MapMemoryObjectError::InvalidMemoryObjectHandle)?;
    let address_space_handle =
        Handle::try_from(address_space_handle).map_err(|_| MapMemoryObjectError::InvalidAddressSpaceHandle)?;

    let memory_object = task
        .handles
        .get(memory_object_handle)
        .ok_or(MapMemoryObjectError::InvalidMemoryObjectHandle)?
        .downcast_arc::<MemoryObject>()
        .ok()
        .ok_or(MapMemoryObjectError::InvalidMemoryObjectHandle)?;

    let (virtual_address, write_to_ptr) = if virtual_address == 0x0 {
        /*
         * No virtual address supplied: we should find a suitable area of the virtual address space
         * to map the object to, and write the address to the supplied pointer.
         */
        todo!()
    } else {
        // TODO: we need to actually validate that the supplied address is canonical and all that jazz
        (VAddr::new(virtual_address), false)
    };

    if address_space_handle == Handle::ZERO {
        /*
         * If the AddressSpace handle is the zero handle, we map the MemoryObject into the calling task's
         * address space.
         */
        task.address_space.map_memory_object(memory_object.clone(), virtual_address, &crate::PMM.get())?;
    } else {
        task.handles
            .get(address_space_handle)
            .ok_or(MapMemoryObjectError::InvalidAddressSpaceHandle)?
            .downcast_arc::<AddressSpace<P>>()
            .ok()
            .ok_or(MapMemoryObjectError::InvalidAddressSpaceHandle)?
            .map_memory_object(memory_object.clone(), virtual_address, &crate::PMM.get())?;
    }

    /*
     * Only write to the pointer if: 1) we had to allocate an address 2) the caller wants to know,
     * and 3) the mapping actually succeeded.
     */
    if write_to_ptr && address_ptr != 0x0 {
        let mut address_ptr = UserPointer::new(address_ptr as *mut VAddr, true);
        address_ptr.validate_write(virtual_address).map_err(|()| MapMemoryObjectError::AddressPointerInvalid)?;
    }

    Ok(())
}

fn create_channel<P>(task: &Arc<Task<P>>, other_end_address: usize) -> Result<Handle, CreateChannelError>
where
    P: Platform,
{
    let (end_a, end_b) = ChannelEnd::new_channel(task.id());
    let end_a_handle = task.handles.add(end_a);
    let end_b_handle = task.handles.add(end_b);

    let mut other_end_ptr = UserPointer::new(other_end_address as *mut Handle, true);
    other_end_ptr.validate_write(end_b_handle).map_err(|()| CreateChannelError::InvalidHandleAddress)?;

    Ok(end_a_handle)
}

fn send_message<P>(
    task: &Arc<Task<P>>,
    channel_handle: usize,
    byte_address: usize,
    num_bytes: usize,
    handles_address: usize,
    num_handles: usize,
) -> Result<(), SendMessageError>
where
    P: Platform,
{
    use poplar::syscall::CHANNEL_MAX_NUM_BYTES;

    if num_bytes > CHANNEL_MAX_NUM_BYTES {
        return Err(SendMessageError::TooManyBytes);
    }
    if num_handles > CHANNEL_MAX_NUM_HANDLES {
        return Err(SendMessageError::TooManyHandles);
    }

    let channel_handle = Handle::try_from(channel_handle).map_err(|_| SendMessageError::InvalidChannelHandle)?;
    let bytes = if num_bytes == 0 {
        &[]
    } else {
        UserSlice::new(byte_address as *mut u8, num_bytes)
            .validate_read()
            .map_err(|()| SendMessageError::BytesAddressInvalid)?
    };
    let handles = if num_handles == 0 {
        &[]
    } else {
        UserSlice::new(handles_address as *mut Handle, num_handles)
            .validate_read()
            .map_err(|()| SendMessageError::HandlesAddressInvalid)?
    };
    let handle_objects = {
        let mut arr = [const { None }; CHANNEL_MAX_NUM_HANDLES];
        for (i, handle) in handles.iter().enumerate() {
            arr[i] = match task.handles.get(*handle) {
                Some(object) => Some(object.clone()),
                None => return Err(SendMessageError::InvalidTransferredHandle),
            };

            /*
             * We're transferring the handle's object, so we remove the handle to it from the sending task.
             */
            task.handles.remove(*handle);
        }
        arr
    };

    task.handles
        .get(channel_handle)
        .ok_or(SendMessageError::InvalidChannelHandle)?
        .downcast_arc::<ChannelEnd>()
        .ok()
        .ok_or(SendMessageError::NotAChannel)?
        .send(Message { bytes: bytes.to_vec(), handle_objects })
}

fn get_message<P>(
    task: &Arc<Task<P>>,
    channel_handle: usize,
    bytes_address: usize,
    bytes_len: usize,
    handles_address: usize,
    handles_len: usize,
) -> Result<usize, GetMessageError>
where
    P: Platform,
{
    let channel_handle = Handle::try_from(channel_handle).map_err(|_| GetMessageError::InvalidChannelHandle)?;

    let channel = task
        .handles
        .get(channel_handle)
        .ok_or(GetMessageError::InvalidChannelHandle)?
        .downcast_arc::<ChannelEnd>()
        .ok()
        .ok_or(GetMessageError::NotAChannel)?;

    channel.receive(|message| {
        let num_handles = message.num_handles();

        if message.bytes.len() > bytes_len {
            return Err((message, GetMessageError::BytesBufferTooSmall));
        }
        if num_handles > handles_len {
            return Err((message, GetMessageError::HandlesBufferTooSmall));
        }

        if bytes_len > 0 && bytes_address != 0x0 {
            let byte_buffer = match UserSlice::new(bytes_address as *mut u8, message.bytes.len()).validate_write()
            {
                Ok(buffer) => buffer,
                Err(()) => return Err((message, GetMessageError::BytesAddressInvalid)),
            };
            byte_buffer.copy_from_slice(&message.bytes);
        }

        if handles_len > 0 && handles_address != 0x0 {
            let handles_buffer = match UserSlice::new(handles_address as *mut Handle, num_handles).validate_write()
            {
                Ok(buffer) => buffer,
                Err(()) => return Err((message, GetMessageError::HandlesAddressInvalid)),
            };
            for i in 0..num_handles {
                handles_buffer[i] = task.handles.add(message.handle_objects[i].as_ref().unwrap().clone());
            }
        }

        let mut status = 0;
        status.set_bits(16..32, message.bytes.len());
        status.set_bits(32..48, num_handles);
        Ok(status)
    })
}

fn pci_get_info<P>(
    task: &Arc<Task<P>>,
    buffer_address: usize,
    buffer_size: usize,
) -> Result<usize, PciGetInfoError>
where
    P: Platform,
{
    use pci_types::{Bar, MAX_BARS};
    use poplar::ddk::pci::PciDeviceInfo;

    // TODO: request this through the platform nicely instead of through a huge global
    if let Some(ref pci_info) = *crate::PCI_INFO.read() {
        let num_descriptors = pci_info.devices.len();

        if buffer_size > 0 && buffer_address != 0x0 {
            if buffer_size < num_descriptors {
                return Err(PciGetInfoError::BufferNotLargeEnough(num_descriptors as u32));
            }

            let descriptor_buffer = UserSlice::new(buffer_address as *mut PciDeviceInfo, buffer_size)
                .validate_write()
                .map_err(|()| PciGetInfoError::BufferPointerInvalid)?;

            for (i, (&address, device)) in pci_info.devices.iter().enumerate() {
                let interrupt_handle = device.interrupt.clone().map(|interrupt| task.handles.add(interrupt));

                let mut device_descriptor = poplar::ddk::pci::PciDeviceInfo {
                    address,
                    vendor_id: device.vendor_id,
                    device_id: device.device_id,
                    revision: device.revision,
                    class: device.class,
                    sub_class: device.sub_class,
                    interface: device.interface,
                    bars: [const { None }; MAX_BARS],
                    interrupt: interrupt_handle,
                };

                for i in 0..MAX_BARS {
                    match device.bars[i] {
                        Some(Bar::Memory32 { address, size, prefetchable }) => {
                            let flags = Flags {
                                writable: true,
                                executable: false,
                                user_accessible: true,
                                cached: prefetchable,
                            };
                            // TODO: should the requesting task own the BAR memory objects, or should the kernel?
                            let memory_object = MemoryObject::new(
                                task.id(),
                                PAddr::new(address as usize).unwrap(),
                                size as usize,
                                flags,
                            );
                            let handle = task.handles.add(memory_object);
                            device_descriptor.bars[i] =
                                Some(poplar::ddk::pci::Bar::Memory32 { memory_object: handle, size });
                        }
                        Some(Bar::Memory64 { address, size, prefetchable }) => {
                            let flags = Flags {
                                writable: true,
                                executable: false,
                                user_accessible: true,
                                cached: prefetchable,
                            };
                            // TODO: should the requesting task own the BAR memory objects, or should the kernel?
                            let memory_object = MemoryObject::new(
                                task.id(),
                                PAddr::new(address as usize).unwrap(),
                                size as usize,
                                flags,
                            );
                            let handle = task.handles.add(memory_object);
                            device_descriptor.bars[i] =
                                Some(poplar::ddk::pci::Bar::Memory64 { memory_object: handle, size });
                        }
                        Some(Bar::Io { .. }) => warn!("PCI device at {} has an I/O BAR. We don't support these, and so they're not passed out to userspace.", address),
                        None => (),
                    }
                }

                descriptor_buffer[i] = device_descriptor;
            }

            let mut status = 0;
            status.set_bits(16..48, num_descriptors);
            Ok(status)
        } else {
            Err(PciGetInfoError::BufferNotLargeEnough(num_descriptors as u32))
        }
    } else {
        Err(PciGetInfoError::PlatformDoesNotSupportPci)
    }
}

pub fn wait_for_event<P>(
    scheduler: &Scheduler<P>,
    task: &Arc<Task<P>>,
    event_handle: usize,
    block: usize,
) -> Result<(), WaitForEventError>
where
    P: Platform,
{
    let event_handle = Handle::try_from(event_handle).map_err(|_| WaitForEventError::InvalidEventHandle)?;
    let block = block != 0;
    let event = task
        .handles
        .get(event_handle)
        .ok_or(WaitForEventError::InvalidEventHandle)?
        .downcast_arc::<Event>()
        .ok()
        .ok_or(WaitForEventError::InvalidEventHandle)?;

    if block {
        /*
         * XXX: This is an extremely simple way of implementing this. We should instead probably block
         * the task, and spawn a tasklet that is awoken when the event is triggered to unblock it. For
         * now, though, this will work well enough.
         */
        while !event.signalled.load(Ordering::SeqCst) {
            scheduler.schedule(TaskState::Ready);
        }
        assert_eq!(Ok(true), event.signalled.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst));
        Ok(())
    } else {
        match event.signalled.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(true) => Ok(()),
            _ => Err(WaitForEventError::NoEvent),
        }
    }
}

pub fn poll_interest<P>(task: &Arc<Task<P>>, object_handle: usize) -> Result<usize, PollInterestError>
where
    P: Platform,
{
    let object_handle = Handle::try_from(object_handle).map_err(|_| PollInterestError::InvalidHandle)?;
    let object = task.handles.get(object_handle).ok_or(PollInterestError::InvalidHandle)?;

    let interesting = match object.typ() {
        KernelObjectType::Channel => {
            let channel = object.downcast_arc::<ChannelEnd>().ok().unwrap();
            let messages = channel.messages.lock();
            messages.len() > 0
        }
        KernelObjectType::Event => {
            let event = object.downcast_arc::<Event>().ok().unwrap();
            event.signalled.load(Ordering::SeqCst)
        }
        KernelObjectType::Interrupt => {
            let interrupt = object.downcast_arc::<Interrupt>().ok().unwrap();
            interrupt.triggered.load(Ordering::SeqCst)
        }
        _ => Err(PollInterestError::UnsupportedObjectType)?,
    };

    Ok(if interesting { 1 << 16 } else { 0 })
}

pub fn create_address_space<P>(task: &Arc<Task<P>>) -> Result<Handle, CreateAddressSpaceError>
where
    P: Platform,
{
    let address_space = AddressSpace::<P>::new(task.id());
    Ok(task.handles.add(address_space))
}

pub fn spawn_task<P>(
    task: &Arc<Task<P>>,
    details_ptr: usize,
    scheduler: &Scheduler<P>,
    vmm: &Vmm<P>,
) -> Result<Handle, SpawnTaskError>
where
    P: Platform,
{
    use crate::object::task::Handles;

    let details = UserPointer::new(details_ptr as *mut SpawnTaskDetails, false).validate_read().unwrap();

    let name = UserString::new(details.name_ptr as *mut u8, details.name_len)
        .validate()
        .map_err(|()| SpawnTaskError::InvalidTaskName)?;
    let address_space_handle =
        Handle::try_from(details.address_space as usize).map_err(|_| SpawnTaskError::NotAnAddressSpace)?;
    let address_space = task
        .handles
        .get(address_space_handle)
        .ok_or(SpawnTaskError::NotAnAddressSpace)?
        .downcast_arc::<AddressSpace<P>>()
        .ok()
        .ok_or(SpawnTaskError::NotAnAddressSpace)?;

    let handles = Handles::new();
    handles.add(address_space.clone());

    // TODO: we should really be adding the required memory objects to the task, or they could be
    // freed from under us. This could be done by convention using the object transfer array?

    let handles_to_transfer =
        UserSlice::new(details.object_array as *mut u32, details.object_array_len).validate_read().unwrap();
    for to_transfer in handles_to_transfer {
        let handle =
            Handle::try_from(*to_transfer as usize).map_err(|_| SpawnTaskError::InvalidHandleToTransfer)?;
        let object = task.handles.get(handle).ok_or(SpawnTaskError::InvalidHandleToTransfer)?;
        handles.add(object);
    }

    let pmm = crate::PMM.get();
    let new_task =
        Task::new(task.id(), address_space, name.to_string(), VAddr::new(details.entry_point), handles, &pmm, vmm)
            .expect("Failed to create task");
    scheduler.add_task(new_task.clone());

    Ok(task.handles.add(new_task))
}

pub fn resize_memory_object<P>(
    task: &Arc<Task<P>>,
    memory_object_handle: usize,
    new_size: usize,
) -> Result<(), ResizeMemoryObjectError>
where
    P: Platform,
{
    let memory_object_handle =
        Handle::try_from(memory_object_handle).map_err(|_| ResizeMemoryObjectError::InvalidMemoryObjectHandle)?;
    let memory_object = task
        .handles
        .get(memory_object_handle)
        .ok_or(ResizeMemoryObjectError::InvalidMemoryObjectHandle)?
        .downcast_arc::<MemoryObject>()
        .ok()
        .ok_or(ResizeMemoryObjectError::InvalidMemoryObjectHandle)?;

    /*
     * TODO: the big remaining question is how we deal with remapping a resized memory object that
     * is mapped into an address space with multiple tasks, or multiple address spaces. This is not
     * easy - in the case that one of the tasks is running on another CPU, this involves sending
     * TLB shootdowns and things... maybe initially just ban this?
     *
     * XXX: We don't actually check this currently as we don't track which address spaces a
     * MemoryObject is mapped into. Probably do this?
     *
     * We might only need to do this when unmapping memory objects because I don't think any arch
     * we're targetting will cache a page not being present?
     */

    let old_size = memory_object.size();
    if new_size > old_size {
        // Grow the memory object
        // TODO: should we require that the size be multiple of the page size, or just up it here?
        let extend_by = mulch::math::align_up(new_size - old_size, Size4KiB::SIZE);
        let new_backing = crate::PMM.get().alloc(extend_by / Size4KiB::SIZE);
        unsafe {
            memory_object.extend(extend_by, new_backing);
        }

        // Map the new region into the current task's address space, if we're already mapped.
        let mappings = task.address_space.mappings.lock();
        let mapping = mappings.iter().find(|(_addr, object)| object.id == memory_object.id);
        if let Some((virtual_addr, object)) = mapping {
            let new_virtual = *virtual_addr + old_size;
            task.address_space
                .page_table
                .lock()
                .map_area(new_virtual, new_backing, extend_by, object.flags(), crate::PMM.get())
                .map_err(|_| ResizeMemoryObjectError::ResizedObjectCannotBeRemapped)?;
        }
    } else if new_size < old_size {
        // Shrink the memory object
        todo!()
    } else {
        // The memory object is already the correct size. Do nothing.
    }

    Ok(())
}

pub fn wait_for_interrupt<P>(
    scheduler: &Scheduler<P>,
    task: &Arc<Task<P>>,
    interrupt_handle: usize,
    block: usize,
) -> Result<(), WaitForInterruptError>
where
    P: Platform,
{
    let interrupt_handle =
        Handle::try_from(interrupt_handle).map_err(|_| WaitForInterruptError::InvalidInterruptHandle)?;
    let block = block != 0;
    let interrupt = task
        .handles
        .get(interrupt_handle)
        .ok_or(WaitForInterruptError::InvalidInterruptHandle)?
        .downcast_arc::<Interrupt>()
        .ok()
        .ok_or(WaitForInterruptError::InvalidInterruptHandle)?;

    if block {
        /*
         * XXX: This is an extremely simple way of implementing this. We should instead probably block
         * the task, and spawn a tasklet that is awoken when the event is triggered to unblock it. For
         * now, though, this will work well enough.
         */
        while !interrupt.triggered.load(Ordering::SeqCst) {
            scheduler.schedule(TaskState::Ready);
        }
        assert_eq!(
            Ok(true),
            interrupt.triggered.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
        );
        Ok(())
    } else {
        match interrupt.triggered.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(true) => Ok(()),
            _ => Err(WaitForInterruptError::NoInterrupt),
        }
    }
}

pub fn ack_interrupt<P>(task: &Arc<Task<P>>, interrupt_handle: usize) -> Result<(), AckInterruptError>
where
    P: Platform,
{
    let interrupt_handle =
        Handle::try_from(interrupt_handle).map_err(|_| AckInterruptError::InvalidInterruptHandle)?;
    let interrupt = task
        .handles
        .get(interrupt_handle)
        .ok_or(AckInterruptError::InvalidInterruptHandle)?
        .downcast_arc::<Interrupt>()
        .ok()
        .ok_or(AckInterruptError::InvalidInterruptHandle)?;

    interrupt.rearm::<P>();
    Ok(())
}
