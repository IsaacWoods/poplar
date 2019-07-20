use alloc::boxed::Box;
use core::{marker::PhantomPinned, mem, pin::Pin};
use x86_64::hw::{
    gdt::{SegmentSelector, TssSegment},
    registers::{write_msr, IA32_GS_BASE},
    tss::Tss,
};

#[derive(Debug)]
pub struct PerCpu {
    /// The first field of this structure must be a pointer to itself. This is because to access
    /// the per-cpu info, we read from the segment at offset `0x0`, which is this pointer, and then
    /// dereference that to access the whole structure.
    _self_pointer: *const PerCpu,
    tss: Tss,
    tss_selector: Option<SegmentSelector>,
    /// `PerCpu` must not move in memory because it's accessed using the GS segment base.
    _pin: PhantomPinned,
}

impl PerCpu {
    pub fn get_tss<'a>(self: Pin<&'a Self>) -> Pin<&'a Tss> {
        unsafe { self.map_unchecked(|per_cpu| &per_cpu.tss) }
    }

    pub fn get_tss_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut Tss> {
        unsafe { self.map_unchecked_mut(|per_cpu| &mut per_cpu.tss) }
    }
}

/// Access the per-CPU data. Unsafe because this must not be called before the per-CPU data has
/// been installed.
pub unsafe fn per_cpu_data() -> Pin<&'static PerCpu> {
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
pub unsafe fn per_cpu_data_mut() -> Pin<&'static mut PerCpu> {
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
            tss,
            tss_selector: None,
            _pin: PhantomPinned,
        });

        /*
         * Install the TSS into the GDT. This gives us a selector, so we can populate the
         * `tss_selector` field. The use of `get_unchecked_mut` is safe here because changing the
         * selector cannot move the memory.
         */
        let tss_selector = super::GDT.lock().add_tss(TssSegment::new(per_cpu.as_ref().get_tss()));
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
