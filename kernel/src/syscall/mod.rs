mod validation;

use crate::{
    object::{
        address_space::AddressSpace,
        channel::{ChannelEnd, Message},
        event::Event,
        memory_object::MemoryObject,
        task::{Task, TaskState},
        KernelObject,
        KernelObjectType,
    },
    scheduler::Scheduler,
    Platform,
};
use alloc::{collections::BTreeMap, string::String, sync::Arc};
use bit_field::BitField;
use core::{convert::TryFrom, sync::atomic::Ordering};
use hal::memory::{Flags, PAddr, VAddr};
use poplar::{
    caps::Capability,
    syscall::{
        self,
        result::{handle_to_syscall_repr, status_to_syscall_repr, status_with_payload_to_syscall_repr},
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
        RegisterServiceError,
        SendMessageError,
        SubscribeToServiceError,
        WaitForEventError,
        CHANNEL_MAX_NUM_HANDLES,
    },
    Handle,
};
use spinning_top::Spinlock;
use tracing::{info, warn};
use validation::{UserPointer, UserSlice, UserString};

/// Maps the name of a service to the channel used to register new service users.
static SERVICE_MAP: Spinlock<BTreeMap<String, Arc<ChannelEnd>>> = Spinlock::new(BTreeMap::new());

/// This is the architecture-independent syscall handler. It should be called by the handler that
/// receives the syscall (each architecture is free to do this however it wishes). The only
/// parameter that is guaranteed to be valid is `number`; the meaning of the rest may be undefined
/// depending on how many parameters the specific system call takes.
pub fn handle_syscall<P>(
    scheduler: &Scheduler<P>,
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
        syscall::SYSCALL_REGISTER_SERVICE => handle_to_syscall_repr(register_service(&task, a, b)),
        syscall::SYSCALL_SUBSCRIBE_TO_SERVICE => handle_to_syscall_repr(subscribe_to_service(&task, a, b)),
        syscall::SYSCALL_PCI_GET_INFO => status_with_payload_to_syscall_repr(pci_get_info(&task, a, b)),
        syscall::SYSCALL_WAIT_FOR_EVENT => status_to_syscall_repr(wait_for_event(scheduler, &task, a, b)),
        syscall::SYSCALL_POLL_INTEREST => status_with_payload_to_syscall_repr(poll_interest(&task, a)),

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
    // Check the current task has the `EarlyLogging` capability
    // if !task.capabilities.contains(&Capability::EarlyLogging) {
    //     // Log a warning because otherwise it's difficult to debug a task (it can't log...)
    //     warn!("Task '{}' tried to use early logging but does not have the correct capability!", task.name);
    //     return Err(EarlyLogError::TaskDoesNotHaveCorrectCapability);
    // }

    // Check if the message is too long
    if str_length > 4096 {
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
    /*
     * Check that the task has the correct capability.
     */
    if !task.capabilities.contains(&Capability::GetFramebuffer) {
        return Err(GetFramebufferError::AccessDenied);
    }

    let (info, memory_object) = crate::FRAMEBUFFER.try_get().ok_or(GetFramebufferError::NoFramebufferCreated)?;
    let handle = task.add_handle(memory_object.clone());

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
    use hal::memory::{FrameSize, Size4KiB};
    use mulch::math::align_up;

    // TODO: should we require that the size be multiple of the page size, or just up it here?
    let size = align_up(size, Size4KiB::SIZE);
    let flags = MemoryObjectFlags::from_bits_truncate(flags as u32);

    // TODO: do something more sensible with this when we have a concept of physical memory "ownership"
    let physical_start = crate::PHYSICAL_MEMORY_MANAGER.get().alloc_bytes(size);

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

    Ok(task.add_handle(memory_object))
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
        Handle::try_from(memory_object_handle).map_err(|_| MapMemoryObjectError::InvalidHandle)?;
    let address_space_handle =
        Handle::try_from(address_space_handle).map_err(|_| MapMemoryObjectError::InvalidHandle)?;

    let memory_object = task
        .handles
        .read()
        .get(&memory_object_handle)
        .ok_or(MapMemoryObjectError::InvalidHandle)?
        .clone()
        .downcast_arc::<MemoryObject>()
        .ok()
        .ok_or(MapMemoryObjectError::NotAMemoryObject)?;

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
        task.address_space.map_memory_object(
            memory_object.clone(),
            virtual_address,
            &crate::PHYSICAL_MEMORY_MANAGER.get(),
        )?;
    } else {
        task.handles
            .read()
            .get(&memory_object_handle)
            .ok_or(MapMemoryObjectError::InvalidHandle)?
            .clone()
            .downcast_arc::<AddressSpace<P>>()
            .ok()
            .ok_or(MapMemoryObjectError::NotAnAddressSpace)?
            .map_memory_object(memory_object.clone(), virtual_address, &crate::PHYSICAL_MEMORY_MANAGER.get())?;
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
    let end_a_handle = task.add_handle(end_a);
    let end_b_handle = task.add_handle(end_b);

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
            arr[i] = match task.handles.read().get(handle) {
                Some(object) => Some(object.clone()),
                None => return Err(SendMessageError::InvalidTransferredHandle),
            };

            /*
             * We're transferring the handle's object, so we remove the handle to it from the sending task.
             */
            task.handles.write().remove(&handle);
        }
        arr
    };

    task.handles
        .read()
        .get(&channel_handle)
        .ok_or(SendMessageError::InvalidChannelHandle)?
        .clone()
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
        .read()
        .get(&channel_handle)
        .ok_or(GetMessageError::InvalidChannelHandle)?
        .clone()
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
                handles_buffer[i] = task.add_handle(message.handle_objects[i].as_ref().unwrap().clone());
            }
        }

        let mut status = 0;
        status.set_bits(16..32, message.bytes.len());
        status.set_bits(32..48, num_handles);
        Ok(status)
    })
}

