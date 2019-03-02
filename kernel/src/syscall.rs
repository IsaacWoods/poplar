use libpebble::syscall;
use log::info;

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

        _ => {
            // TODO: unsupported system call number
            1
        }
    }
}
