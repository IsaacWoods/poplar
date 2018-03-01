/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use core::fmt;
use gdt::GdtSelectors;
use idt::Idt;
use memory::{FrameAllocator,MemoryController};
use port::Port;
use apic::{LOCAL_APIC,IO_APIC};

/*
 * |------------------|-----------------------------|
 * | Interrupt Vector |            Usage            |
 * |------------------|-----------------------------|
 * |       00-1F      | Intel Reserved (Exceptions) |
 * |       20-2F      | i8259 PIC Interrupts        |
 * |       30-47      | IOAPIC Interrupts           |
 * |        48        | Local APIC timer            |
 * |        ..        | Unused                      |
 * |        FF        | APIC spurious interrupt     |
 * |------------------|-----------------------------|
 */
pub const LEGACY_PIC_BASE           : u8 = 0x20;
pub const IOAPIC_BASE               : u8 = 0x30;
pub const LOCAL_APIC_TIMER          : u8 = 0x48;
pub const APIC_SPURIOUS_INTERRUPT   : u8 = 0xFF;

#[repr(C)]
struct ExceptionStackFrame
{
    instruction_pointer : u64,
    code_segment        : u64,
    cpu_flags           : u64,
    stack_pointer       : u64,
    stack_segment       : u64,
}

impl fmt::Debug for ExceptionStackFrame
{
    fn fmt(&self, f : &mut fmt::Formatter) -> fmt::Result
    {
        write!(f, "Exception occurred at {:#x}:{:x} with flags {:#x} and stack {:#x}:{:x}",
                  self.code_segment,
                  self.instruction_pointer,
                  self.cpu_flags,
                  self.stack_segment,
                  self.stack_pointer)
    }
}

macro_rules! save_regs
{
    () =>
    {
        asm!("push rax
              push rcx
              push rdx
              push rsi
              push rdi
              push r8
              push r9
              push r10
              push r11"
             :::: "intel", "volatile");
    }
}

macro_rules! restore_regs
{
    () =>
    {
        asm!("pop r11
              pop r10
              pop r9
              pop r8
              pop rdi
              pop rsi
              pop rdx
              pop rcx
              pop rax"
             :::: "intel", "volatile");
    }
}

macro_rules! wrap_handler
{
    ($name : ident) =>
    {{
        #[naked]
        extern "C" fn wrapper() -> !
        {
            unsafe
            {
                /*
                 * To calculate the address of the exception stack frame, we add 0x48 bytes
                 * (9 registers times 64-bits). We don't need to align the stack; it should be
                 * aligned already.
                 */
                save_regs!();
                asm!("mov rdi, rsp
                      add rdi, 0x48
                      call $0"
                     :: "i"($name as extern "C" fn(&ExceptionStackFrame))
                     : "rdi"
                     : "intel", "volatile");
                restore_regs!();
                asm!("iretq" :::: "intel", "volatile");
                ::core::intrinsics::unreachable();
            }
        }
        wrapper
    }}
}

