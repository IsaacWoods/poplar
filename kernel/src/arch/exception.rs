use hal::cpu::interrupt::InterruptStackFrame;
use tracing::{error, info};

pub extern "C" fn divide_error(frame: &mut InterruptStackFrame) {
    error!("Exception: divide error at {:#x}", frame.ip);
}

pub extern "C" fn debug_exception(frame: &mut InterruptStackFrame) {
    info!("Exception: debug at {:#x}", frame.ip);
}

pub extern "C" fn nmi(_frame: &mut InterruptStackFrame) {
    info!("NMI");
}

pub extern "C" fn breakpoint(frame: &mut InterruptStackFrame) {
    info!("Breakpoint at {:#x}", frame.ip);
}

pub extern "C" fn invalid_opcode(frame: &mut InterruptStackFrame) {
    error!("Exception: invalid opcode at {:#x}", frame.ip);
}

pub extern "C" fn device_not_available(frame: &mut InterruptStackFrame) {
    error!("Exception: device not available at {:#x}", frame.ip);
}

pub extern "C" fn double_fault(frame: &mut InterruptStackFrame) {
    error!("Exception: double fault at {:#x}", frame.ip);
}

pub extern "C" fn invalid_tss(frame: &mut InterruptStackFrame) {
    error!("Exception: invalid TSS at {:#x}", frame.ip);
}

pub extern "C" fn segment_not_present(frame: &mut InterruptStackFrame) {
    error!("Exception: segment not present at {:#x}", frame.ip);
}

pub extern "C" fn stack_segment_fault(frame: &mut InterruptStackFrame) {
    error!("Exception: stack segment fault at {:#x}", frame.ip);
}

pub extern "C" fn general_protection_fault(frame: &mut InterruptStackFrame) {
    error!(
        "Exception: general protection fault at {:#x} with error {:#b}",
        frame.ip, frame.error_code
    );
    todo!();
}

pub extern "C" fn page_fault(frame: &mut InterruptStackFrame) {
    error!(
        "Exception: page fault at {:#x} with error: {:#b}",
        frame.ip, frame.error_code
    );
}

pub extern "C" fn math_fault(frame: &mut InterruptStackFrame) {
    error!("Exception: math fault at {:#x}", frame.ip);
}
