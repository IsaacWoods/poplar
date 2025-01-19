use acpi::InterruptModel;
use alloc::{alloc::Global, vec};
use aml::{value::Args as AmlArgs, AmlContext, AmlName, AmlValue};
use bit_field::BitField;
use core::{str::FromStr, time::Duration};
use hal::memory::PAddr;
use hal_x86_64::{
    hw::{
        cpu::CpuInfo,
        gdt::PrivilegeLevel,
        i8259_pic::Pic,
        idt::{
            wrap_handler,
            wrap_handler_with_error_code,
            ExceptionWithErrorStackFrame,
            Idt,
            InterruptStackFrame,
        },
        ioapic::{DeliveryMode, IoApic, PinPolarity, TriggerMode},
        lapic::LocalApic,
        registers::read_control_reg,
    },
    kernel_map,
};
use mulch::{BinaryPrettyPrint, InitGuard};
use spinning_top::Spinlock;
use tracing::{error, info, warn};

/// This should only be accessed directly by the bootstrap processor.
///
/// The IDT is laid out like so:
/// |------------------|-----------------------------|
/// | Interrupt Vector |            Usage            |
/// |------------------|-----------------------------|
/// |       00-1f      | Reserved for exceptions     |
/// |       20-2f      | ISA interrupts              |
/// |       30-fd      | Dynamically allocated       |
/// |        fe        | Local APIC timer            |
/// |        ff        | APIC spurious interrupt     |
/// |------------------|-----------------------------|
static IDT: Spinlock<Idt> = Spinlock::new(Idt::empty());
static LOCAL_APIC: InitGuard<LocalApic> = InitGuard::uninit();
static IO_APIC: InitGuard<IoApic> = InitGuard::uninit();

/*
 * Constants for allocated portions of the IDT. These should match the layout above.
 */
const ISA_INTERRUPTS_START: u8 = 0x20;
const NUM_ISA_INTERRUPTS: usize = 16;
const FREE_VECTORS_START: u8 = 0x30;
const APIC_TIMER_VECTOR: u8 = 0xfe;
const APIC_SPURIOUS_VECTOR: u8 = 0xff;

pub struct InterruptController {
    model: InterruptModel<Global>,
    isa_gsi_mappings: [u32; NUM_ISA_INTERRUPTS],
    // TODO: PCI routing information
    // TODO: dynamically allocate IDT entries
}

impl InterruptController {
    /// Install handlers for exceptions, and load the IDT. This is done early in initialization to catch issues
    /// like page faults and kernel stack overflows nicely.
    pub fn install_exception_handlers() {
        let mut idt = IDT.lock();
        idt.nmi().set_handler(wrap_handler!(nmi_handler));
        idt.breakpoint().set_handler(wrap_handler!(breakpoint_handler)).set_privilege_level(PrivilegeLevel::Ring3);
        idt.invalid_opcode().set_handler(wrap_handler!(invalid_opcode_handler));
        idt.general_protection_fault()
            .set_handler(wrap_handler_with_error_code!(general_protection_fault_handler));
        idt.page_fault().set_handler(wrap_handler_with_error_code!(page_fault_handler));
        idt.double_fault().set_handler(wrap_handler_with_error_code!(double_fault_handler));

        idt.load();
    }

