#![no_std]
#![feature(asm, decl_macro, const_fn, global_asm, naked_functions, type_ascription)]

pub mod hw;
pub mod kernel_map;
pub mod logger;
pub mod paging;
pub mod task;

/*
 * On `x86_64`, we also use the HAL crate from `efiloader`, which doesn't have any heap allocation. Because of
 * this, for Rust to be happy, we have to encase everything that uses heap allocation in a feature that is only
 * used by the kernel.
 */
cfg_if::cfg_if! {
    if #[cfg(feature = "kernel")] {
        extern crate alloc;

        mod acpi_handler;
        mod interrupts;
        mod per_cpu;

        use core::marker::PhantomData;
        use acpi::Acpi;
        use acpi_handler::PebbleAcpiHandler;
        use aml::AmlContext;
        use core::time::Duration;
        use hal::{
            boot_info::BootInfo,
            memory::{Frame, PhysicalAddress, Size4KiB},
            Hal,
        };
        use hw::cpu::CpuInfo;
        use interrupts::InterruptController;
        use log::{error, info, warn};
        use paging::PageTableImpl;
        use core::pin::Pin;
        use per_cpu::PerCpuImpl;
        use alloc::boxed::Box;

        pub struct HalImpl<T> {
            cpu_info: CpuInfo,
            acpi_info: Option<Acpi>,
            aml_context: AmlContext,
            kernel_page_table: PageTableImpl,
            bsp_percpu: Pin<Box<PerCpuImpl<T>>>,
            interrupt_controller: InterruptController,
            _phantom: PhantomData<T>,
        }

        impl<T> Hal<T> for HalImpl<T> {
            type PageTableSize = Size4KiB;
            type PageTable = paging::PageTableImpl;
            type TaskHelper = task::TaskHelperImpl;
            type PerCpu = per_cpu::PerCpuImpl<T>;

            fn init_logger() {
                log::set_logger(&logger::KernelLogger).unwrap();
                log::set_max_level(log::LevelFilter::Trace);
            }

            fn init(boot_info: &BootInfo, per_cpu_data: T) -> Self {
                let cpu_info = CpuInfo::new();
                info!(
                    "We're running on an {:?} processor, model info = {:?}, microarch = {:?}",
                    cpu_info.vendor,
                    cpu_info.model_info,
                    cpu_info.microarch()
                );
                if let Some(ref hypervisor_info) = cpu_info.hypervisor_info {
                    info!("We're running under a hypervisor ({:?})", hypervisor_info.vendor);
                }
                check_support_and_enable_features(&cpu_info);

                /*
                 * Create our version of the kernel page table. This assumes that the loader has correctly installed a
                 * set of page tables, including a full physical mapping at the correct location. Strange things
                 * will happen if this is not the case, so this is a tad unsafe.
                 */
                let kernel_page_table = unsafe {
                    PageTableImpl::from_frame(
                        Frame::starts_with(PhysicalAddress::new(hw::registers::read_control_reg!(cr3) as usize).unwrap()),
                        kernel_map::PHYSICAL_MAPPING_BASE,
                    )
                };

                /*
                 * Parse the static ACPI tables.
                 */
                let acpi_info = match boot_info.rsdp_address {
                    Some(rsdp_address) => {
                        let mut handler = PebbleAcpiHandler;
                        match unsafe { acpi::parse_rsdp(&mut handler, usize::from(rsdp_address)) } {
                            Ok(acpi_info) => Some(acpi_info),

                            Err(err) => {
                                error!("Failed to parse static ACPI tables: {:?}", err);
                                warn!("Continuing. Some functionality may not work, or the kernel may panic!");
                                None
                            }
                        }
                    }

                    None => None,
                };
                info!("{:#?}", acpi_info);

                /*
                 * Parse the DSDT.
                 */
                let mut aml_context = AmlContext::new();
                if let Some(ref dsdt_info) = acpi_info.as_ref().unwrap().dsdt {
                    let virtual_address =
                        kernel_map::physical_to_virtual(PhysicalAddress::new(dsdt_info.address).unwrap());
                    info!(
                        "DSDT parse: {:?}",
                        aml_context.parse_table(unsafe {
                            core::slice::from_raw_parts(virtual_address.ptr(), dsdt_info.length as usize)
                        })
                    );

                    // TODO: we should parse the SSDTs here. Only bother if we've managed to parse the DSDT.

                    // info!("----- Printing AML namespace -----");
                    // info!("{:#?}", aml_context.namespace);
                    // info!("----- Finished AML namespace -----");
                }

                /*
                 * Create the per-CPU data (which adds a TSS to the GDT) for the BSP, loads the GDT, then installs
                 * the BSP's per-CPU data.
                 * NOTE: Installing the GDT zeros the `gs` segment descriptor so it's important we do it before
                 * installing the per-CPU data.
                 * TODO: create a TSS for each AP here so we can load the GDT all in one go.
                 */
                let (mut bsp_percpu, bsp_tss_selector) = per_cpu::PerCpuImpl::<T>::new(per_cpu_data);
                unsafe {
                    hw::gdt::GDT.lock().load(bsp_tss_selector);
                }
                bsp_percpu.as_mut().install();

                /*
                 * Initialise the interrupt controller, which enables interrupts, and start the per-cpu timer.
                 */
                let mut interrupt_controller = InterruptController::init(acpi_info.as_ref().unwrap(), &mut aml_context);
                interrupt_controller.enable_local_timer(&cpu_info, Duration::from_secs(3));

                HalImpl { cpu_info, acpi_info, aml_context, kernel_page_table, bsp_percpu, interrupt_controller, _phantom: PhantomData }
            }

            unsafe fn disable_interrupts() {
                asm!("cli");
            }

            unsafe fn enable_interrupts() {
                asm!("sti");
            }

            fn cpu_halt() -> ! {
                loop {
                    unsafe {
                        asm!("hlt");
                    }
                }
            }

            fn kernel_page_table(&mut self) -> &mut Self::PageTable {
                &mut self.kernel_page_table
            }

            unsafe fn per_cpu<'a>() -> Pin<&'a mut Self::PerCpu> {
                per_cpu::get_per_cpu_data()
            }
        }

        /// We rely on certain processor features to be present for simplicity and sanity-retention. This
        /// function checks that we support everything we need to, and enable features that we need.
        fn check_support_and_enable_features(cpu_info: &CpuInfo) {
            use bit_field::BitField;
            use hw::registers::{
                read_control_reg,
                read_msr,
                write_control_reg,
                write_msr,
                CR4_ENABLE_GLOBAL_PAGES,
                CR4_RESTRICT_RDTSC,
                CR4_XSAVE_ENABLE_BIT,
                EFER,
                EFER_ENABLE_NX_BIT,
                EFER_ENABLE_SYSCALL,
            };

            if !cpu_info.supported_features.xsave {
                panic!("Processor does not support xsave instruction!");
            }

            let mut cr4 = read_control_reg!(CR4);
            cr4.set_bit(CR4_XSAVE_ENABLE_BIT, true);
            cr4.set_bit(CR4_ENABLE_GLOBAL_PAGES, true);
            cr4.set_bit(CR4_RESTRICT_RDTSC, true);
            unsafe {
                write_control_reg!(CR4, cr4);
            }

            let mut efer = read_msr(EFER);
            efer.set_bit(EFER_ENABLE_SYSCALL, true);
            efer.set_bit(EFER_ENABLE_NX_BIT, true);
            unsafe {
                write_msr(EFER, efer);
            }
        }
    }
}
