mod exception;

use super::Arch;
use acpi::interrupt::InterruptModel;
use aml::{value::Args as AmlArgs, AmlName, AmlValue};
use bit_field::BitField;
use core::time::Duration;
use log::{info, warn};
use x86_64::{
    hw::{
        gdt::KERNEL_CODE_SELECTOR,
        i8259_pic::Pic,
        idt::{Idt, InterruptStackFrame},
        local_apic::LocalApic,
        registers::write_msr,
    },
    memory::{kernel_map, EntryFlags, Frame, Page, PhysicalAddress},
};

global_asm!(include_str!("syscall.s"));
extern "C" {
    fn syscall_handler() -> !;
}

/// This should only be accessed directly by the bootstrap processor.
///
/// The IDT is laid out like so:
/// |------------------|-----------------------------|
/// | Interrupt Vector |            Usage            |
/// |------------------|-----------------------------|
/// |       00-1f      | Intel Reserved (Exceptions) |
/// |       20-2f      | i8259 PIC Interrupts        |
/// |       30-??      | IOAPIC Interrupts           |
/// |        ..        |                             |
/// |        fe        | Local APIC timer
/// |        ff        | APIC spurious interrupt     |
/// |------------------|-----------------------------|
static mut IDT: Idt = Idt::empty();

/*
 * These constants define the IDT's layout. Refer to the documentation of the `IDT` static for
 * the full layout.
 */
const LEGACY_PIC_VECTOR: u8 = 0x20;
const FREE_VECTORS_START: u8 = 0x30;
const APIC_TIMER_VECTOR: u8 = 0xfe;
const APIC_SPURIOUS_VECTOR: u8 = 0xff;

pub struct InterruptController {}

impl InterruptController {
    pub fn init(arch: &Arch) -> InterruptController {
        Self::install_syscall_handler();
        Self::install_exception_handlers();

        unsafe {
            IDT.load();
        }

        match arch.acpi_info.as_ref().unwrap().interrupt_model.as_ref().unwrap() {
            InterruptModel::Apic {
                local_apic_address,
                io_apics: acpi_io_apics,
                ref local_apic_nmi_line,
                ref interrupt_source_overrides,
                ref nmi_sources,
                also_has_legacy_pics,
            } => {
                if *also_has_legacy_pics {
                    unsafe { Pic::new() }.remap_and_disable(LEGACY_PIC_VECTOR, LEGACY_PIC_VECTOR + 8);
                }

                /*
                 * Tell ACPI that we intend to use the APICs instead of the legacy PIC.
                 */
                arch.aml_context
                    .lock()
                    .invoke_method(
                        &AmlName::from_str("\\_PIC").unwrap(),
                        AmlArgs { arg_0: Some(AmlValue::Integer(1)), ..Default::default() },
                    )
                    .expect("Failed to invoke \\_PIC method");

                /*
                 * Map the local APIC's configuration space into the kernel address space.
                 */
                arch.kernel_page_table
                    .lock()
                    .mapper()
                    .map_to(
                        Page::contains(kernel_map::LOCAL_APIC_CONFIG),
                        Frame::contains(PhysicalAddress::new(*local_apic_address as usize).unwrap()),
                        EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NO_EXECUTE | EntryFlags::NO_CACHE,
                        &arch.physical_memory_manager,
                    )
                    .unwrap();

                /*
                 * Install handlers for the spurious interrupt and local APIC timer, and then
                 * enable the local APIC.
                 * Install a spurious interrupt handler and enable the local APIC.
                 */
                unsafe {
                    IDT[APIC_TIMER_VECTOR]
                        .set_handler(wrap_handler!(local_apic_timer_handler), KERNEL_CODE_SELECTOR);
                    IDT[APIC_SPURIOUS_VECTOR].set_handler(wrap_handler!(spurious_handler), KERNEL_CODE_SELECTOR);
                    LocalApic::enable(APIC_SPURIOUS_VECTOR);
                }

                InterruptController {}
            }

            _ => panic!("Unsupported interrupt model!"),
        }
    }

    /// Enable the per-CPU timer on the local APIC, so that it ticks every `period` ms. Cannot be
    /// called before interrupt handlers are installed, because this borrows `self`.
    pub fn enable_local_timer(&mut self, arch: &Arch, period: Duration) {
        /*
         * TODO: currently, this relies upon being able to get the frequency from the
         * CpuInfo. We should probably build a backup to calibrate it using another timer.
         */
        match arch.cpu_info.apic_frequency() {
            Some(apic_frequency) => {
                LocalApic::enable_timer(period.as_millis() as u32, apic_frequency, APIC_TIMER_VECTOR)
            }
            None => warn!("Couldn't find frequency of APIC from cpuid. Local APIC timer not enabled!"),
        }
    }

