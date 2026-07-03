use crate::cpu::CpuFlags;
use bnb::BitOps;
use core::{
    arch::asm,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use lock_api::{GuardNoSend, GuardSend, RawMutex};

pub type Spinlock<T> = lock_api::Mutex<RawSpinlock, T>;
pub type SpinlockGuard<'a, T> = lock_api::MutexGuard<'a, RawSpinlock, T>;

pub struct RawSpinlock(AtomicBool);

unsafe impl RawMutex for RawSpinlock {
    const INIT: RawSpinlock = RawSpinlock(AtomicBool::new(false));
    type GuardMarker = GuardSend;

    fn lock(&self) {
        while self
            .0
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
    }

    fn try_lock(&self) -> bool {
        self.0
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    unsafe fn unlock(&self) {
        self.0.store(false, Ordering::Release);
    }
}

/// A spinlock that disables interrupts when taken, and restores the previous interrupt state when
/// released.
pub type IrqSpinlock<T> = lock_api::Mutex<RawIrqSpinlock, T>;
pub type IrqSpinlockGuard<'a, T> = lock_api::MutexGuard<'a, RawIrqSpinlock, T>;

/// Backing spinlock for `IrqSpinlock`. Can be atomically taken and will disable interrupts on
/// locking, storing the previous interrupt state for the current CPU. When the lock is released,
/// the previous interrupt state is resumed.
///
/// Valid states
///    - `0` - the lock is free and may be taken
///    - `0b01` - the lock is taken, interrupts are disabled, and were disabled before taking the lock
///    - `0b11` - the lock is taken, interrupts are disabled, but should be re-enabled when the lock is released
pub struct RawIrqSpinlock(AtomicUsize);

unsafe impl RawMutex for RawIrqSpinlock {
    const INIT: RawIrqSpinlock = RawIrqSpinlock(AtomicUsize::new(0));
    /// Locking the mutex disables interrupts on the *current* CPU, and therefore locks must not be
    /// transferred between CPUs.
    type GuardMarker = GuardNoSend;

    fn lock(&self) {
        let interrupts_enabled: bool = CpuFlags::read().get(CpuFlags::INTERRUPT_EN);
        let value = 0b1 | if interrupts_enabled { 0b10 } else { 0 };
        unsafe { asm!("cli") };

        while self
            .0
            .compare_exchange_weak(0, value, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
    }

    fn try_lock(&self) -> bool {
        let interrupts_enabled: bool = CpuFlags::read().get(CpuFlags::INTERRUPT_EN);
        let state = 0b1 | if interrupts_enabled { 0b10 } else { 0 };
        unsafe { asm!("cli") };

        let taken = self
            .0
            .compare_exchange(0, state, Ordering::Acquire, Ordering::Relaxed)
            .is_ok();

        if !taken && interrupts_enabled {
            unsafe { asm!("sti") };
        }

        taken
    }

    unsafe fn unlock(&self) {
        let state = self.0.swap(0, Ordering::Release);
        if state.bit(1) {
            unsafe { asm!("sti") };
        }
    }
}