macro_rules! wrap_handler_with_error_code
{
    ($name : ident) =>
    {{
         #[naked]
         extern "C" fn wrapper() -> !
         {
             unsafe
             {
                 /*
                  * We also need to skip the saved callee registers here, but also need to skip
                  * over the error code to get to the exception stack frame, so we skip 0x50 bytes.
                  * However, in this case we also need to manually align the stack.
                  */
                 save_regs!();
                 asm!("mov rsi, [rsp+0x48]  // Put the error code into RSI
                       mov rdi, rsp
                       add rdi, 0x50
                       sub rsp, 8           // Align the stack pointer
                       call $0
                       add rsp, 8           // Undo the stack alignment"
                      :: "i"($name as extern "C" fn(&ExceptionStackFrame,u64))
                      : "rdi","rsi"
                      : "intel", "volatile");
                 restore_regs!();
                 asm!("add rsp, 8   // Pop the error code
                       iretq" :::: "intel", "volatile");
                 ::core::intrinsics::unreachable();
             }
         }
         wrapper
    }}
}

static mut IDT : Idt = Idt::new();

pub fn init<A>(memory_controller    : &mut MemoryController<A>,
               gdt_selectors        : &GdtSelectors)
    where A : FrameAllocator
{
    unsafe
    {
        /*
         * We want to use the APIC, so we remap and disable the legacy PIC.
         * XXX: We do this regardless of whether ACPI tells us we need to, because some chipsets
         *      lie.
         */
        let mut legacy_pic = ::i8259_pic::PIC_PAIR.lock();
        legacy_pic.remap();
        legacy_pic.disable();

        /*
         * We write 0 to CR8 (the Task Priority Register) to say that we want to recieve all
         * interrupts.
         */
        write_control_reg!(cr8, 0u64);

        /*
         * Install exception handlers
         */
        IDT.nmi()                       .set_handler(wrap_handler!(nmi_handler),                                        gdt_selectors.kernel_code);
        IDT.breakpoint()                .set_handler(wrap_handler!(breakpoint_handler),                                 gdt_selectors.kernel_code);
        IDT.invalid_opcode()            .set_handler(wrap_handler!(invalid_opcode_handler),                             gdt_selectors.kernel_code);
        IDT.general_protection_fault()  .set_handler(wrap_handler_with_error_code!(general_protection_fault_handler),   gdt_selectors.kernel_code);
        IDT.page_fault()                .set_handler(wrap_handler_with_error_code!(page_fault_handler),                 gdt_selectors.kernel_code);
        IDT.double_fault()              .set_handler(wrap_handler_with_error_code!(double_fault_handler),               gdt_selectors.kernel_code).set_ist_handler(::tss::DOUBLE_FAULT_IST_INDEX as u8);

        /*
         * Install handlers for local APIC interrupts
         */
        IDT[LOCAL_APIC_TIMER].set_handler(wrap_handler!(apic_timer_handler),        gdt_selectors.kernel_code);
        IDT[APIC_SPURIOUS_INTERRUPT].set_handler(wrap_handler!(spurious_handler),   gdt_selectors.kernel_code);

        /*
         * Install handlers for ISA IRQs from the IOAPIC
         */
        IDT.apic_irq(0).set_handler(wrap_handler!(pit_handler), gdt_selectors.kernel_code);
        IDT.apic_irq(1).set_handler(wrap_handler!(key_handler), gdt_selectors.kernel_code);

        IDT.load();

        /*
         * Unmask handled entries on the IOAPIC
         */
        IO_APIC.lock().set_irq_mask(2, false);  // PIT
        IO_APIC.lock().set_irq_mask(1, false);  // PS/2 controller
    }
}

extern "C" fn invalid_opcode_handler(stack_frame : &ExceptionStackFrame)
{
    error!("INVALID OPCODE AT: {:#x}", stack_frame.instruction_pointer);
    loop {}
}

extern "C" fn nmi_handler(_ : &ExceptionStackFrame)
{
    info!("NMI occured!");
}

extern "C" fn breakpoint_handler(stack_frame : &ExceptionStackFrame)
{
    info!("BREAKPOINT: {:#?}", stack_frame);
}

extern "C" fn general_protection_fault_handler(stack_frame : &ExceptionStackFrame, error_code : u64)
{
    error!("General protection fault: (error code = {:#x})", error_code);
    error!("{:#?}", stack_frame);
    loop { }
}

extern "C" fn page_fault_handler(stack_frame : &ExceptionStackFrame, error_code  : u64)
{
    error!("PAGE_FAULT: {} ({:#x})", match (/* U/S (User/Supervisor )*/(error_code >> 2) & 0b1,
                                            /* I/D (Instruction/Data)*/(error_code >> 4) & 0b1,
                                            /* R/W (Read/Write      )*/(error_code >> 1) & 0b1,
                                            /*  P  (Present         )*/(error_code >> 0) & 0b1)
    {
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

        (_, _, _, _) => { panic!("INVALID PAGE-FAULT ERROR CODE"); },
    },
    read_control_reg!(cr2));    // CR2 holds the address of the page that caused the #PF

    error!("{:#?}", stack_frame);

    /*
     * Page-faults can be recovered from and so are faults, but we never will so just give up.
     */
    loop { }
}

extern "C" fn double_fault_handler(stack_frame : &ExceptionStackFrame, error_code : u64)
{
    error!("EXCEPTION: DOUBLE FAULT   (Error code: {})\n{:#?}", error_code, stack_frame);
    loop { }
}

extern "C" fn pit_handler(_ : &ExceptionStackFrame)
{
    // XXX: Printing here seems to lock everything up (probably due to the contention on the mutex
    // involved) so probably avoid that.
    LOCAL_APIC.lock().send_eoi();
}

extern "C" fn apic_timer_handler(_ : &ExceptionStackFrame)
{
    trace!("APIC Tick");
    LOCAL_APIC.lock().send_eoi();
}

static KEYBOARD_PORT : Port<u8> = unsafe { Port::new(0x60) };

extern "C" fn key_handler(_ : &ExceptionStackFrame)
{
    info!("Key interrupt: Scancode={:#x}", unsafe { KEYBOARD_PORT.read() });
    LOCAL_APIC.lock().send_eoi();
}

extern "C" fn spurious_handler(_ : &ExceptionStackFrame) { }
