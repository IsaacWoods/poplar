mod exception;
mod pci;

use acpi::{interrupt::InterruptModel, Acpi};
use aml::{value::Args as AmlArgs, AmlContext, AmlName, AmlValue};
use bit_field::BitField;
use core::time::Duration;
use hal::memory::PhysicalAddress;
use hal_x86_64::{
    hw::{
        cpu::CpuInfo,
        gdt::KERNEL_CODE_SELECTOR,
        i8259_pic::Pic,
        idt::{wrap_handler, wrap_handler_with_error_code, Idt, InterruptStackFrame},
        local_apic::LocalApic,
        registers::write_msr,
    },
    kernel_map,
};
use log::warn;
use pci::PciResolver;
use pebble_util::InitGuard;

global_asm!(include_str!("syscall.s"));
extern "C" {
    fn syscall_handler() -> !;
}

#[no_mangle]
extern "C" fn rust_syscall_entry(number: usize, a: usize, b: usize, c: usize, d: usize, e: usize) -> usize {
    kernel::syscall::handle_syscall::<crate::PlatformImpl>(number, a, b, c, d, e)
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

static LOCAL_APIC: InitGuard<LocalApic> = InitGuard::uninit();

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
    pub fn init(acpi_info: &Acpi, aml_context: &mut AmlContext) -> InterruptController {
        Self::install_syscall_handler();
        Self::install_exception_handlers();

        unsafe {
            IDT.load();
        }

        match acpi_info.interrupt_model.as_ref().unwrap() {
            InterruptModel::Apic(info) => {
                if info.also_has_legacy_pics {
                    unsafe { Pic::new() }.remap_and_disable(LEGACY_PIC_VECTOR, LEGACY_PIC_VECTOR + 8);
                }

                /*
                 * Initialise `LOCAL_APIC` to point at the right address.
                 * TODO: we might need to map it separately or something so we can set custom flags on the
                 * paging entry (do we need to set NO_CACHE on it?)
                 */
                // TODO: change the region to be NO_CACHE
                LOCAL_APIC.initialize(unsafe {
                    LocalApic::new(kernel_map::physical_to_virtual(
                        PhysicalAddress::new(info.local_apic_address as usize).unwrap(),
                    ))
                });

                /*
                 * Tell ACPI that we intend to use the APICs instead of the legacy PIC.
                 */
                aml_context
                    .invoke_method(
                        &AmlName::from_str("\\_PIC").unwrap(),
                        AmlArgs { arg_0: Some(AmlValue::Integer(1)), ..Default::default() },
                    )
                    .expect("Failed to invoke \\_PIC method");

                /*
                 * Resolve all the PCI info.
                 * XXX: not sure this is the right place to do this just yet.
                 */
                let pci_info = PciResolver::resolve(acpi_info.pci_config_regions.as_ref().unwrap(), aml_context);

                /*
                 * Install handlers for the spurious interrupt and local APIC timer, and then
                 * enable the local APIC.
                 * Install a spurious interrupt handler and enable the local APIC.
                 */
                unsafe {
                    IDT[APIC_TIMER_VECTOR]
                        .set_handler(wrap_handler!(local_apic_timer_handler), KERNEL_CODE_SELECTOR);
                    IDT[APIC_SPURIOUS_VECTOR].set_handler(wrap_handler!(spurious_handler), KERNEL_CODE_SELECTOR);
                    LOCAL_APIC.get().enable(APIC_SPURIOUS_VECTOR);
                }

                InterruptController {}
            }

            _ => panic!("Unsupported interrupt model!"),
        }
    }

    /// Enable the per-CPU timer on the local APIC, so that it ticks every `period` ms. Cannot be
    /// called before interrupt handlers are installed, because this borrows `self`.
    pub fn enable_local_timer(&mut self, cpu_info: &CpuInfo, period: Duration) {
        /*
         * TODO: currently, this relies upon being able to get the frequency from the
         * CpuInfo. We should probably build a backup to calibrate it using another timer.
         */
        match cpu_info.apic_frequency() {
            Some(apic_frequency) => {
                LOCAL_APIC.get().enable_timer(period.as_millis() as u32, apic_frequency, APIC_TIMER_VECTOR);
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

    /// We use the `syscall` instruction to make system calls, as it's always present on supported systems. We need
    /// to set a few MSRs to configure how the `syscall` instruction works:
    ///     - `IA32_LSTAR` contains the address that `syscall` jumps to
    ///     - `IA32_STAR` contains the segment selectors that `syscall` and `sysret` use. These are not validated
    ///       against the actual contents of the GDT so these must be correct.
    ///     - `IA32_FMASK` contains a mask used to set the kernels `rflags`
    ///
    /// To understand the values we set these MSRs to, it helps to look at a (simplified) version of the operation
    /// of `syscall`:
    /// ```
    ///     rcx <- rip
    ///     rip <- IA32_LSTAR
    ///     r11 <- rflags
    ///     rflags <- rflags AND NOT(IA32_FMASK)
    ///
    ///     cs.selector <- IA32_STAR[47:32] AND 0xfffc          (the AND forces the RPL to 0)
    ///     cs.base <- 0                                        (note that for speed, the selector is not actually
    ///     cs.limit <- 0xfffff                                  loaded from the GDT, but hardcoded)
    ///     cs.type <- 11;
    ///     (some other fields of the selector, see Intel manual for details)
    ///
    ///     cpl <- 0
    ///
    ///     ss.selector <- IA32_STAR[47:32] + 8
    ///     ss.base <- 0
    ///     ss.limit <- 0xfffff
    ///     ss.type <- 3
    ///     (some other fields of the selector)
    /// ```
    fn install_syscall_handler() {
        use hal_x86_64::hw::{
            gdt::USER_COMPAT_CODE_SELECTOR,
            registers::{CpuFlags, IA32_FMASK, IA32_LSTAR, IA32_STAR},
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

        /*
         * Upon `syscall`, `rflags` is moved into `r11`, and then the bits set in this mask are cleared. We
         * clear some stuff so the kernel runs in a sensible environment, regardless of what usermode is
         * doing.
         *
         * Importantly, we disable interrupts because they're not safe until we've stopped messing about with
         * stacks.
         */
        let flags_mask = CpuFlags::STATUS_MASK
            | CpuFlags::TRAP_FLAG
            | CpuFlags::INTERRUPT_ENABLE_FLAG
            | CpuFlags::IO_PRIVILEGE_MASK
            | CpuFlags::NESTED_TASK_FLAG
            | CpuFlags::ALIGNMENT_CHECK_FLAG;

        unsafe {
            write_msr(IA32_STAR, selectors);
            write_msr(IA32_LSTAR, syscall_handler as u64);
            write_msr(IA32_FMASK, flags_mask);
        }
    }
}

extern "C" fn local_apic_timer_handler(_: &InterruptStackFrame) {
    unsafe {
        LOCAL_APIC.get().send_eoi();
    }
}

extern "C" fn spurious_handler(_: &InterruptStackFrame) {}
