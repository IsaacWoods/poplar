/*
 * Copyright (C) 2018, Isaac Woods.
 * See LICENCE.md
 */

use interrupts::InterruptStackFrame;
use apic::LOCAL_APIC;

static mut PIT_TICKS : usize = 0;

/// Handler for interrupts from the Programmable Interrupt Controller.
/// **Should not be called manually!**
pub extern "C" fn pit_handler(_ : &InterruptStackFrame)
{
    /*
     * XXX: Printing here seems to lock everything up (probably due to the contention on the mutex
     * involved) so probably avoid that.
     */
    unsafe
    {
        PIT_TICKS += 1;
    }

    LOCAL_APIC.lock().send_eoi();
}
