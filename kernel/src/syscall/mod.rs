use crate::{
    arch_impl::{common_per_cpu_data, common_per_cpu_data_mut},
    mailbox::Mailbox,
    object::common::{CommonTask, MemoryObjectMappingError},
    COMMON,
};
use core::{slice, str};
use libpebble::{
    caps::Capability,
    syscall,
    syscall::{mailbox::MailboxError, result::result_to_syscall_repr},
    KernelObjectId,
};
use log::{info, trace, warn};
use spin::RwLock;

/// This is the architecture-independent syscall handler. It should be called by the handler that
/// receives the syscall (each architecture is free to do this however it wishes). The only
/// parameter that is guaranteed to be valid is `number`; the meaning of the rest may be undefined
/// depending on how many parameters the specific system call takes.
///
/// It is defined as using the C ABI, so an architecture can call it stably from assembly if it
/// wants to.
#[no_mangle]
pub extern "C" fn rust_syscall_handler(number: usize, a: usize, b: usize, c: usize, d: usize, e: usize) -> usize {
    // info!("Syscall! number = {}, a = {}, b = {}, c = {}, d = {}, e = {}", number, a, b, c, d, e);

    match number {
        syscall::SYSCALL_YIELD => yield_syscall(),
        syscall::SYSCALL_EARLY_LOG => early_log(a, b),
        syscall::SYSCALL_REQUEST_SYSTEM_OBJECT => request_system_object(a, b, c, d, e),
        syscall::SYSCALL_MY_ADDRESS_SPACE => my_address_space(),
        syscall::SYSCALL_MAP_MEMORY_OBJECT => map_memory_object(a, b),
        syscall::SYSCALL_CREATE_MAILBOX => create_mailbox(),

        _ => {
            // TODO: unsupported system call number, kill process or something?
            warn!("Process made system call with invalid syscall number: {}", number);
            1
        }
    }
}

fn yield_syscall() -> usize {
    /*
     * This is a fairly unique system call in that it can return into a different context than the
     * one that called it. We ask the scheduler to move us to the next task, then return to the new
     * userspace context.
     */
    info!("Process yielded!");
    unsafe {
        common_per_cpu_data_mut().scheduler.switch_to_next();
    }

    0
}

fn early_log(str_length: usize, str_address: usize) -> usize {
    /*
     * Returns:
     *      0 => message was successfully logged
     *      1 => message was too long
     *      2 => string was not valid UTF-8
     *      3 => task doesn't have `EarlyLogging` capability
     *
     * TODO: check that b is a valid userspace pointer and that it's mapped to physical
     * memory
     */
    let task = unsafe { common_per_cpu_data().running_task().object.task().unwrap().read() };

    // Check the current task has the `EarlyLogging` capability
    if !task.capabilities.contains(&Capability::EarlyLogging) {
        return 3;
    }

    // Check if the message is too long
    if str_length > 1024 {
        return 1;
    }

    // Check the message is valid UTF-8
    let message = match str::from_utf8(unsafe { slice::from_raw_parts(str_address as *const u8, str_length) }) {
        Ok(message) => message,
        Err(_) => return 2,
    };

    trace!("Early log message from {}: {}", task.name(), message);
    0
}

fn request_system_object(id: usize, b: usize, c: usize, d: usize, e: usize) -> usize {
    use libpebble::syscall::system_object::*;

    result_to_syscall_repr(match id {
        SYSTEM_OBJECT_BACKUP_FRAMEBUFFER_ID => {
            /*
             * Check that the task has the correct capability. It's important to do this first, so we don't leak
             * the fact that a system object doesn't exist to an unprivileged task.
             */
            if unsafe { common_per_cpu_data() }
                .running_task()
                .object
                .task()
                .unwrap()
                .read()
                .capabilities
                .contains(&Capability::AccessBackupFramebuffer)
            {
                match *COMMON.get().backup_framebuffer.lock() {
                    Some((id, info)) => {
                        /*
                         * `b` contains a userspace address that we write the framebuffer info back into.
                         */
                        unsafe {
                            // TODO: at the moment, userspace can trivially crash the kernel by passing an address
                            // in here that is either not mapped, or does not point into userspace (potentially
                            // overwriting important kernel info). We should validate that the address passed is
                            // both mapped and points into userspace, and provide some helpers for doing so.
                            *(b as *mut FramebufferSystemObjectInfo) = info;
                        }

                        Ok(id)
                    }
                    None => Err(RequestSystemObjectError::ObjectDoesNotExist),
                }
            } else {
                Err(RequestSystemObjectError::AccessDenied)
            }
        }

        _ => Err(RequestSystemObjectError::NotAValidId),
    })
}

fn my_address_space() -> usize {
    unsafe { common_per_cpu_data() }
        .running_task()
        .object
        .task()
        .unwrap()
        .read()
        .address_space
        .id
        .to_syscall_repr()
}

fn map_memory_object(memory_object_id: usize, address_space_id: usize) -> usize {
    /*
     * TODO: enforce that the calling task must have access to the AddressSpace and MemoryObject
     * for this to work (we need to build the owning / access system first).
     */
    const STATUS_SUCCESS: usize = 0;
    const STATUS_ALREADY_OCCUPIED: usize = 1;
    const STATUS_ACCESS_TO_MEMORY_OBJECT_DENIED: usize = 2;
    const STATUS_ACCESS_TO_ADDRESS_SPACE_DENIED: usize = 3;
    const STATUS_NOT_A_MEMORY_OBJECT: usize = 4;
    const STATUS_NOT_AN_ADDRESS_SPACE: usize = 5;

    let memory_object =
        match COMMON.get().object_map.read().get(KernelObjectId::from_syscall_repr(memory_object_id)) {
            Some(object) => object.clone(),
            None => return STATUS_NOT_A_MEMORY_OBJECT,
        };

    // Check it's a MemoryObject
    if memory_object.object.memory_object().is_none() {
        return STATUS_NOT_A_MEMORY_OBJECT;
    }

    match COMMON.get().object_map.read().get(KernelObjectId::from_syscall_repr(address_space_id)) {
        Some(address_space) => match address_space.object.address_space() {
            Some(address_space) => match address_space.write().map_memory_object(memory_object) {
                Ok(()) => STATUS_SUCCESS,
                Err(MemoryObjectMappingError::SpaceAlreadyOccupied) => STATUS_ALREADY_OCCUPIED,
            },
            None => STATUS_NOT_AN_ADDRESS_SPACE,
        },
        None => STATUS_NOT_AN_ADDRESS_SPACE,
    }
}

fn create_mailbox() -> usize {
    let object_map = &mut crate::COMMON.get().object_map.write();
    let mailbox = KernelObject::Mailbox(RwLock::new(box Mailbox::new())).add_to_map(object_map);
    result_to_syscall_repr::<MailboxError>(Ok(mailbox.id))
}
