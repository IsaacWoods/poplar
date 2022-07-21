use core::ptr;
use hal::memory::VirtualAddress;

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
        unsafe { ptr::read_volatile(self.ptr) }
    }

    /// Write to this register. Unsafe because not all registers can be written to.
    pub unsafe fn write(&mut self, value: u32) {
        unsafe {
            ptr::write_volatile(self.ptr, value);
        }
    }
}

pub struct LocalApic(VirtualAddress);

impl LocalApic {
    pub unsafe fn new(address: VirtualAddress) -> LocalApic {
        LocalApic(address)
    }

    pub unsafe fn enable(&self, spurious_vector: u8) {
        /*
         * - Enable the local APIC by setting bit 8
         * - Set the IRQ for spurious interrupts
         */
        unsafe {
            self.register(0xf0).write((1 << 8) | u32::from(spurious_vector));
        }
    }

    /// Set the local APIC timer to interrupt every `duration` ms, and then enable it. The timer
    /// will signal on the specified vector. The frequency of the local APIC must be passed (in Hz), and
    /// can sometimes be retrieved from the `CpuInfo`.
    pub fn enable_timer(&self, duration: u32, apic_frequency: u32, vector: u8) {
        /*
         * Calculate the number of ticks in one millisecond. We also divide by 16 because we will
         * set the divider to 16, so the given frequency will be 16 times faster than the count
         * will be decremented by.
         */
        let ticks_in_1ms = apic_frequency / 1000 / 16;
        let ticks = duration * ticks_in_1ms;

        /*
         * Start the APIC timer in Periodic mode with a divide value of 16, to interrupt every
         * `duration` ms.
         *
         * We pick 16 as the divider here because some hardware apparently has issues with using a
         * divider of 1, which would be the simplest.
         */
        unsafe {
            let timer_entry = {
                use bit_field::BitField;

                let mut entry = u32::from(vector);
                entry.set_bits(17..19, 0b01); // Periodic mode
                entry
            };
            self.register(0x3e0).write(0b0011); // Step 1: Set the divider to 16
            self.register(0x320).write(timer_entry); // Step 2: enable the timer
            self.register(0x380).write(ticks); // Step 3: Set the initial count
        }

        /*
         * TODO: we used to calibrate the timer by timing with the PIC. This could still be a good
         * backup if the cpuid doesn't have the info we need:
         */
        // unsafe {
        //     /*
        //     * Set divide value to 16 and initial counter value to -1. We use 16 because apparently
        //     * some hardware has issues with other divide values (especially 1, which would be the
        //     * simplest otherwise). 16 seems to be the most supported.
        //     */
        //     Self::register(0x3e0).write(0x3);
        //     Self::register(0x380).write(0xffff_ffff);

        //     /*
        //      * Sleep for 10ms with the PIT and then stop the APIC timer
        //      */
        //     ::pit::PIT.do_sleep(10);
        //     Self::register(0x320).write(0x10000);

        //     let ticks_in_10ms = 0xffff_ffff - Self::register(0x390).read() as usize;
        //     trace!("Timing of local APIC bus frequency complete");

        //     /*
        //     * Start the APIC timer in Periodic mode with a divide value of 16 again, to interrupt
        //     * every 10 ms on the given vector.
        //     */
        //     Self::register(0x320).write(u32::from(vector) | 0x20000);
        //     Self::register(0x3e0).write(0x3);
        //     Self::register(0x380).write(((ticks_in_10ms / 10) * duration) as u32);
        // }
    }

    pub unsafe fn register(&self, offset: usize) -> LocalApicRegister {
        unsafe { LocalApicRegister::new((self.0 + offset).mut_ptr() as *mut u32) }
    }

    /// Send an End Of Interrupt to the local APIC. This should be called by interrupt handlers
    /// that handle external interrupts. Unsafe because the local APIC will get confused if you
    /// send it a random EOI. An EOI should not be sent when handling a spurious interrupt.
    pub unsafe fn send_eoi(&self) {
        /*
         * To send an EOI, we write 0 to the register with offset 0xb0. Writing any other value
         * will cause a #GP.
         */
        unsafe {
            self.register(0xb0).write(0);
        }
    }
}
