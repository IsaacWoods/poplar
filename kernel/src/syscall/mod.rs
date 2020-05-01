use crate::{
    object::{
        address_space::AddressSpace,
        memory_object::MemoryObject,
        task::{Task, TaskState},
        KernelObject,
    },
    per_cpu::PerCpu,
    Platform,
};
use alloc::sync::Arc;
use bit_field::BitField;
use core::{convert::TryFrom, slice, str};
use hal::memory::{Flags, VirtualAddress};
use libpebble::{
    caps::Capability,
    syscall::{
        self,
        result::{handle_to_syscall_repr, status_to_syscall_repr},
        CreateMemoryObjectError,
        EarlyLogError,
        FramebufferInfo,
        GetFramebufferError,
        MapMemoryObjectError,
        SendMessageError,
    },
    Handle,
    ZERO_HANDLE,
};
use log::{info, trace, warn};

/// This is the architecture-independent syscall handler. It should be called by the handler that
/// receives the syscall (each architecture is free to do this however it wishes). The only
/// parameter that is guaranteed to be valid is `number`; the meaning of the rest may be undefined
/// depending on how many parameters the specific system call takes.
pub fn handle_syscall<P>(number: usize, a: usize, b: usize, c: usize, d: usize, e: usize) -> usize
where
    P: Platform,
{
    info!("Syscall! number = {}, a = {}, b = {}, c = {}, d = {}, e = {}", number, a, b, c, d, e);
    let task = P::per_cpu().scheduler().get_mut().running_task.as_ref().unwrap();

    match number {
        syscall::SYSCALL_YIELD => yield_syscall::<P>(),
        syscall::SYSCALL_EARLY_LOG => status_to_syscall_repr(early_log(task, a, b)),
        syscall::SYSCALL_GET_FRAMEBUFFER => handle_to_syscall_repr(get_framebuffer(task, a)),
        syscall::SYSCALL_CREATE_MEMORY_OBJECT => handle_to_syscall_repr(create_memory_object(task, a, b, c)),
        syscall::SYSCALL_MAP_MEMORY_OBJECT => status_to_syscall_repr(map_memory_object(task, a, b, c)),
        syscall::SYSCALL_CREATE_CHANNEL => unimplemented!(),
        syscall::SYSCALL_SEND_MESSAGE => status_to_syscall_repr(send_message(task, a, b, c, d, e)),

        _ => {
            // TODO: unsupported system call number, kill process or something?
            warn!("Process made system call with invalid syscall number: {}", number);
            1
        }
    }
}

fn yield_syscall<P>() -> usize
where
    P: Platform,
{
    info!("Process yielded!");
    P::per_cpu().scheduler().switch_to_next(TaskState::Ready);
    0
}

fn early_log<P>(task: &Arc<Task<P>>, str_length: usize, str_address: usize) -> Result<(), EarlyLogError>
where
    P: Platform,
{
    // Check the current task has the `EarlyLogging` capability
    if !task.capabilities.contains(&Capability::EarlyLogging) {
        return Err(EarlyLogError::TaskDoesNotHaveCorrectCapability);
    }

    // Check if the message is too long
    if str_length > 1024 {
        return Err(EarlyLogError::MessageTooLong);
    }

    // Check the message is valid UTF-8
    // TODO: validate user pointer before creating slice from it
    let message = str::from_utf8(unsafe { slice::from_raw_parts(str_address as *const u8, str_length) })
        .map_err(|_| EarlyLogError::MessageNotValidUtf8)?;

    trace!("Early log message from {}: {}", task.name, message);
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

    // TODO: validate the info pointer before we do this
    unsafe {
        *(info_address as *mut FramebufferInfo) = *info;
    }

    Ok(handle)
}

fn create_memory_object<P>(
    task: &Arc<Task<P>>,
    virtual_address: usize,
    size: usize,
    flags: usize,
) -> Result<Handle, CreateMemoryObjectError>
where
    P: Platform,
{
    let writable = flags.get_bit(0);
    let executable = flags.get_bit(1);

    // TODO: do something more sensible with this when we have a concept of physical memory "ownership"
    let physical_start = crate::PHYSICAL_MEMORY_MANAGER.get().alloc_bytes(size);

    let memory_object = MemoryObject::new(
        task.id(),
        VirtualAddress::new(virtual_address),
        physical_start,
        size,
        Flags { writable, executable, user_accessible: true, ..Default::default() },
    );

    Ok(task.add_handle(memory_object))
}

fn map_memory_object<P>(
    task: &Arc<Task<P>>,
    memory_object_handle: usize,
    address_space_handle: usize,
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

    if address_space_handle == ZERO_HANDLE {
        /*
         * If the AddressSpace handle is the zero handle, we map the MemoryObject into the calling task's
         * address space.
         */
        task.address_space.clone()
    } else {
        task.handles
            .read()
            .get(&memory_object_handle)
            .ok_or(MapMemoryObjectError::InvalidHandle)?
            .clone()
            .downcast_arc::<AddressSpace<P>>()
            .ok()
            .ok_or(MapMemoryObjectError::NotAnAddressSpace)?
    }
    .map_memory_object(memory_object.clone(), &crate::PHYSICAL_MEMORY_MANAGER.get())?;

    /*
     * An address pointer of `0` signals to the kernel that the caller does not need to know the virtual
     * address, so don't bother writing it back.
     */
    if address_ptr != 0x0 {
        // TODO: validate the user pointer
        unsafe {
            *(address_ptr as *mut VirtualAddress) = memory_object.virtual_address;
        }
    }

    Ok(())
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
    info!("Message: {:x?}", unsafe { core::slice::from_raw_parts(byte_address as *const u8, num_bytes) });
    Ok(())
}
