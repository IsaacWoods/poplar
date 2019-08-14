use super::Arch;
use crate::{per_cpu::CommonPerCpu, scheduler::Scheduler};
use alloc::boxed::Box;
use core::{marker::PhantomPinned, mem, pin::Pin};
use x86_64::{
    hw::{
        gdt::{SegmentSelector, TssSegment},
        registers::{write_msr, IA32_GS_BASE},
        tss::Tss,
    },
    memory::VirtualAddress,
};

#[derive(Debug)]
pub struct PerCpu {
    /// The first field of this structure must be a pointer to itself. This is because to access
    /// the per-cpu info, we read from `gs:0x0`, which is this pointer, and then dereference that
    /// to access the whole structure.
    _self_pointer: *const PerCpu,
    /// This structure must be pinned in memory for two reasons:
    ///     - the self pointer makes this structure self-referential.
    ///     - we put the address of this structure in the `IA32_GS_BASE` MSR. If this structure moves, that
    ///       memory address becomes invalid and accessing the per-cpu data no longer has defined behaviour.
    _pin: PhantomPinned,

    /// This holds the kernel `rsp` of the current task, and makes it efficient and easy to switch
    /// to the kernel stack upon kernel entry using `syscall`. It **must** remain at a fixed offset
    /// within this struct, because we refer to it manually with `gs:0x8`.
    current_task_kernel_rsp: VirtualAddress,

    common: CommonPerCpu<Arch>,
    tss: Tss,
    tss_selector: Option<SegmentSelector>,
}

impl PerCpu {
    pub fn tss<'a>(self: Pin<&'a Self>) -> Pin<&'a Tss> {
        unsafe { self.map_unchecked(|per_cpu| &per_cpu.tss) }
    }

    pub fn tss_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut Tss> {
        unsafe { self.map_unchecked_mut(|per_cpu| &mut per_cpu.tss) }
    }

    pub fn current_task_kernel_rsp_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut VirtualAddress> {
        unsafe { self.map_unchecked_mut(|per_cpu| &mut per_cpu.current_task_kernel_rsp) }
    }

    pub fn common<'a>(self: Pin<&'a Self>) -> Pin<&'a CommonPerCpu<Arch>> {
        unsafe { self.map_unchecked(|per_cpu| &per_cpu.common) }
    }

    pub fn common_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut CommonPerCpu<Arch>> {
        unsafe { self.map_unchecked_mut(|per_cpu| &mut per_cpu.common) }
    }

    pub fn scheduler<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut Scheduler<Arch>> {
        unsafe { self.map_unchecked_mut(|per_cpu| &mut per_cpu.common.scheduler) }
    }
}

/// Access the common per-CPU data. This is exported from the x86_64 module so it can be used from
/// the rest of the kernel.
pub unsafe fn common_per_cpu_data<'a>() -> Pin<&'a CommonPerCpu<Arch>> {
    per_cpu_data().common()
}

/// Get a mutable reference to the common per-CPU data. Exported from the x86_64 module so it can
/// be used from the rest of the kernel.
pub unsafe fn common_per_cpu_data_mut<'a>() -> Pin<&'a mut CommonPerCpu<Arch>> {
    per_cpu_data_mut().common_mut()
}

/// Access the per-CPU data. Unsafe because this must not be called before the per-CPU data has
/// been installed.
pub unsafe fn per_cpu_data<'a>() -> Pin<&'a PerCpu> {
    let ptr: *const PerCpu;
    asm!("mov $0, gs:0x0"
        : "=r"(ptr)
        :
        :
        : "intel", "volatile"
    );
    Pin::new_unchecked(&*ptr)
}

/// Get a mutable reference to the per-cpu data. This is unsafe because you must assure that only
/// one mutable reference to the `PerCpu` exists at any one time (this is easier, however, than
/// the general case, because it is only affected by the code running on one CPU - however, you
/// must still consider interrupt handlers. If interrupts can occur around your call to this
/// method, you must disable them and re-enable them after this reference has been dropped).
///
/// This is also unsafe because it must not be called before the per-CPU data has been installed.
pub unsafe fn per_cpu_data_mut<'a>() -> Pin<&'a mut PerCpu> {
    let ptr: *mut PerCpu;
    asm!("mov $0, gs:0x0"
        : "=r"(ptr)
        :
        :
        : "intel", "volatile"
    );
    Pin::new_unchecked(&mut *ptr)
}

/// This guards a `PerCpu` instance, preventing it from being dropped, or moved in memory. This
/// makes it safe to assume that accessing the per-cpu data is safe from the rest of the kernel.
pub struct GuardedPerCpu(Pin<Box<PerCpu>>);

impl GuardedPerCpu {
    /// Create a new `GuardedPerCpu`, but does not install the per-CPU data. This also creates a
    /// TSS for the current CPU and adds it to the GDT. The per-CPU data can only be installed (its
    /// address put in `IA32_GS_BASE`) after the GDT has been loaded, because we zero `GS` during
    /// GDT initialization.
    pub fn new() -> (GuardedPerCpu, SegmentSelector) {
        let tss = Tss::new();
        let mut per_cpu = Box::pin(PerCpu {
            // We haven't allocated space for the structure yet, so we don't know where it'll be.
            // We fill this in after it's been allocated.
            _self_pointer: 0x0 as *const PerCpu,
            _pin: PhantomPinned,

            current_task_kernel_rsp: VirtualAddress::new(0x0).unwrap(),

            common: CommonPerCpu::new(),
            tss,
            tss_selector: None,
        });

        /*
         * Install the TSS into the GDT. This gives us a selector, so we can populate the
         * `tss_selector` field. The use of `get_unchecked_mut` is safe here because changing the
         * selector cannot move the memory.
         */
        let tss_selector = super::GDT.lock().add_tss(TssSegment::new(per_cpu.as_ref().tss()));
        unsafe {
            Pin::get_unchecked_mut(Pin::as_mut(&mut per_cpu)).tss_selector = Some(tss_selector);
        }

        /*
         * XXX: This relies on `Pin` being transparent (having the same memory layout as the
         * underlying pointer).
         */
        let address: *const PerCpu = unsafe { mem::transmute(per_cpu.as_ref()) };

        // Fill out the self-pointer. This is safe because changing a field doesn't move the structure.
        unsafe {
            Pin::get_unchecked_mut(Pin::as_mut(&mut per_cpu))._self_pointer = address;
        }

        (GuardedPerCpu(per_cpu), tss_selector)
    }

    pub fn install(&self) {
        /*
         * Put the address of the per-cpu data into the GS_BASE MSR. The structure can then be accessed
         * at `gs:0x0`.
         */
        unsafe { write_msr(IA32_GS_BASE, self.0.as_ref()._self_pointer as usize as u64) };
    }
}

impl Drop for GuardedPerCpu {
    fn drop(&mut self) {
        panic!("Per-cpu data should never be dropped!");
    }
}
