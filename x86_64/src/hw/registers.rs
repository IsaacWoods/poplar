/// Read a control register. The name of the control register should be passed as any of: `CR0`,
/// `CR1`, `CR2`, `CR3`, `CR4`, `CR8`.
#[rustfmt::skip]
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
#[rustfmt::skip]
pub macro write_control_reg($reg: ident, $value: expr) {
    asm!(concat!("mov $0, %", stringify!($reg))
         :
         : "r"($value)
         : "memory"
         : "volatile"
        );
}

pub const EFER: u32 = 0xC0000080;

/// Read from a model-specific register.
#[rustfmt::skip]
pub macro read_msr($reg: expr) {{
    let (high, low): (u32, u32);
    unsafe {
        asm!("rdmsr"
             : "={eax}"(low), "={edx}"(high)
             : "{ecx}"($reg)
             : "memory"
             : "volatile"
            );
    }
    ((high as u64) << 32 | (low as u64))
}}

/// Write to a model-specific register. This is unsafe, because writing to certain MSRs can
/// compromise memory safety.
#[rustfmt::skip]
pub macro write_msr($reg: expr, $value: expr) {
    asm!("wrmsr"
         :
         : "{ecx}"($reg), "{eax}"(($value) as u32), "{edx}"((($value) >> 32) as u32)
         : "memory"
         : "volatile"
        );
}