fn register_service<P>(
    task: &Arc<Task<P>>,
    name_length: usize,
    name_ptr: usize,
) -> Result<Handle, RegisterServiceError>
where
    P: Platform,
{
    use poplar::syscall::SERVICE_NAME_MAX_LENGTH;

    // Check that the task has the `ServiceProvider` capability
    if !task.capabilities.contains(&Capability::ServiceProvider) {
        return Err(RegisterServiceError::TaskDoesNotHaveCorrectCapability);
    }

    // Check that the name is not too short or long
    if name_length == 0 || name_length > SERVICE_NAME_MAX_LENGTH {
        return Err(RegisterServiceError::NameLengthNotValid);
    }

    let service_name = UserString::new(name_ptr as *mut u8, name_length)
        .validate()
        .map_err(|()| RegisterServiceError::NamePointerNotValid)?;

    info!("Task {} has registered a service called {}", task.name, service_name);
    let channel = ChannelEnd::new_kernel_channel(task.id());
    SERVICE_MAP.lock().insert(task.name.clone() + "." + service_name, channel.clone());

    Ok(task.add_handle(channel))
}

fn subscribe_to_service<P>(
    task: &Arc<Task<P>>,
    name_length: usize,
    name_ptr: usize,
) -> Result<Handle, SubscribeToServiceError>
where
    P: Platform,
{
    use poplar::syscall::SERVICE_NAME_MAX_LENGTH;

    // Check that the task has the `ServiceUser` capability
    if !task.capabilities.contains(&Capability::ServiceUser) {
        return Err(SubscribeToServiceError::TaskDoesNotHaveCorrectCapability);
    }

    // Check that the name is not too short or long
    if name_length == 0 || name_length > SERVICE_NAME_MAX_LENGTH {
        return Err(SubscribeToServiceError::NameLengthNotValid);
    }

    let service_name = UserString::new(name_ptr as *mut u8, name_length)
        .validate()
        .map_err(|()| SubscribeToServiceError::NamePointerNotValid)?;

    if let Some(register_channel) = SERVICE_MAP.lock().get(service_name) {
        // Create new channel to allow the two tasks to communicate
        let (provider_end, user_end) = ChannelEnd::new_channel(task.id());

        /*
         * Send a message down `register_channel` to tell it about its new service user, transferring the
         * provider's half of the created service channel.
         *
         * XXX: we manually construct a Ptah message here so userspace can use the `poplar::Channel` type if it
         * wants to, but without having to pull that in here.
         */
        let mut handle_objects = [const { None }; CHANNEL_MAX_NUM_HANDLES];
        handle_objects[0] = Some(provider_end as Arc<dyn KernelObject>);
        register_channel.add_message(Message { bytes: [ptah::make_handle_slot(0)].to_vec(), handle_objects });

        // Return the user's end of the new channel to it
        Ok(task.add_handle(user_end))
    } else {
        Err(SubscribeToServiceError::NoServiceWithThatName)
    }
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

    // Check that the task has the 'PciBusDriver' capability
    if !task.capabilities.contains(&Capability::PciBusDriver) {
        return Err(PciGetInfoError::TaskDoesNotHaveCorrectCapability);
    }

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
                let interrupt_handle = device.interrupt_event.clone().map(|interrupt| task.add_handle(interrupt));

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
                            let handle = task.add_handle(memory_object);
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
                            let handle = task.add_handle(memory_object);
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
    let event_handle = Handle::try_from(event_handle).map_err(|_| WaitForEventError::InvalidHandle)?;
    let block = block != 0;
    let event = task
        .handles
        .read()
        .get(&event_handle)
        .ok_or(WaitForEventError::InvalidHandle)?
        .clone()
        .downcast_arc::<Event>()
        .ok()
        .ok_or(WaitForEventError::NotAnEvent)?;

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
    let object = task.handles.read().get(&object_handle).ok_or(PollInterestError::InvalidHandle)?.clone();

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

        // TODO: should this return an error instead?
        _ => false,
    };

    Ok(if interesting { 1 << 16 } else { 0 })
}
