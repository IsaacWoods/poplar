use bit_field::BitField;
use core::fmt;

/// A wrapper for the `RFLAGS` register, providing a nice `Debug` implementation that details which
/// flags are set and unset.
#[derive(Clone, Copy)]
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
            if self.0 & 0x0000_4000 > 0 { 'N' } else { '-' }, // Nested Task flag
            ['0', '1', '2', '3'][(self.0 & 0x0000_3001 >> 12) as usize], // I/O privilege level
            if self.0 & 0x0000_0800 > 0 { 'O' } else { '-' }, // Overflow flag
            if self.0 & 0x0000_0400 > 0 { 'D' } else { '-' }, // Direction flag
            if self.0 & 0x0000_0200 > 0 { 'I' } else { '-' }, // Interrupt flag
            if self.0 & 0x0000_0100 > 0 { 'T' } else { '-' }, // Trap flag
            if self.0 & 0x0000_0080 > 0 { 'S' } else { '-' }, // Sign flag
            if self.0 & 0x0000_0040 > 0 { 'Z' } else { '-' }, // Zero flag
            if self.0 & 0x0000_0010 > 0 { 'A' } else { '-' }, // Adjust flag
            if self.0 & 0x0000_0004 > 0 { 'P' } else { '-' }, // Parity flag
            if self.0 & 0x0000_0001 > 0 { 'C' } else { '-' }, // Carry flag
            self.0
        )
    }
}

/// Read a control register. The name of the control register should be passed as any of: `CR0`,
/// `CR1`, `CR2`, `CR3`, `CR4`, `CR8`.
pub macro read_control_reg($reg: ident) {{
    let result: u64;
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

pub const EFER: u32 = 0xC0000080;

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
    ((high as u64) << 32 | (low as u64))
}

/// Write to a model-specific register. This is unsafe, because writing to certain MSRs can
/// compromise memory safety.
pub unsafe fn write_msr(reg: u32, value: u64) {
    use bit_field::BitField;

    asm!("wrmsr"
         :
         : "{ecx}"(reg), "{eax}"(value as u32), "{edx}"(value.get_bits(32..64) as u32)
         : "memory"
         : "volatile"
        );
}
