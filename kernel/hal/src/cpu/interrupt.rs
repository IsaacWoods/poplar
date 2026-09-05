use crate::{cpu::CpuFlags, mem::VAddr};

pub const NUM_IDT_ENTRIES: usize = 256;
pub const NUM_EXCEPTIONS: usize = 32;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Exception {
    DivideError = 0,
    Debug = 1,
    Nmi = 2,
    Breakpoint = 3,
    /// Produced by the `INTO` instruction when `OF=1`. Not produced in x86_64 mode.
    Overflow = 4,
    /// Produced when the `BOUND` instruction detects out-of-range indices. Not produced in x86_64
    /// mode.
    BoundRangeExceeded = 5,
    InvalidOpcode = 6,
    /// Produced by x87/SSE/AVX instructions while `TS=1` or `EM=1`
    DeviceNotAvailable = 7,
    DoubleFault = 8,
    /// Historical i80287 exception. Not produced in x86_64 mode.
    CoprocessorSegmentOverrun = 9,
    InvalidTss = 10,
    SegmentNotPresent = 11,
    StackSegmentFault = 12,
    GeneralProtectionFault = 13,
    PageFault = 14,
    Reserved = 15,
    MathFault = 16,
    AlignmentCheck = 17,
    MachineCheck = 18,
    SimdException = 19,
    VirtException = 20,
    ControlProtectionException = 21,
}

/// The layout of the stack when either an interrupt or exception is taken. It contains the frame
/// pushed by the CPU when an interrupt is taken, potentially an error code for some exceptions (see
/// comment below), and a copy of all registers - these are popped when returning, and so can be
/// modified by a handler.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct InterruptStackFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,

    /// For exceptions that carry an error code, this is it. For interrupts and other exceptions,
    /// this is a dummy value that should be ignored.
    ///
    /// There is no disadvantage to carrying this for all interrupts, as it has the advantage of
    /// naturally aligning the stack for entry to the handler.
    pub error_code: u64,
    pub ip: VAddr,
    pub cs: u64,
    pub flags: CpuFlags,
    pub sp: VAddr,
    pub ss: u64,
}

pub type InterruptHandler = extern "C" fn(&mut InterruptStackFrame);

/// A function that can act as an entry point in the IDT. Should almost always be constructed using
/// the [`wrap_handler`] or [`wrap_handler_with_error_code`] macros.
pub type RawHandler = extern "C" fn() -> !;

/// Wrap a handler for an interrupt or an exception that **does not** carry an error code. A bogus
/// error code will be inserted to ensure the correct layout of the `InterruptStackFrame` and to
/// align the stack upon entry to the handler to `0x10`. This will create a function of type
/// `RawHandler`.
#[macro_export]
macro_rules! wrap_handler {
    ($name: path) => {{
        #[unsafe(naked)]
        extern "C" fn wrapper() -> ! {
            core::arch::naked_asm!(
                "
                sub rsp, 8      // Bogus error code
                push rax
                push rbx
                push rcx
                push rdx
                push rsi
                push rdi
                push rbp
                push r8
                push r9
                push r10
                push r11
                push r12
                push r13
                push r14
                push r15

                mov rdi, rsp
                call {}

                pop r15
                pop r14
                pop r13
                pop r12
                pop r11
                pop r10
                pop r9
                pop r8
                pop rbp
                pop rdi
                pop rsi
                pop rdx
                pop rcx
                pop rbx
                pop rax
                add rsp, 8      // Pop off the bogus error code

                iretq
            ",
            sym $name,
            )
        }

        wrapper
    }}
}

/// Wrap a handler for an exception that **does** carry its own error code. This will create a
/// function of type `RawHandler`.
#[macro_export]
macro_rules! wrap_handler_with_error_code {
    ($name: path) => {{
        #[unsafe(naked)]
        extern "C" fn wrapper() -> ! {
            core::arch::naked_asm!(
                "
                push rax
                push rbx
                push rcx
                push rdx
                push rsi
                push rdi
                push rbp
                push r8
                push r9
                push r10
                push r11
                push r12
                push r13
                push r14
                push r15

                // The stack is already aligned correctly
                mov rdi, rsp
                call {}

                pop r15
                pop r14
                pop r13
                pop r12
                pop r11
                pop r10
                pop r9
                pop r8
                pop rbp
                pop rdi
                pop rsi
                pop rdx
                pop rcx
                pop rbx
                pop rax
                add rsp, 8  // Pop the error code

                iretq
            ",
            sym $name,
            )
        }

        wrapper
    }}
}