    pub fn init(interrupt_model: InterruptModel<Global>, aml_context: &mut AmlContext) -> InterruptController {
        match &interrupt_model {
            InterruptModel::Apic(info) => {
                if info.also_has_legacy_pics {
                    unsafe { Pic::new() }.remap_and_disable(ISA_INTERRUPTS_START, ISA_INTERRUPTS_START + 8);
                }

                /*
                 * Initialise `LOCAL_APIC` to point at the right address.
                 */
                // TODO: change the region to be NO_CACHE
                let lapic = unsafe {
                    LocalApic::new(kernel_map::physical_to_virtual(
                        PAddr::new(info.local_apic_address as usize).unwrap(),
                    ))
                };
                LOCAL_APIC.initialize(lapic);

                /*
                 * Tell ACPI that we intend to use the APICs instead of the legacy PIC.
                 */
                aml_context
                    .invoke_method(
                        &AmlName::from_str("\\_PIC").unwrap(),
                        AmlArgs::from_list(vec![AmlValue::Integer(1)]).unwrap(),
                    )
                    .expect("Failed to invoke \\_PIC method");

                /*
                 * Install handlers for the spurious interrupt and local APIC timer, and then
                 * enable the local APIC.
                 */
                unsafe {
                    let mut idt = IDT.lock();
                    idt[APIC_TIMER_VECTOR].set_handler(wrap_handler!(local_apic_timer_handler));
                    idt[APIC_SPURIOUS_VECTOR].set_handler(wrap_handler!(spurious_handler));
                    LOCAL_APIC.get().enable(APIC_SPURIOUS_VECTOR);
                }

                assert!(info.io_apics.len() == 1);
                let io_apic_addr = hal_x86_64::kernel_map::physical_to_virtual(
                    PAddr::new(info.io_apics.first().unwrap().address as usize).unwrap(),
                );
                let mut io_apic = unsafe { IoApic::new(io_apic_addr, 0) };

                /*
                 * Process the MADT's interrupt source overrides. These define differences in how
                 * the IOAPIC is wired, compared to the standard IA-PC dual-i8259 wiring. Only ISA
                 * interrupts that are wired in a non-standard way are overridden, otherwise an
                 * identity mapping can be assumed.
                 *
                 * The default settings for the ISA bus are high pin-polarity and edge-triggered
                 * interrupts.
                 */
                let isa_gsi_mappings = {
                    struct IsaGsiMapping {
                        gsi: u32,
                        polarity: PinPolarity,
                        trigger_mode: TriggerMode,
                    }
                    let mut mappings: [IsaGsiMapping; NUM_ISA_INTERRUPTS] =
                        core::array::from_fn(|i| IsaGsiMapping {
                            gsi: i as u32,
                            polarity: PinPolarity::High,
                            trigger_mode: TriggerMode::Edge,
                        });
                    for entry in info.interrupt_source_overrides.iter() {
                        use acpi::platform::interrupt::{
                            Polarity as AcpiPolarity,
                            TriggerMode as AcpiTriggerMode,
                        };

                        mappings[entry.isa_source as usize].gsi = entry.global_system_interrupt;
                        mappings[entry.isa_source as usize].polarity = match entry.polarity {
                            AcpiPolarity::ActiveHigh => PinPolarity::High,
                            AcpiPolarity::ActiveLow => PinPolarity::Low,
                            AcpiPolarity::SameAsBus => PinPolarity::High,
                        };
                        mappings[entry.isa_source as usize].trigger_mode = match entry.trigger_mode {
                            AcpiTriggerMode::Edge => TriggerMode::Edge,
                            AcpiTriggerMode::Level => TriggerMode::Level,
                            AcpiTriggerMode::SameAsBus => TriggerMode::Edge,
                        };
                    }
                    for (isa, entry) in mappings.iter().enumerate() {
                        io_apic.write_entry(
                            entry.gsi,
                            ISA_INTERRUPTS_START + isa as u8,
                            DeliveryMode::Fixed,
                            entry.polarity,
                            entry.trigger_mode,
                            true,
                            0,
                        );
                    }
                    core::array::from_fn(|i| mappings[i].gsi)
                };

                IO_APIC.initialize(io_apic);

                InterruptController { model: interrupt_model, isa_gsi_mappings }
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
}

extern "C" fn local_apic_timer_handler(_: &InterruptStackFrame) {
    unsafe {
        LOCAL_APIC.get().send_eoi();
    }
}

extern "C" fn spurious_handler(_: &InterruptStackFrame) {}

/*
 * Exception handlers
 */
pub extern "C" fn nmi_handler(_: &InterruptStackFrame) {
    info!("NMI occured!");
}

pub extern "C" fn breakpoint_handler(stack_frame: &InterruptStackFrame) {
    info!("BREAKPOINT: {:#x?}", stack_frame);

    /*
     * TEMP: Do a stacktrace.
     */
    let mut rbp = stack_frame.rbp as usize;
    info!("Starting stacktrace. First frame is at: {:#x}", rbp);
    if rbp != 0 {
        for i in 0..16 {
            let next_rbp = unsafe { *(rbp as *const usize) };
            let return_address = unsafe { *((rbp + 8) as *const usize) };

            info!("     {}: return address: {:#x}. next frame is at: {:#x}", i, return_address, next_rbp);

            if next_rbp == 0x0 {
                break;
            } else {
                rbp = next_rbp;
            }
        }
    }
}

pub extern "C" fn invalid_opcode_handler(stack_frame: &InterruptStackFrame) {
    error!("INVALID OPCODE AT: {:#x}", stack_frame.instruction_pointer);
    error!("Stack frame: {:x?}", stack_frame);

    /*
     * TEMP: Do a stacktrace.
     */
    let mut rbp = stack_frame.rbp as usize;
    info!("Starting stacktrace. First frame is at: {:#x}", rbp);
    if rbp != 0 {
        for i in 0..16 {
            let next_rbp = unsafe { *(rbp as *const usize) };
            let return_address = unsafe { *((rbp + 8) as *const usize) };

            info!("     {}: return address: {:#x}. next frame is at: {:#x}", i, return_address, next_rbp);

            if next_rbp == 0x0 {
                break;
            } else {
                rbp = next_rbp;
            }
        }
    }

    panic!("Unrecoverable fault");
}

pub extern "C" fn general_protection_fault_handler(stack_frame: &ExceptionWithErrorStackFrame) {
    error!("General protection fault (error code = {:#x}). Interrupt stack frame: ", stack_frame.error_code);
    error!("{:#x?}", stack_frame);
    panic!("Unrecoverable fault");
}

pub extern "C" fn page_fault_handler(stack_frame: &ExceptionWithErrorStackFrame) {
    error!(
        "PAGE_FAULT: {} ({:#x})",
        match (
            stack_frame.error_code.get_bit(2), // User / Supervisor
            stack_frame.error_code.get_bit(4), // Instruction / Data
            stack_frame.error_code.get_bit(1), // Read / Write
            stack_frame.error_code.get_bit(0)  // Present
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

    error!("Error code: {}", BinaryPrettyPrint(stack_frame.error_code));
    error!("{:#x?}", stack_frame);

    /*
     * Page-faults can be recovered from and so are faults, but we never will so just give up.
     */
    /*
     * In the future, page faults can be used for demand paging and so are recoverable. At the moment, they're
     * always bad, so we panic here.
     */
    panic!("Unrecoverable fault");
}

pub extern "C" fn double_fault_handler(stack_frame: &ExceptionWithErrorStackFrame) {
    error!("EXCEPTION: DOUBLE FAULT   (Error code: {})\n{:#?}", stack_frame.error_code, stack_frame);
    panic!("Unrecoverable fault");
}
