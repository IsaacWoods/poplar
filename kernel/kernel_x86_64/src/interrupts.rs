use crate::kacpi::AcpiManager;
use acpi::InterruptModel;
use alloc::{alloc::Global, sync::Arc, vec};
use aml::namespace::AmlName;
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

pub static INTERRUPT_CONTROLLER: InitGuard<Spinlock<InterruptController>> = InitGuard::uninit();

/*
 * Constants for allocated portions of the IDT. These should match the layout above.
 */
const ISA_INTERRUPTS_START: u8 = 0x20;
const NUM_ISA_INTERRUPTS: usize = 16;
const FREE_VECTORS_START: u8 = 0x30;
const NUM_PLATFORM_VECTORS: usize = 64;
const APIC_TIMER_VECTOR: u8 = 0xfe;
const APIC_SPURIOUS_VECTOR: u8 = 0xff;

type PlatformHandler = fn(&InterruptStackFrame, u8);

pub struct InterruptController {
    model: InterruptModel<Global>,
    isa_gsi_mappings: [u32; NUM_ISA_INTERRUPTS],
    platform_handlers: [Option<PlatformHandler>; NUM_PLATFORM_VECTORS],
    io_apic: IoApic,
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

    pub fn init(acpi: &AcpiManager) {
        match &acpi.platform_info.interrupt_model {
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
                acpi.interpreter
                    .invoke_method(
                        AmlName::from_str("\\_PIC").unwrap(),
                        vec![Arc::new(aml::object::Object::Integer(1))],
                    )
                    .expect("Failed to invoke \\_PIC");

                /*
                 * Install handlers for the spurious interrupt and local APIC timer, and then
                 * enable the local APIC.
                 */
                unsafe {
                    let mut idt = IDT.lock();
                    idt[APIC_TIMER_VECTOR].set_handler(wrap_handler!(local_apic_timer_handler));
                    idt[APIC_SPURIOUS_VECTOR].set_handler(wrap_handler!(spurious_handler));
                    LOCAL_APIC.get().enable(APIC_SPURIOUS_VECTOR);

                    idt[FREE_VECTORS_START + 0].set_handler(platform_handler_30);
                    idt[FREE_VECTORS_START + 1].set_handler(platform_handler_31);
                    idt[FREE_VECTORS_START + 2].set_handler(platform_handler_32);
                    idt[FREE_VECTORS_START + 3].set_handler(platform_handler_33);
                    idt[FREE_VECTORS_START + 4].set_handler(platform_handler_34);
                    idt[FREE_VECTORS_START + 5].set_handler(platform_handler_35);
                    idt[FREE_VECTORS_START + 6].set_handler(platform_handler_36);
                    idt[FREE_VECTORS_START + 7].set_handler(platform_handler_37);
                    idt[FREE_VECTORS_START + 8].set_handler(platform_handler_38);
                    idt[FREE_VECTORS_START + 9].set_handler(platform_handler_39);
                    idt[FREE_VECTORS_START + 10].set_handler(platform_handler_3a);
                    idt[FREE_VECTORS_START + 11].set_handler(platform_handler_3b);
                    idt[FREE_VECTORS_START + 12].set_handler(platform_handler_3c);
                    idt[FREE_VECTORS_START + 13].set_handler(platform_handler_3d);
                    idt[FREE_VECTORS_START + 14].set_handler(platform_handler_3e);
                    idt[FREE_VECTORS_START + 15].set_handler(platform_handler_3f);
                    idt[FREE_VECTORS_START + 16].set_handler(platform_handler_40);
                    idt[FREE_VECTORS_START + 17].set_handler(platform_handler_41);
                    idt[FREE_VECTORS_START + 18].set_handler(platform_handler_42);
                    idt[FREE_VECTORS_START + 19].set_handler(platform_handler_43);
                    idt[FREE_VECTORS_START + 20].set_handler(platform_handler_44);
                    idt[FREE_VECTORS_START + 21].set_handler(platform_handler_45);
                    idt[FREE_VECTORS_START + 22].set_handler(platform_handler_46);
                    idt[FREE_VECTORS_START + 23].set_handler(platform_handler_47);
                    idt[FREE_VECTORS_START + 24].set_handler(platform_handler_48);
                    idt[FREE_VECTORS_START + 25].set_handler(platform_handler_49);
                    idt[FREE_VECTORS_START + 26].set_handler(platform_handler_4a);
                    idt[FREE_VECTORS_START + 27].set_handler(platform_handler_4b);
                    idt[FREE_VECTORS_START + 28].set_handler(platform_handler_4c);
                    idt[FREE_VECTORS_START + 29].set_handler(platform_handler_4d);
                    idt[FREE_VECTORS_START + 30].set_handler(platform_handler_4e);
                    idt[FREE_VECTORS_START + 31].set_handler(platform_handler_4f);
                    idt[FREE_VECTORS_START + 32].set_handler(platform_handler_50);
                    idt[FREE_VECTORS_START + 33].set_handler(platform_handler_51);
                    idt[FREE_VECTORS_START + 34].set_handler(platform_handler_52);
                    idt[FREE_VECTORS_START + 35].set_handler(platform_handler_53);
                    idt[FREE_VECTORS_START + 36].set_handler(platform_handler_54);
                    idt[FREE_VECTORS_START + 37].set_handler(platform_handler_55);
                    idt[FREE_VECTORS_START + 38].set_handler(platform_handler_56);
                    idt[FREE_VECTORS_START + 39].set_handler(platform_handler_57);
                    idt[FREE_VECTORS_START + 40].set_handler(platform_handler_58);
                    idt[FREE_VECTORS_START + 41].set_handler(platform_handler_59);
                    idt[FREE_VECTORS_START + 42].set_handler(platform_handler_5a);
                    idt[FREE_VECTORS_START + 43].set_handler(platform_handler_5b);
                    idt[FREE_VECTORS_START + 44].set_handler(platform_handler_5c);
                    idt[FREE_VECTORS_START + 45].set_handler(platform_handler_5d);
                    idt[FREE_VECTORS_START + 46].set_handler(platform_handler_5e);
                    idt[FREE_VECTORS_START + 47].set_handler(platform_handler_5f);
                    idt[FREE_VECTORS_START + 48].set_handler(platform_handler_60);
                    idt[FREE_VECTORS_START + 49].set_handler(platform_handler_61);
                    idt[FREE_VECTORS_START + 50].set_handler(platform_handler_62);
                    idt[FREE_VECTORS_START + 51].set_handler(platform_handler_63);
                    idt[FREE_VECTORS_START + 52].set_handler(platform_handler_64);
                    idt[FREE_VECTORS_START + 53].set_handler(platform_handler_65);
                    idt[FREE_VECTORS_START + 54].set_handler(platform_handler_66);
                    idt[FREE_VECTORS_START + 55].set_handler(platform_handler_67);
                    idt[FREE_VECTORS_START + 56].set_handler(platform_handler_68);
                    idt[FREE_VECTORS_START + 57].set_handler(platform_handler_69);
                    idt[FREE_VECTORS_START + 58].set_handler(platform_handler_6a);
                    idt[FREE_VECTORS_START + 59].set_handler(platform_handler_6b);
                    idt[FREE_VECTORS_START + 60].set_handler(platform_handler_6c);
                    idt[FREE_VECTORS_START + 61].set_handler(platform_handler_6d);
                    idt[FREE_VECTORS_START + 62].set_handler(platform_handler_6e);
                    idt[FREE_VECTORS_START + 63].set_handler(platform_handler_6f);
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

                INTERRUPT_CONTROLLER.initialize(Spinlock::new(InterruptController {
                    model: acpi.platform_info.interrupt_model.clone(),
                    isa_gsi_mappings,
                    platform_handlers: [None; NUM_PLATFORM_VECTORS],
                    io_apic,
                }));
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

    pub fn allocate_platform_interrupt(&mut self, handler: PlatformHandler) -> u8 {
        for i in 0..NUM_PLATFORM_VECTORS {
            if self.platform_handlers[i].is_none() {
                self.platform_handlers[i] = Some(handler);
                return FREE_VECTORS_START + i as u8;
            }
        }

        panic!("Run out of free platform interrupt vectors!");
    }

    pub fn configure_gsi(
        &mut self,
        gsi: u32,
        pin_polarity: PinPolarity,
        trigger_mode: TriggerMode,
        handler: PlatformHandler,
    ) -> Result<u8, ()> {
        let platform_interrupt = self.allocate_platform_interrupt(handler);
        self.io_apic.write_entry(
            gsi,
            platform_interrupt,
            DeliveryMode::Fixed,
            pin_polarity,
            trigger_mode,
            false,
            0,
        );
        Ok(platform_interrupt)
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

#[no_mangle]
pub extern "C" fn handle_platform_interrupt(stack_frame: &InterruptStackFrame, number: u8) {
    assert!((FREE_VECTORS_START..(FREE_VECTORS_START + NUM_PLATFORM_VECTORS as u8)).contains(&number));
    if let Some(handler) =
        INTERRUPT_CONTROLLER.get().try_lock().unwrap().platform_handlers[(number - FREE_VECTORS_START) as usize]
    {
        (handler)(stack_frame, number);
    } else {
        panic!("Got platform interrupt but it has no handler! Vector = {}", number);
    }

    unsafe {
        LOCAL_APIC.get().send_eoi();
    }
}

macro_rules! platform_handler_stub {
    ($name:ident, $number:literal) => {
        #[naked]
        extern "C" fn $name() -> ! {
            unsafe {
                core::arch::naked_asm!("
                    // TODO: this could interrupt from usermode. We should test and `swapgs` if
                    // needed
                    /*
                     * Save registers to match the layout of `InterruptStackFrame`.
                     */
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

                    /*
                     * Load the stack frame and interrupt number into the argument registers, and
                     * then align the stack (as we've pushed `0xa0` bytes).
                     */
                    mov rdi, rsp
                    mov rsi, {}
                    sub rsp, 8
                    call handle_platform_interrupt
                    add rsp, 8

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

                    // TODO: restore gs if needed

                    iretq
                ", const $number)
            }
        }
    }
}

platform_handler_stub!(platform_handler_30, 0x30);
platform_handler_stub!(platform_handler_31, 0x31);
platform_handler_stub!(platform_handler_32, 0x32);
platform_handler_stub!(platform_handler_33, 0x33);
platform_handler_stub!(platform_handler_34, 0x34);
platform_handler_stub!(platform_handler_35, 0x35);
platform_handler_stub!(platform_handler_36, 0x36);
platform_handler_stub!(platform_handler_37, 0x37);
platform_handler_stub!(platform_handler_38, 0x38);
platform_handler_stub!(platform_handler_39, 0x39);
platform_handler_stub!(platform_handler_3a, 0x3a);
platform_handler_stub!(platform_handler_3b, 0x3b);
platform_handler_stub!(platform_handler_3c, 0x3c);
platform_handler_stub!(platform_handler_3d, 0x3d);
platform_handler_stub!(platform_handler_3e, 0x3e);
platform_handler_stub!(platform_handler_3f, 0x3f);
platform_handler_stub!(platform_handler_40, 0x40);
platform_handler_stub!(platform_handler_41, 0x41);
platform_handler_stub!(platform_handler_42, 0x42);
platform_handler_stub!(platform_handler_43, 0x43);
platform_handler_stub!(platform_handler_44, 0x44);
platform_handler_stub!(platform_handler_45, 0x45);
platform_handler_stub!(platform_handler_46, 0x46);
platform_handler_stub!(platform_handler_47, 0x47);
platform_handler_stub!(platform_handler_48, 0x48);
platform_handler_stub!(platform_handler_49, 0x49);
platform_handler_stub!(platform_handler_4a, 0x4a);
platform_handler_stub!(platform_handler_4b, 0x4b);
platform_handler_stub!(platform_handler_4c, 0x4c);
platform_handler_stub!(platform_handler_4d, 0x4d);
platform_handler_stub!(platform_handler_4e, 0x4e);
platform_handler_stub!(platform_handler_4f, 0x4f);
platform_handler_stub!(platform_handler_50, 0x50);
platform_handler_stub!(platform_handler_51, 0x51);
platform_handler_stub!(platform_handler_52, 0x52);
platform_handler_stub!(platform_handler_53, 0x53);
platform_handler_stub!(platform_handler_54, 0x54);
platform_handler_stub!(platform_handler_55, 0x55);
platform_handler_stub!(platform_handler_56, 0x56);
platform_handler_stub!(platform_handler_57, 0x57);
platform_handler_stub!(platform_handler_58, 0x58);
platform_handler_stub!(platform_handler_59, 0x59);
platform_handler_stub!(platform_handler_5a, 0x5a);
platform_handler_stub!(platform_handler_5b, 0x5b);
platform_handler_stub!(platform_handler_5c, 0x5c);
platform_handler_stub!(platform_handler_5d, 0x5d);
platform_handler_stub!(platform_handler_5e, 0x5e);
platform_handler_stub!(platform_handler_5f, 0x5f);
platform_handler_stub!(platform_handler_60, 0x60);
platform_handler_stub!(platform_handler_61, 0x61);
platform_handler_stub!(platform_handler_62, 0x62);
platform_handler_stub!(platform_handler_63, 0x63);
platform_handler_stub!(platform_handler_64, 0x64);
platform_handler_stub!(platform_handler_65, 0x65);
platform_handler_stub!(platform_handler_66, 0x66);
platform_handler_stub!(platform_handler_67, 0x67);
platform_handler_stub!(platform_handler_68, 0x68);
platform_handler_stub!(platform_handler_69, 0x69);
platform_handler_stub!(platform_handler_6a, 0x6a);
platform_handler_stub!(platform_handler_6b, 0x6b);
platform_handler_stub!(platform_handler_6c, 0x6c);
platform_handler_stub!(platform_handler_6d, 0x6d);
platform_handler_stub!(platform_handler_6e, 0x6e);
platform_handler_stub!(platform_handler_6f, 0x6f);
