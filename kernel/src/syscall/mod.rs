use crate::{
    object::{address_space::AddressSpace, memory_object::MemoryObject, task::TaskState},
    HalImpl,
};
use core::{convert::TryFrom, slice, str};
use hal::{memory::VirtualAddress, Hal};
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
    },
    Handle,
    ZERO_HANDLE,
};
use log::{info, trace, warn};

/// This is the architecture-independent syscall handler. It should be called by the handler that
/// receives the syscall (each architecture is free to do this however it wishes). The only
/// parameter that is guaranteed to be valid is `number`; the meaning of the rest may be undefined
/// depending on how many parameters the specific system call takes.
///
/// It is defined as using the C ABI, so an architecture can call it stably from assembly if it
/// wants to.
#[no_mangle]
pub extern "C" fn rust_syscall_handler(number: usize, a: usize, b: usize, c: usize, d: usize, e: usize) -> usize {
    info!("Syscall! number = {}, a = {}, b = {}, c = {}, d = {}, e = {}", number, a, b, c, d, e);

    match number {
        syscall::SYSCALL_YIELD => yield_syscall(),
        syscall::SYSCALL_EARLY_LOG => status_to_syscall_repr(early_log(a, b)),
        syscall::SYSCALL_GET_FRAMEBUFFER => handle_to_syscall_repr(get_framebuffer(a)),
        // syscall::SYSCALL_CREATE_MEMORY_OBJECT => result_to_syscall_repr(create_memory_object(a, b, c)),
        syscall::SYSCALL_MAP_MEMORY_OBJECT => status_to_syscall_repr(map_memory_object(a, b, c)),

        _ => {
            // TODO: unsupported system call number, kill process or something?
            warn!("Process made system call with invalid syscall number: {}", number);
            1
        }
    }
}

fn yield_syscall() -> usize {
    info!("Process yielded!");
    unsafe { HalImpl::per_cpu() }.kernel_data().scheduler().switch_to_next(TaskState::Ready);
    0
}

fn early_log(str_length: usize, str_address: usize) -> Result<(), EarlyLogError> {
    /*
     * TODO: check that b is a valid userspace pointer and that it's mapped to physical
     * memory
     */
    let task = unsafe { HalImpl::per_cpu() }.kernel_data().scheduler().running_task.as_ref().unwrap();

    // Check the current task has the `EarlyLogging` capability
    if !task.capabilities.contains(&Capability::EarlyLogging) {
        return Err(EarlyLogError::TaskDoesNotHaveCorrectCapability);
    }

    // Check if the message is too long
    if str_length > 1024 {
        return Err(EarlyLogError::MessageTooLong);
    }

    // Check the message is valid UTF-8
    let message = str::from_utf8(unsafe { slice::from_raw_parts(str_address as *const u8, str_length) })
        .map_err(|_| EarlyLogError::MessageNotValidUtf8)?;

    trace!("Early log message from {}: {}", task.name, message);
    Ok(())
}

fn get_framebuffer(info_address: usize) -> Result<Handle, GetFramebufferError> {
    let task = unsafe { HalImpl::per_cpu() }.kernel_data().scheduler().running_task.as_ref().unwrap();

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

// fn create_memory_object(
//     virtual_address: usize,
//     size: usize,
//     flags: usize,
// ) -> Result<KernelObjectId, MemoryObjectError> {
//     let writable = flags.get_bit(0);
//     let executable = flags.get_bit(1);

//     unimplemented!()
// }

fn map_memory_object(
    memory_object_handle: usize,
    address_space_handle: usize,
    address_pointer: usize,
) -> Result<(), MapMemoryObjectError> {
    let memory_object_handle =
        Handle::try_from(memory_object_handle).map_err(|_| MapMemoryObjectError::InvalidHandle)?;
    let address_space_handle =
        Handle::try_from(address_space_handle).map_err(|_| MapMemoryObjectError::InvalidHandle)?;

    let task = unsafe { HalImpl::per_cpu() }.kernel_data().scheduler().running_task.as_ref().unwrap();

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
            .downcast_arc::<AddressSpace<HalImpl>>()
            .ok()
            .ok_or(MapMemoryObjectError::NotAnAddressSpace)?
    }
    .map_memory_object(memory_object.clone(), &crate::PHYSICAL_MEMORY_MANAGER.get())?;

    // TODO: validate the user pointer
    unsafe {
        *(address_pointer as *mut VirtualAddress) = memory_object.virtual_address;
    }

    Ok(())
}
