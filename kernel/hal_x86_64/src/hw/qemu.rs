use crate::hw::port::Port;

/// Exit codes to use with `ExitPort`. We need to differentiate these from the exit codes QEMU itself uses, so we
/// can differentiate between success/failure in QEMU vs the kernel itself.
///
/// The code passed to the exit port is then turned into a QEMU exit code with `(code << 1) | 1`, so `Success` ends
/// up as `0x21`, and `Failed` ends up as `0x23`. These can be handled specially by whatever invokes QEMU.
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum ExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub struct ExitPort(Port<u32>);

impl ExitPort {
    pub unsafe fn new() -> ExitPort {
        ExitPort(unsafe { Port::new(0xf4) })
    }

    pub fn exit(mut self, exit_code: ExitCode) -> ! {
        unsafe {
            self.0.write(exit_code as u32);
        }
        unreachable!()
    }
}
