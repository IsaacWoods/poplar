/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

mod idt;
mod pic;

use x86_64::tss::Tss;
use x86_64::gdt::{Gdt,GdtDescriptor,DescriptorFlags};
use memory::{FrameAllocator,MemoryController};
use rustos_common::port::Port;
use self::idt::Idt;
use self::pic::PicPair;

#[derive(Debug)]
#[repr(C)]
struct ExceptionStackFrame
{
    instruction_pointer : u64,
    code_segment        : u64,
    cpu_flags           : u64,
    stack_pointer       : u64,
    stack_segment       : u64,
}

const DOUBLE_FAULT_IST_INDEX : usize = 0;

static mut TSS : Option<Tss> = None;
static mut GDT : Option<Gdt> = None;
static mut IDT : Option<Idt> = None;
//static PIC_PAIR : Mutex<PicPair> = Mutex::new(unsafe { PicPair::new(0x20, 0x28) });

fn unwrap_option<'a, T>(option : &'a mut Option<T>) -> &'a mut T
{
    match option
    {
        &mut Some(ref mut value) => value,
        &mut None => panic!("Tried to unwrap None option"),
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
                asm!("mov rdi, rsp
                      sub rsp, 8        // Align the stack pointer
                      call $0"
                     :: "i"($name as extern "C" fn(&ExceptionStackFrame) -> !)
                     : "rdi"
                     : "intel");
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
                 asm!("pop rsi          // Pop the error code
                       mov rdi, rsp
                       sub rsp, 8       // Align the stack pointer
                       call $0"
                      :: "i"($name as extern "C" fn(&ExceptionStackFrame,u64) -> !)
                      : "rdi","rsi"
                      : "intel");
                 ::core::intrinsics::unreachable();
             }
         }
         wrapper
    }}
}

pub fn init<A>(memory_controller : &mut MemoryController<A>) where A : FrameAllocator
{
    /*
     * Allocate a 4KiB stack for the double-fault handler. Using a separate stack
     * avoids a triple fault happening when the guard page of the normal stack is hit (after a stack
     * overflow) which would otherwise
     *      Page Fault -> Page Fault -> Double Fault -> Page Fault -> Triple Fault
     */
    let double_fault_stack = memory_controller.alloc_stack(1).expect("Failed to allocate double-fault stack");

    unsafe
    {
        // Create a TSS
        TSS = Some(Tss::new());
        unwrap_option(&mut TSS).interrupt_stack_table[DOUBLE_FAULT_IST_INDEX] = double_fault_stack.top();

        /*
         * We need a new GDT, because the current one resides in the bootstrap and so is now not
         * mapped into the address space
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
        unwrap_option(&mut IDT).breakpoint().set_handler(wrap_handler!(breakpoint_handler), code_selector)
                                            .set_ist_handler(DOUBLE_FAULT_IST_INDEX as u8);
        unwrap_option(&mut IDT).invalid_opcode().set_handler(wrap_handler!(invalid_opcode_handler), code_selector);
        unwrap_option(&mut IDT).page_fault().set_handler(wrap_handler_with_error_code!(page_fault_handler), code_selector);
        unwrap_option(&mut IDT).load();

        // PIC_PAIR.lock().remap();
    }
}

extern "C" fn invalid_opcode_handler(stack_frame : &ExceptionStackFrame) -> !
{
    println!("INVALID OPCODE AT: {:#x}", stack_frame.instruction_pointer);
    loop {}
}

extern "C" fn breakpoint_handler(stack_frame : &ExceptionStackFrame) -> !
{
    println!("BREAKPOINT: {:#?}", stack_frame);
    loop {}
}

extern "C" fn page_fault_handler(stack_frame : &ExceptionStackFrame, error_code  : u64) -> !
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
    read_control_reg!(cr2));        // CR2 holds the address of the page that caused the #PF


    println!("\n{:#?}", stack_frame);

    /*
     * Page-faults can be recovered from and so are faults, but we never will so just give up.
     */
    loop { }
}
/*
extern "x86-interrupt" fn double_fault_handler(stack_frame : &mut ExceptionStackFrame,
                                               error_code : u64)
{
    println!("EXCEPTION: DOUBLE FAULT   (Error code: {})\n{:#?}", error_code, stack_frame);
    loop { }
}

extern "x86-interrupt" fn timer_handler(_ : &mut ExceptionStackFrame)
{
    unsafe { PIC_PAIR.lock().send_eoi(32); }
}

static KEYBOARD_PORT : Port<u8> = unsafe { Port::new(0x60) };

extern "x86-interrupt" fn key_handler(_ : &mut ExceptionStackFrame)
{
    println!("Key interrupt: Scancode={:#x}", unsafe { KEYBOARD_PORT.read() });

    unsafe { PIC_PAIR.lock().send_eoi(33); }
}*/
