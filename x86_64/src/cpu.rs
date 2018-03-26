/*
 * Copyright (C) 2018, Pebble Developers.
 * See LICENCE.md
 */

/// The state of a CPU. The bootstrap processor will start in `Running`, while the APs start in
/// `WaitingForSipi`. Processors marked `Disabled` are disabled (for faulty hardware, for
/// example)
#[derive(Clone,Debug)]
pub enum CpuState
{
    Running,
    WaitingForSipi,
    Disabled,
}

/// A physical CPU. On SMP systems, each core appears as a separate CPU.
#[derive(Clone,Debug)]
pub struct Cpu
{
    processor_id    : u8,
    local_apic_id   : u8,
    is_ap           : bool,
    state           : CpuState,
}

impl Cpu
{
    pub fn new(processor_id     : u8,
               local_apic_id    : u8,
               is_ap            : bool,
               state            : CpuState) -> Cpu
    {
        Cpu
        {
            processor_id,
            local_apic_id,
            is_ap,
            state,
        }
    }
}

/// Notify the CPU that this is a spinlock wait loop. This allows certain memory and cache access
/// optimizations. Unsafe as `pause` is a specialised instruction, and can be detrimental to CPU
/// performance if misused.
pub unsafe fn notify_spinlock()
{
    asm!("pause");
}

/// Idle the CPU until an interrupt wakes it up. Useful for spinning until an interrupt handler
/// signals that something is done. Unsafe because it halts the system, and turns on interrupts!
pub unsafe fn wait_for_interrupt()
{
    asm!("sti
          hlt");
}
