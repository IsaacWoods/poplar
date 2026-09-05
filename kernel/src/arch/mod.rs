//! The `arch` module performs architecture-specific initialization and interfacing with the
//! hardware. Much of this abstraction can be done in the HAL (Hardware Abstraction Layer),
//! implemented in the `hal` crate, but some relies upon definitions in other parts of the `kernel`
//! crate.
//!
//! To present a common interface to the rest of the kernel, this module makes assumptions about
//! architectures Poplar will run on. The system boots one CPU, called the BSP (Boot-Strap
//! Processor), which performs initialization that should occur globally. Other processors, called
//! APs (Application Processors) are either booted from the BSP, or are booted by the hardware
//! alongside the BSP, and only perform initialization needed on each CPU.

mod exception;
pub mod interrupt;

use alloc::boxed::Box;
use bnb::BitOps;
use core::{arch::global_asm, marker::PhantomPinned, mem, mem::MaybeUninit};
use hal::{
    cpu::{
        interrupt::Exception,
        tables::{DescriptorTablePtr, GdtDescriptor, Tss},
    },
    mem::VAddr,
    wrap_handler,
    wrap_handler_with_error_code,
};
use interrupt::IDT;

global_asm!(include_str!("helpers.asm"));

unsafe extern "C" {
    unsafe fn load_gdt(ptr: *const DescriptorTablePtr);
}

#[repr(C)]
struct PerCpu {
    /// A self-referential pointer to the `PerCpu` structure. This allows a pointer to this
    /// structure for the current CPU to be loaded with `lea r#x, gs:[0x0]`.
    _self_ptr: *mut PerCpu,

    tss: Tss,
    gdt: Gdt,

    _pinned: PhantomPinned,
}

impl PerCpu {
    fn new() -> *mut PerCpu {
        let mut per_cpu: Box<MaybeUninit<PerCpu>> = Box::new(MaybeUninit::uninit());
        let per_cpu_ptr = Box::as_ptr(&per_cpu) as *mut PerCpu;

        per_cpu.write(PerCpu {
            _self_ptr: per_cpu_ptr,
            tss: Tss::new(),
            gdt: Gdt::new(
                unsafe { per_cpu_ptr.byte_add(mem::offset_of!(PerCpu, tss)) } as *const Tss,
            ),
            _pinned: PhantomPinned,
        });

        unsafe { Box::leak(per_cpu).assume_init_mut() as *mut PerCpu }
        // TODO: install into the MSR to access via GS
    }

    fn gdt(ptr: *const PerCpu) -> *const Gdt {
        unsafe { ptr.byte_add(mem::offset_of!(PerCpu, gdt)) as *const Gdt }
    }
}

#[repr(C)]
struct Gdt {
    null: u64,
    kernel_code: GdtDescriptor,
    kernel_data: GdtDescriptor,
    user_code_compat: GdtDescriptor,
    user_data: GdtDescriptor,
    user_code: GdtDescriptor,
    tss_lo: GdtDescriptor,
    tss_hi: u64,
}

impl Gdt {
    pub fn new(tss: *const Tss) -> Gdt {
        Gdt {
            null: 0,
            kernel_code: GdtDescriptor::KERNEL_CODE64,
            kernel_data: GdtDescriptor::KERNEL_DATA64,
            user_code_compat: GdtDescriptor::USER_CODE32,
            user_data: GdtDescriptor::USER_DATA64,
            user_code: GdtDescriptor::USER_CODE64,
            tss_lo: GdtDescriptor::new_tss_lo(VAddr::from(tss)),
            tss_hi: (tss as usize).bits(32..64) as u64,
        }
    }
}

pub fn initialize_bsp() {
    let per_cpu = PerCpu::new();

    let gdt_ptr = DescriptorTablePtr {
        limit: (mem::size_of::<Gdt>() - 1) as u16,
        base: PerCpu::gdt(per_cpu) as usize as u64,
    };
    unsafe { load_gdt(&gdt_ptr as *const DescriptorTablePtr) };

    {
        use exception::*;
        IDT.install_exception_handler(Exception::DivideError, wrap_handler!(divide_error));
        IDT.install_exception_handler(Exception::Debug, wrap_handler!(debug_exception));
        IDT.install_exception_handler(Exception::Nmi, wrap_handler!(nmi));
        IDT.install_exception_handler(Exception::Breakpoint, wrap_handler!(breakpoint));
        IDT.install_exception_handler(Exception::InvalidOpcode, wrap_handler!(invalid_opcode));
        IDT.install_exception_handler(
            Exception::DeviceNotAvailable,
            wrap_handler!(device_not_available),
        );
        IDT.install_exception_handler(
            Exception::DoubleFault,
            wrap_handler_with_error_code!(double_fault),
        );
        IDT.install_exception_handler(
            Exception::InvalidTss,
            wrap_handler_with_error_code!(invalid_tss),
        );
        IDT.install_exception_handler(
            Exception::SegmentNotPresent,
            wrap_handler_with_error_code!(segment_not_present),
        );
        IDT.install_exception_handler(
            Exception::StackSegmentFault,
            wrap_handler_with_error_code!(stack_segment_fault),
        );
        IDT.install_exception_handler(
            Exception::GeneralProtectionFault,
            wrap_handler_with_error_code!(general_protection_fault),
        );
        IDT.install_exception_handler(
            Exception::PageFault,
            wrap_handler_with_error_code!(page_fault),
        );
        IDT.install_exception_handler(Exception::MathFault, wrap_handler!(math_fault));
    }
    IDT.install();
    todo!()
}
