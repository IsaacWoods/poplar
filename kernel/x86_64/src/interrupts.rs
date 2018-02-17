/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use core::fmt;
use tss::Tss;
use gdt::{Gdt,GdtDescriptor,DescriptorFlags};
use idt::Idt;
use memory::{FrameAllocator,MemoryController};
use port::Port;

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

const DOUBLE_FAULT_IST_INDEX : usize = 0;

static mut TSS : Option<Tss> = None;
static mut GDT : Option<Gdt> = None;
static mut IDT : Option<Idt> = None;

fn unwrap_option<'a, T>(option : &'a mut Option<T>) -> &'a mut T
{
    match option
    {
        &mut Some(ref mut value) => value,
        &mut None => panic!("Tried to unwrap None option"),
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
                 * (9 registers X 64-bits). We don't need to align the stack; it should be aligned
                 * already.
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

pub fn init<A>(memory_controller : &mut MemoryController<A>) where A : FrameAllocator
{
    /*
     * Allocate a 4KiB stack for the double-fault handler. Using a separate stack avoids a triple
     * fault happening when the guard page of the normal stack is hit (after a stack overflow),
     * which would otherwise:
     *      Page Fault -> Page Fault -> Double Fault -> Page Fault -> Triple Fault
     */
    let double_fault_stack = memory_controller.alloc_stack(1).expect("Failed to allocate stack");

    unsafe
    {
        // Create a TSS
        TSS = Some(Tss::new());
        unwrap_option(&mut TSS).interrupt_stack_table[DOUBLE_FAULT_IST_INDEX] = double_fault_stack.top();

        /*
         * We need a new GDT, because the current one resides in the bootstrap and so is now not
         * mapped into the address space.
         * XXX: We don't bother creating a new data segment. This relies on all the segment
         * registers (*especially SS*) being cleared (which should've been done in the bootstrap).
         */
        GDT = Some(Gdt::new());
        let code_selector = unwrap_option(&mut GDT).add_entry(GdtDescriptor::UserSegment((DescriptorFlags::USER_SEGMENT  |
                                                                                          DescriptorFlags::PRESENT       |
                                                                                          DescriptorFlags::EXECUTABLE    |
                                                                                          DescriptorFlags::LONG_MODE).bits()));
        let tss_selector = unwrap_option(&mut GDT).add_entry(GdtDescriptor::create_tss_segment(unwrap_option(&mut TSS)));
        unwrap_option(&mut GDT).load(code_selector, tss_selector);

        // Create the IDT
        IDT = Some(Idt::new());

        /* #BP */unwrap_option(&mut IDT).breakpoint()    .set_handler(wrap_handler!(breakpoint_handler),                     code_selector);
        /* #UD */unwrap_option(&mut IDT).invalid_opcode().set_handler(wrap_handler!(invalid_opcode_handler),                 code_selector);
        /* #PF */unwrap_option(&mut IDT).page_fault()    .set_handler(wrap_handler_with_error_code!(page_fault_handler),     code_selector);
        /* #DF */unwrap_option(&mut IDT).double_fault()  .set_handler(wrap_handler_with_error_code!(double_fault_handler),   code_selector)
                                                         .set_ist_handler(DOUBLE_FAULT_IST_INDEX as u8);
        unwrap_option(&mut IDT).irq(0).set_handler(wrap_handler!(timer_handler), code_selector);
        unwrap_option(&mut IDT).irq(1).set_handler(wrap_handler!(key_handler), code_selector);
        unwrap_option(&mut IDT).load();
    }
}

extern "C" fn invalid_opcode_handler(stack_frame : &ExceptionStackFrame)
{
    println!("INVALID OPCODE AT: {:#x}", stack_frame.instruction_pointer);
    loop {}
}

extern "C" fn breakpoint_handler(stack_frame : &ExceptionStackFrame)
{
    println!("BREAKPOINT: {:#?}", stack_frame);
}

extern "C" fn page_fault_handler(stack_frame : &ExceptionStackFrame, error_code  : u64)
{
    println!("PAGE_FAULT: {} ({:#x})", match (/*  P  (Present        ) */(error_code >> 0) & 0b1,
                                              /* R/W (Read/Write     ) */(error_code >> 1) & 0b1,
                                              /* U/S (User/Supervisor) */(error_code >> 2) & 0b1)
    {
        (0, 0, 0) => "Kernel tried to read a non-present page"                                      ,
        (0, 0, 1) => "Kernel tried to read a non-present page, causing a protection fault"          ,
        (0, 1, 0) => "Kernel tried to write to a non-present page"                                  ,
        (0, 1, 1) => "Kernel tried to write to a non-present page, causing a protection fault"      ,
        (1, 0, 0) => "User process tried to read a non-present page"                                ,
        (1, 0, 1) => "User process tried to read a non-present page, causing a protection fault"    ,
        (1, 1, 0) => "User process tried to write to a non-present page"                            ,
        (1, 1, 1) => "User process tried to write to a non-present page, causing a protection fault",

        (_, _, _) => { panic!("UNRECOGNISED PAGE-FAULT ERROR CODE"); },
    },
    read_control_reg!(cr2));    // CR2 holds the address of the page that caused the #PF

    println!("{:#?}", stack_frame);

    /*
     * Page-faults can be recovered from and so are faults, but we never will so just give up.
     */
    loop { }
}

extern "C" fn double_fault_handler(stack_frame : &ExceptionStackFrame, error_code : u64)
{
    println!("EXCEPTION: DOUBLE FAULT   (Error code: {})\n{:#?}", error_code, stack_frame);
    loop { }
}

extern "C" fn timer_handler(_ : &ExceptionStackFrame)
{
    println!("Tick");
    ::apic::LOCAL_APIC.lock().send_eoi();
}

static KEYBOARD_PORT : Port<u8> = unsafe { Port::new(0x60) };

extern "C" fn key_handler(_ : &ExceptionStackFrame)
{
    println!("Key interrupt: Scancode={:#x}", unsafe { KEYBOARD_PORT.read() });

    unsafe { ::i8259_pic::PIC_PAIR.lock().send_eoi(33); }
}
