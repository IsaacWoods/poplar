use bit_field::BitField;
use core::fmt;

/// A wrapper for the `RFLAGS` register, providing a nice `Debug` implementation that details which
/// flags are set and unset.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct CpuFlags(pub u64);

impl CpuFlags {
    /// Read the contents of `RFLAGS`, creating a `CpuFlags`.
    pub fn read() -> CpuFlags {
        let flags: u64;
        unsafe {
            asm!("pushfq
                  pop rax"
                 : "={rax}"(flags)
                 :
                 : "rax"
                 : "intel", "volatile");
        }

        CpuFlags(flags)
    }

    pub fn interrupts_enabled(&self) -> bool {
        self.0.get_bit(9)
    }
}

impl fmt::Debug for CpuFlags {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[{}{}{}{}{}{}{}{}{}{}{}] {:#x}",
            if self.0.get_bit(14) { 'N' } else { '-' }, // Nested Task flag
            ['0', '1', '2', '3'][(self.0.get_bits(12..14)) as usize], // I/O privilege level
            if self.0.get_bit(11) { 'O' } else { '-' }, // Overflow flag
            if self.0.get_bit(10) { 'D' } else { '-' }, // Direction flag
            if self.0.get_bit(9) { 'I' } else { '-' },  // Interrupt flag
            if self.0.get_bit(8) { 'T' } else { '-' },  // Trap flag
            if self.0.get_bit(7) { 'S' } else { '-' },  // Sign flag
            if self.0.get_bit(6) { 'Z' } else { '-' },  // Zero flag
            if self.0.get_bit(4) { 'A' } else { '-' },  // Adjust flag
            if self.0.get_bit(2) { 'P' } else { '-' },  // Parity flag
            if self.0.get_bit(0) { 'C' } else { '-' },  // Carry flag
            self.0
        )
    }
}

/*
 * Constants for bits in CR4.
 */
/// If this is set, `rdtsc` can only be used in Ring 0.
pub const CR4_RESTRICT_RDTSC: usize = 2;
pub const CR4_ENABLE_PAE: usize = 5;
pub const CR4_ENABLE_GLOBAL_PAGES: usize = 7;
pub const CR4_XSAVE_ENABLE_BIT: usize = 18;

/// Read a control register. The name of the control register should be passed as any of: `CR0`,
/// `CR1`, `CR2`, `CR3`, `CR4`, `CR8`.
pub macro read_control_reg($reg: ident) {{
    let result: u64;

    /*
     * If this macro is used inside an unsafe block, this causes a warning, which can be unexpected and is noisy,
     * so we suppress it here.
     */
    #[allow(unused_unsafe)]
    unsafe {
        asm!(concat!("mov %", stringify!($reg), ", $0")
             : "=r"(result)
             :
             : "memory"
             : "volatile"
            );
    }
    result
}}

/// Write to a control register. Calls to this macro will need to be inside an unsafe block, which
/// is intended because writing to control registers can be kinda dangerous. The name of the control
/// register should be passed as any of: `CR0`, `CR1`, `CR2`, `CR3`, `CR4`, `CR8`.
pub macro write_control_reg($reg: ident, $value: expr) {
    /*
     * This will cause a type-check error if $value isn't a u64.
     */
    let value_u64: u64 = $value;
    asm!(concat!("mov $0, %", stringify!($reg))
         :
         : "r"(value_u64)
         : "memory"
         : "volatile"
        );
}

pub const EFER: u32 = 0xc000_0080;

pub const EFER_ENABLE_SYSCALL: usize = 0;
pub const EFER_ENABLE_LONG_MODE: usize = 8;
pub const EFER_ENABLE_NX_BIT: usize = 11;

/// Contains the Ring 0 and Ring 3 code-segment selectors loaded by `syscall` and `sysret`,
/// respectively:
/// * `syscall` loads bits 32-47 into CS (so this should be the Ring 0 code-segment)
/// * `sysret` loads bits 48-63 into CS (so this should be the Ring 3 code-segment)
///
/// These instructions assume that the data-segment for each ring is directly after the
/// code-segment.
pub const IA32_STAR: u32 = 0xc000_0081;

/// Contains the virtual address of the handler to call upon `syscall`.
pub const IA32_LSTAR: u32 = 0xc000_0082;

/// Upon `syscall`, the value of this MSR is used to mask `RFLAGS`. Specifically, if a bit is set
/// in this MSR, that bit in RFLAGS is zerod.
pub const IA32_FMASK: u32 = 0xc000_0084;

/// A virtual address can be stored in this MSR, and acts as the base of the GS segment.
pub const IA32_GS_BASE: u32 = 0xc000_0101;

/// Read from a model-specific register.
pub fn read_msr(reg: u32) -> u64 {
    let (high, low): (u32, u32);
    unsafe {
        asm!("rdmsr"
         : "={eax}"(low), "={edx}"(high)
         : "{ecx}"(reg)
         : "memory"
         : "volatile"
        );
    }
    (high as u64) << 32 | (low as u64)
}

/// Write to a model-specific register. This is unsafe, because writing to certain MSRs can
/// compromise memory safety.
pub unsafe fn write_msr(reg: u32, value: u64) {
    asm!("wrmsr"
     :
     : "{ecx}"(reg), "{eax}"(value as u32), "{edx}"(value.get_bits(32..64) as u32)
     : "memory"
     : "volatile"
    );
}
