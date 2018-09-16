use core::ptr;
use interrupts::InterruptStackFrame;
use memory::map::LOCAL_APIC_CONFIG_START;

pub extern "C" fn apic_timer_handler(_: &InterruptStackFrame) {
    unsafe {
        LocalApic::send_eoi();
    }
}

pub struct LocalApicRegister {
    ptr: *mut u32,
}

impl LocalApicRegister {
    unsafe fn new(ptr: *mut u32) -> LocalApicRegister {
        LocalApicRegister { ptr }
    }

    pub unsafe fn read(&self) -> u32 {
        ptr::read_volatile(self.ptr)
    }

    pub unsafe fn write(&mut self, value: u32) {
        ptr::write_volatile(self.ptr, value);
    }
}

/// Methods on this type operate on the local APIC of the **current core**. Because we need to
/// access it from interrupt handlers, this type should not be created, and the mapping for the local
/// APIC's configuration space is managed by the `InterruptController`. It is not safe to enable
/// interrupts until that mapping exists and the local APIC has been enabled.
pub struct LocalApic {}

impl LocalApic {
    pub unsafe fn enable() {
        /*
         * - Enable the local APIC by setting bit 8
         * - Set the IRQ for spurious interrupts
         */
        let spurious_interrupt_vector = (1 << 8) | u32::from(::interrupts::APIC_SPURIOUS_INTERRUPT);
        Self::register(0xf0).write(spurious_interrupt_vector);
    }

    /// Set the local APIC timer to interrupt every `duration` ms, and then enable it
    pub fn enable_timer(duration: usize) {
        trace!("Timing local APIC bus frequency [freezing here suggests problem with PIT sleep]");
        unsafe {
            /*
             * Set divide value to 16 and initial counter value to -1. We use 16 because apparently
             * some hardware has issues with other divide values (especially 1, which would be the
             * simplest otherwise). 16 seems to be the most supported.
             */
            Self::register(0x3e0).write(0x3);
            Self::register(0x380).write(0xffff_ffff);

            /*
             * Sleep for 10ms with the PIT and then stop the APIC timer
             */
            ::pit::PIT.do_sleep(10);
            Self::register(0x320).write(0x10000);

            let ticks_in_10ms = 0xffff_ffff - Self::register(0x390).read() as usize;
            trace!("Timing of local APIC bus frequency complete");

            /*
             * Start the APIC timer in Periodic mode with a divide value of 16 again, to interrupt
             * every 10 ms.
             */
            Self::register(0x320).write(u32::from(::interrupts::LOCAL_APIC_TIMER) | 0x20000);
            Self::register(0x3e0).write(0x3);
            Self::register(0x380).write(((ticks_in_10ms / 10) * duration) as u32);
        }
    }

    pub unsafe fn register(offset: usize) -> LocalApicRegister {
        LocalApicRegister::new(LOCAL_APIC_CONFIG_START.offset(offset as isize).mut_ptr() as *mut u32)
    }

    /// Send an End Of Interrupt to the local APIC. This should be called by interrupt handlers
    /// that handle external interrupts. Unsafe because the local APIC will get confused if you
    /// send it a random EOI.
    pub unsafe fn send_eoi() {
        /*
         * To send an EOI, we write 0 to the register with offset 0xB0. Writing any other value
         * will cause a #GP.
         */
        Self::register(0xb0).write(0);
    }
}