    fn install_exception_handlers() {
        macro set_handler($name: ident, $handler: path) {
            unsafe {
                IDT.$name().set_handler(wrap_handler!($handler), KERNEL_CODE_SELECTOR);
            }
        }

        macro set_handler_with_error_code($name: ident, $handler: path) {
            unsafe {
                IDT.$name().set_handler(wrap_handler_with_error_code!($handler), KERNEL_CODE_SELECTOR);
            }
        }

        set_handler!(nmi, exception::nmi_handler);
        set_handler!(breakpoint, exception::breakpoint_handler);
        set_handler!(invalid_opcode, exception::invalid_opcode_handler);
        set_handler_with_error_code!(general_protection_fault, exception::general_protection_fault_handler);
        set_handler_with_error_code!(page_fault, exception::page_fault_handler);
        set_handler_with_error_code!(double_fault, exception::double_fault_handler);
    }

    fn install_syscall_handler() {
        /*
         * On x86_64, the `syscall` instruction will always be present, so we only support that
         * for making system calls.
         *
         * Refer to the documentation comments of each MSR to understand what this code is doing.
         */
        use x86_64::hw::{
            gdt::USER_COMPAT_CODE_SELECTOR,
            registers::{IA32_FMASK, IA32_LSTAR, IA32_STAR},
        };

        let mut selectors = 0_u64;
        selectors.set_bits(32..48, KERNEL_CODE_SELECTOR.0 as u64);

        /*
         * We put the selector for the Compatibility-mode code segment in here, because `sysret` expects
         * the segments to be in this order:
         *      STAR[48..64]        => 32-bit Code Segment
         *      STAR[48..64] + 8    => Data Segment
         *      STAR[48..64] + 16   => 64-bit Code Segment
         */
        selectors.set_bits(48..64, USER_COMPAT_CODE_SELECTOR.0 as u64);

        unsafe {
            write_msr(IA32_STAR, selectors);
            write_msr(IA32_LSTAR, syscall_handler as u64);
            write_msr(IA32_FMASK, 0);
        }
    }
}

extern "C" fn local_apic_timer_handler(_: &InterruptStackFrame) {
    info!("Tick!");
    unsafe {
        LocalApic::send_eoi();
    }
}

extern "C" fn spurious_handler(_: &InterruptStackFrame) {}

/// Macro to save the scratch registers. In System-V, `rbx`, `rbp`, `r12`, `r13`, `r14`, and `r15`
/// must be restored by the callee, so Rust automatically generates code to restore them, but for
/// the rest we have to manually preserve them. Use `restore_regs` to restore the scratch registers
/// before returning from the handler.
macro save_regs() {
    asm!("push rax
          push rcx
          push rdx
          push rsi
          push rdi
          push r8
          push r9
          push r10
          push r11"
        :
        :
        :
        : "intel"
        );
}

/// Restore the saved scratch registers.
macro restore_regs() {
    asm!("pop r11
          pop r10
          pop r9
          pop r8
          pop rdi
          pop rsi
          pop rdx
          pop rcx
          pop rax"
        :
        :
        :
        : "intel"
        );
}

macro wrap_handler($name: path) {
    {
        #[naked]
        extern "C" fn wrapper() -> ! {
            unsafe {
                /*
                 * To calculate the address of the exception stack frame, we add 0x48 bytes (9
                 * 64-bit registers). We don't need to manually align the stack, as it should
                 * already be aligned correctly.
                 */
                save_regs!();
                asm!("mov rdi, rsp
                      add rdi, 0x48
                      call $0"
                    :
                    : "i"($name as extern "C" fn(&InterruptStackFrame))
                    : "rdi"
                    : "intel"
                    );
                restore_regs!();
                asm!("iretq"
                     :
                     :
                     :
                     : "intel"
                     );
                unreachable!();
            }
        }

        wrapper
    }
}

macro wrap_handler_with_error_code($name: path) {
    {
        #[naked]
        extern "C" fn wrapper() -> ! {
            unsafe {
                /*
                 * To calculate the address of the exception stack frame, we add 0x48 bytes (9
                 * 64-bit registers), and then the two bytes of the error code. Because we skip
                 * 0x50 bytes, we need to manually align the stack.
                 */
                save_regs!();
                asm!("mov rsi, [rsp+0x48]   // Put the error code in RSI
                      mov rdi, rsp
                      add rdi, 0x50
                      sub rsp, 8            // Align the stack pointer
                      call $0
                      add rsp, 8            // Restore the stack pointer"
                     :
                     : "i"($name as extern "C" fn(&InterruptStackFrame, _error_code: u64))
                     : "rdi", "rsi"
                     : "intel"
                    );
                restore_regs!();
                asm!("add rsp, 8            // Pop the error code
                      iretq"
                     :
                     :
                     :
                     : "intel"
                    );
                unreachable!();
            }
        }

        wrapper
    }
}
