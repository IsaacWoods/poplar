use core::{slice, str};
use libpebble::syscall;
use log::{info, trace, warn};

/// This is the architecture-independent syscall handler. It should be called by the handler that
/// receives the syscall (each architecture is free to do this however it wishes). The only
/// parameter that is guaranteed to be valid is `number`; the meaning of the rest may be undefined
/// depending on how many parameters the specific system call takes.
pub fn handle_syscall(number: usize, a: usize, b: usize, c: usize, d: usize, e: usize) -> usize {
    info!("Syscall! number = {}, a = {}, b = {}, c = {}, d = {}, e = {}", number, a, b, c, d, e);

    match number {
        syscall::SYSCALL_YIELD => {
            info!("Process yielded!");
            // TODO: schedule another task or something
            0
        }

        syscall::SYSCALL_EARLY_LOG => {
            /*
             * a = length of string in bytes (must be <= 1024)
             * b = pointer to string in userspace
             *
             * Returns:
             *      0 => message was successfully logged
             *      1 => message was too long
             *      2 => string was not valid UTF-8
             *
             * TODO: check that b is a valid userspace pointer and that it's mapped to physical
             * memory
             * TODO: log the process ID / name to help identify stuff
             */
            if a > 1024 {
                return 1;
            }

            let message = match str::from_utf8(unsafe { slice::from_raw_parts(b as *const u8, a) }) {
                Ok(message) => message,
                Err(_) => return 2,
            };

            trace!("Userspace task early log message: {}", message);
            0
        }

        _ => {
            // TODO: unsupported system call number, kill process or something?
            warn!("Process made system call with invalid syscall number: {}", number);
            1
        }
    }
}
