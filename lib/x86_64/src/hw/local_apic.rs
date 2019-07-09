use crate::memory::kernel_map;
use core::ptr;

/// Represents a register in the local APIC's configuration area.
pub struct LocalApicRegister {
    ptr: *mut u32,
}

impl LocalApicRegister {
    unsafe fn new(ptr: *mut u32) -> LocalApicRegister {
        LocalApicRegister { ptr }
    }

    /// Read from this register. Unsafe because not all registers can be read from.
    pub unsafe fn read(&self) -> u32 {
        ptr::read_volatile(self.ptr)
    }

    /// Write to this register. Unsafe because not all registers can be written to.
    pub unsafe fn write(&mut self, value: u32) {
        ptr::write_volatile(self.ptr, value);
    }
}

/// Methods on this type operate on the local APIC of the **current core**. Because we need to
/// access it from interrupt handlers, this type does not borrow `self`, and the mapping for the
/// local APIC's configuration space (mapped to the virtual address
/// `kernel_map::LOCAL_APIC_CONFIG`) is managed by the `InterruptController`. None of the methods
/// on this type are safe to use until that mapping has been constructed.
pub struct LocalApic;

impl LocalApic {
    pub unsafe fn enable(spurious_vector: u8) {
        /*
         * - Enable the local APIC by setting bit 8
         * - Set the IRQ for spurious interrupts
         */
        Self::register(0xf0).write((1 << 8) | u32::from(spurious_vector));
    }

    // /// Set the local APIC timer to interrupt every `duration` ms, and then enable it. The timer
    // /// will signal on the specified vector.
    // TODO: maybe take a divisor directly here and time it in kernel::x86_64::interrupts::init
    // pub fn enable_timer(duration: usize, vector: u8) {
    //     trace!("Timing local APIC bus frequency [freezing here suggests problem with PIT
    // sleep]");     unsafe {
    //         /*
    //          * Set divide value to 16 and initial counter value to -1. We use 16 because
    //            apparently
    //          * some hardware has issues with other divide values (especially 1, which would be
    //            the
    //          * simplest otherwise). 16 seems to be the most supported.
    //          */
    //         Self::register(0x3e0).write(0x3);
    //         Self::register(0x380).write(0xffff_ffff);

    //         /*
    //          * Sleep for 10ms with the PIT and then stop the APIC timer
    //          */
    //         ::pit::PIT.do_sleep(10);
    //         Self::register(0x320).write(0x10000);

    //         let ticks_in_10ms = 0xffff_ffff - Self::register(0x390).read() as usize;
    //         trace!("Timing of local APIC bus frequency complete");

    //         /*
    //          * Start the APIC timer in Periodic mode with a divide value of 16 again, to
    //            interrupt
    //          * every 10 ms on the given vector.
    //          */
    //         Self::register(0x320).write(u32::from(vector) | 0x20000);
    //         Self::register(0x3e0).write(0x3);
    //         Self::register(0x380).write(((ticks_in_10ms / 10) * duration) as u32);
    //     }
    // }

    pub unsafe fn register(offset: usize) -> LocalApicRegister {
        LocalApicRegister::new(
            kernel_map::LOCAL_APIC_CONFIG.offset(offset as isize).mut_ptr() as *mut u32
        )
    }

    /// Send an End Of Interrupt to the local APIC. This should be called by interrupt handlers
    /// that handle external interrupts. Unsafe because the local APIC will get confused if you
    /// send it a random EOI. An EOI should not be sent when handling a spurious interrupt.
    pub unsafe fn send_eoi() {
        /*
         * To send an EOI, we write 0 to the register with offset 0xB0. Writing any other value
         * will cause a #GP.
         */
        Self::register(0xb0).write(0);
    }
}
