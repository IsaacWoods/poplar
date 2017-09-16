/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

mod gdt;
mod pic;

use spin::{Once,Mutex};
use x86_64::VirtualAddress;
use x86_64::structures::gdt::SegmentSelector;
use x86_64::structures::idt::{Idt,ExceptionStackFrame,PageFaultErrorCode};
use x86_64::structures::tss::TaskStateSegment;
use memory::{FrameAllocator,MemoryController};
use rustos_common::port::Port;
use self::pic::{PicPair};

const DOUBLE_FAULT_IST_INDEX : usize = 0;

lazy_static!
{
    static ref IDT : Idt =
        {
            let mut idt = Idt::new();
            idt.breakpoint.set_handler_fn(breakpoint_handler);
            idt.page_fault.set_handler_fn(page_fault_handler);

            unsafe
            {
                idt.double_fault.set_handler_fn(double_fault_handler)
                                .set_stack_index(DOUBLE_FAULT_IST_INDEX as u16);
            }

            idt[32].set_handler_fn(timer_handler);  // IRQ0
            idt[33].set_handler_fn(key_handler);    // IRQ1

            idt
        };
}

static TSS : Once<TaskStateSegment> = Once::new();
static GDT : Once<gdt::Gdt>         = Once::new();

static PIC_PAIR : Mutex<PicPair> = Mutex::new(unsafe { PicPair::new(0x20, 0x28) });

pub fn init<A>(memory_controller : &mut MemoryController<A>) where A : FrameAllocator
{
    use x86_64::instructions::segmentation::set_cs;
    use x86_64::instructions::tables::load_tss;

    /*
     * Allocate a 4096 byte (1 page) stack for the double-fault handler. Using a separate stack
     * avoids a triple fault happening when the guard page of the normal stack is hit (after a stack
     * overflow) which would otherwise
     *      Page Fault -> Page Fault -> Double Fault -> Page Fault -> Triple Fault
     */
    let double_fault_stack = memory_controller.alloc_stack(1).expect("Failed to allocate double-fault stack");

    let tss = TSS.call_once(
        || {
            let mut tss = TaskStateSegment::new();
            tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX] = VirtualAddress(double_fault_stack.top());
            tss
        });

    let mut code_selector = SegmentSelector(0);
    let mut tss_selector  = SegmentSelector(0);
    let gdt = GDT.call_once(
        || {
            let mut gdt = gdt::Gdt::new();
            code_selector = gdt.add_entry(gdt::Descriptor::create_kernel_code_segment());
            tss_selector  = gdt.add_entry(gdt::Descriptor::create_tss_segment(&tss));
            gdt
        });
    
    gdt.load();

    unsafe
    {
        /*
         * Reload the GDT
         */
        set_cs(code_selector);
        load_tss(tss_selector);
    }

    IDT.load();
    unsafe
    {
        PIC_PAIR.lock().remap();
    }
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame : &mut ExceptionStackFrame)
{
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn page_fault_handler(stack_frame : &mut ExceptionStackFrame,
                                             error_code  : PageFaultErrorCode)
{
    println!("PAGE FAULT!");

    if error_code.contains(PageFaultErrorCode::PROTECTION_VIOLATION)    { println!("  * Caused by a page-protection-violation");                                }
                                                                   else { println!("  * Caused by a not-present page");                                         }

    if error_code.contains(PageFaultErrorCode::INSTRUCTION_FETCH)
    {
        println!("  * Caused by an instruction fetch");
    }
    else
    {
        if error_code.contains(PageFaultErrorCode::CAUSED_BY_WRITE)     { println!("  * Caused by an invalid write (maybe)");                                   }
                                                                   else { println!("  * Caused by an invalid read (maybe)");                                    }
    }

    if error_code.contains(PageFaultErrorCode::USER_MODE)               { println!("  * Occured in user-mode (CPL=3) (doesn't = privilege violation)");         }
                                                                   else { println!("  * Occured in supervisor mode (CPL=0,1,2) (not = privilege violation)");   }

    if error_code.contains(PageFaultErrorCode::MALFORMED_TABLE)         { println!("  * Something's fucky with a page table");                                  }

    println!("\n{:#?}", stack_frame);

    /*
     * Page-faults can be recovered from and so are faults, but we never will so just give up.
     */
    loop { }
}

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
}
