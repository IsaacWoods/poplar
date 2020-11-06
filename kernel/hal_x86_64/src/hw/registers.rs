use bit_field::BitField;
use core::{fmt, ops::Range};

/// A wrapper for the `RFLAGS` register, providing a nice `Debug` implementation that details which
/// flags are set and unset.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct CpuFlags(u64);

impl CpuFlags {
    pub const CARRY_FLAG: u64 = 0;
    pub const PARITY_FLAG: u64 = 2;
    pub const ADJUST_FLAG: u64 = 4;
    pub const ZERO_FLAG: u64 = 6;
    pub const SIGN_FLAG: u64 = 7;
    pub const TRAP_FLAG: u64 = 8;
    pub const INTERRUPT_ENABLE_FLAG: u64 = 9;
    pub const DIRECTION_FLAG: u64 = 10;
    pub const OVERFLOW_FLAG: u64 = 11;
    pub const IO_PRIVILEGE: Range<usize> = 12..14;
    pub const NESTED_TASK_FLAG: u64 = 14;
    pub const RESUME_FLAG: u64 = 16;
    pub const VIRTUAL_8086_FLAG: u64 = 17;
    pub const ALIGNMENT_CHECK_FLAG: u64 = 18;
    pub const VIRTUAL_INTERRUPT_FLAG: u64 = 19;
    pub const VIRTUAL_INTERRUPT_PENDING_FLAG: u64 = 20;
    pub const CPUID_FLAG: u64 = 21;

    /// This is a mask to select all of the 'status' bits out of the flags (the Carry, Parity, Adjust, Zero, Sign,
    /// Trap, Interrupt Enable, Direction, and Overflow flags)
    pub const STATUS_MASK: u64 = 0b110011010101;
    /// This is a mask to select the I/O privilege bits out of the flags
    pub const IO_PRIVILEGE_MASK: u64 = 0x3000;

    /// Read the contents of `RFLAGS`, creating a `CpuFlags`.
    pub fn read() -> CpuFlags {
        let flags: u64;
        unsafe {
            asm!("pushfq
                  pop {}",
                out(reg) flags
            );
        }

        CpuFlags(flags)
    }

    /// Create a new `CpuFlags`, with all the bits in `flags` set, then all the reserved bits set to their correct
    /// value.
    /// Note: this does not set `RFLAGS`!
    pub const fn new(flags: u64) -> CpuFlags {
        let mut result = flags;
        result |= 0b1000000000101010;
        result &= 0b11_1111_1111_1111_1111_1111;
        // TODO: this is equivalent to the above, but we need const fn in traits first
        // result.set_bit(1, true);
        // result.set_bit(3, false);
        // result.set_bit(5, false);
        // result.set_bit(15, false);
        // result.set_bits(22..64, 0);
        CpuFlags(result)
    }

    pub fn interrupts_enabled(&self) -> bool {
        self.0.get_bit(Self::INTERRUPT_ENABLE_FLAG as usize)
    }
}

impl From<CpuFlags> for u64 {
    fn from(flags: CpuFlags) -> Self {
        flags.0
    }
}

impl fmt::Debug for CpuFlags {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[({})({})({})({}){}{}{}{}{}{}{}{}{}{}{}{}{}] {:#x}",
            if self.0.get_bit(Self::CPUID_FLAG as usize) { "ID" } else { "-" },
            if self.0.get_bit(Self::VIRTUAL_INTERRUPT_PENDING_FLAG as usize) { "VIP" } else { "-" },
            if self.0.get_bit(Self::VIRTUAL_INTERRUPT_FLAG as usize) { "VIF" } else { "-" },
            if self.0.get_bit(Self::ALIGNMENT_CHECK_FLAG as usize) { "AC" } else { "-" },
            if self.0.get_bit(Self::VIRTUAL_8086_FLAG as usize) { 'V' } else { '-' },
            if self.0.get_bit(Self::RESUME_FLAG as usize) { 'R' } else { '-' },
            if self.0.get_bit(Self::NESTED_TASK_FLAG as usize) { 'N' } else { '-' },
            ['0', '1', '2', '3'][self.0.get_bits(Self::IO_PRIVILEGE) as usize],
            if self.0.get_bit(Self::OVERFLOW_FLAG as usize) { 'O' } else { '-' },
            if self.0.get_bit(Self::DIRECTION_FLAG as usize) { 'D' } else { '-' },
            if self.0.get_bit(Self::INTERRUPT_ENABLE_FLAG as usize) { 'I' } else { '-' },
            if self.0.get_bit(Self::TRAP_FLAG as usize) { 'T' } else { '-' },
            if self.0.get_bit(Self::SIGN_FLAG as usize) { 'S' } else { '-' },
            if self.0.get_bit(Self::ZERO_FLAG as usize) { 'Z' } else { '-' },
            if self.0.get_bit(Self::ADJUST_FLAG as usize) { 'A' } else { '-' },
            if self.0.get_bit(Self::PARITY_FLAG as usize) { 'P' } else { '-' },
            if self.0.get_bit(Self::CARRY_FLAG as usize) { 'C' } else { '-' },
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
        asm!(concat!("mov {}, ", stringify!($reg)), out(reg) result);
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
    asm!(concat!("mov ", stringify!($reg), ", {}"),
        in(reg) value_u64
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

/// A virtual address can be stored in this MSR, and acts as the base of the FS segment.
pub const IA32_FS_BASE: u32 = 0xc000_0100;

/// A virtual address can be stored in this MSR, and acts as the base of the GS segment.
pub const IA32_GS_BASE: u32 = 0xc000_0101;

/// Read from a model-specific register.
pub fn read_msr(reg: u32) -> u64 {
    let (high, low): (u32, u32);
    unsafe {
        asm!("rdmsr",
            in("ecx") reg,
            out("eax") low,
            out("edx") high
        );
    }
    (high as u64) << 32 | (low as u64)
}

/// Write to a model-specific register. This is unsafe, because writing to certain MSRs can
/// compromise memory safety.
pub unsafe fn write_msr(reg: u32, value: u64) {
    asm!("wrmsr",
        in("ecx") reg,
        in("eax") value.get_bits(0..32) as u32,
        in("edx") value.get_bits(32..64) as u32
    );
}
