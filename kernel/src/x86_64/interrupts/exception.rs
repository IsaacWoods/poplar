//! This module contains all the interrupt handlers used to handle CPU exceptions. Some of these
//! exceptions are handled and recovered from, while some are fatal errors and lead to kernel
//! panics.

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
    // TODO: use get_bit method on BitField instead and replace the patterns with exhaustive bool
    // ones
    error!(
        "PAGE_FAULT: {} ({:#x})",
        match (
            (error_code >> 2) & 0b1, // User / Supervisor
            (error_code >> 4) & 0b1, // Instruction / Data
            (error_code >> 1) & 0b1, // Read / Write
            (error_code >> 0) & 0b1  // Present
        ) {
            // Page faults caused by the kernel
            (0, 0, 0, 0) => "Kernel read non-present page",
            (0, 0, 0, 1) => "Kernel read present page",
            (0, 0, 1, 0) => "Kernel wrote to non-present page",
            (0, 0, 1, 1) => "Kernel wrote to present page",
            (0, 1, _, 0) => "Kernel fetched instruction from non-present page",
            (0, 1, _, 1) => "Kernel fetched instruction from present page",

            // Page faults caused by user processes
            (1, 0, 0, 0) => "User process read non-present page",
            (1, 0, 0, 1) => "User process read present page (probable access violation)",
            (1, 0, 1, 0) => "User process wrote to non-present page",
            (1, 0, 1, 1) => "User process wrote to present page (probable access violation)",
            (1, 1, _, 0) => "User process fetched instruction from non-present page",
            (1, 1, _, 1) => "User process fetched instruction from present page (probable access violation)",

            (_, _, _, _) => {
                panic!("INVALID PAGE-FAULT ERROR CODE");
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
