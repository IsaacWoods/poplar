//! This module contains all the interrupt handlers used to handle CPU exceptions. Some of these
//! exceptions are handled and recovered from, while some are fatal errors and lead to kernel
//! panics.

use bit_field::BitField;
use log::{error, info};
use pebble_util::BinaryPrettyPrint;
use x86_64::hw::{idt::InterruptStackFrame, registers::read_control_reg};

pub extern "C" fn nmi_handler(_: &InterruptStackFrame) {
    info!("NMI occured!");
}

pub extern "C" fn breakpoint_handler(stack_frame: &InterruptStackFrame) {
    info!("BREAKPOINT: {:#?}", stack_frame);
}

pub extern "C" fn invalid_opcode_handler(stack_frame: &InterruptStackFrame) {
    error!("INVALID OPCODE AT: {:#x}", stack_frame.instruction_pointer);

    loop {}
}

pub extern "C" fn general_protection_fault_handler(stack_frame: &InterruptStackFrame, error_code: u64) {
    error!("General protection fault (error code = {:#x}). Interrupt stack frame: ", error_code);
    error!("{:#?}", stack_frame);

    loop {}
}

pub extern "C" fn page_fault_handler(stack_frame: &InterruptStackFrame, error_code: u64) {
    error!(
        "PAGE_FAULT: {} ({:#x})",
        match (
            error_code.get_bit(2), // User / Supervisor
            error_code.get_bit(4), // Instruction / Data
            error_code.get_bit(1), // Read / Write
            error_code.get_bit(0)  // Present
        ) {
            // Page faults caused by the kernel
            (false, false, false, false) => "Kernel read non-present page",
            (false, false, false, true) => "Kernel read present page",
            (false, false, true, false) => "Kernel wrote to non-present page",
            (false, false, true, true) => "Kernel wrote to present page",
            (false, true, _, false) => "Kernel fetched instruction from non-present page",
            (false, true, _, true) => "Kernel fetched instruction from present page",

            // Page faults caused by user processes
            (true, false, false, false) => "User process read non-present page",
            (true, false, false, true) => "User process read present page (probable access violation)",
            (true, false, true, false) => "User process wrote to non-present page",
            (true, false, true, true) => "User process wrote to present page (probable access violation)",
            (true, true, _, false) => "User process fetched instruction from non-present page",
            (true, true, _, true) => {
                "User process fetched instruction from present page (probable access violation)"
            }
        },
        read_control_reg!(cr2) // CR2 holds the address of the page that caused the #PF
    );

    error!("Error code: {}", BinaryPrettyPrint(error_code));
    error!("{:#?}", stack_frame);

    /*
     * Page-faults can be recovered from and so are faults, but we never will so just give up.
     */
    loop {}
}

pub extern "C" fn double_fault_handler(stack_frame: &InterruptStackFrame, error_code: u64) {
    error!("EXCEPTION: DOUBLE FAULT   (Error code: {})\n{:#?}", error_code, stack_frame);

    loop {}
}
