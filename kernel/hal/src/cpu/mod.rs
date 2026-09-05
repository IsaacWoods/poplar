use core::arch::asm;

pub mod interrupt;
pub mod tables;

macro_rules! read_control_reg {
    ($cr:ident) => {
        {
            let result: u64;
            #[allow(unused_unsafe)] // Suppresses noisy warning when macro appears within an unsafe block
            unsafe {
                core::arch::asm!(concat!("mov {}, ", stringify!($cr)), out(reg) result);
            }
            result
        }
    };
}

/// Write to a control register. Must appear within an `unsafe` block.
macro_rules! write_control_reg {
    ($cr:ident, $value:expr) => {
        let value_u64: u64 = $value;    // Type-check that the value is a u64
        core::arch::asm!(concat!("mov ", stringify!($reg), " , {}"), in(reg) value_u64);
    }
}

pub struct Cr3;

impl Cr3 {
    pub fn read() -> u64 {
        read_control_reg!(cr3)
    }

    pub unsafe fn write(value: u64) {
        unsafe {
            write_control_reg!(cr3, value);
        }
    }
}

bitfield::bitfield! {
    // TODO: the `bitfield` macro should generate a better `Debug`
    #[derive(Debug)]
    pub struct CpuFlags<u64> {
        pub const CARRY: bool;
        const _RESERVED0: bool;
        pub const PARITY: bool;
        const _RESERVED1: bool;
        pub const AUX_CARRY: bool;
        const _RESERVED2: bool;
        pub const ZERO: bool;
        pub const SIGN: bool;
        pub const TRAP: bool;
        pub const INTERRUPT_EN: bool;
        pub const DIRECTION: bool;
        pub const OVERFLOW: bool;
        pub const IO_PRIV = 2;
        pub const NESTED_TASK: bool;
        const _RESERVED3: bool;
        pub const RESUME: bool;
        pub const VIRTUAL_8086: bool;
        pub const ALIGNMENT_CHECK: bool;
        pub const VIRT_INTERRUPT: bool;
        pub const VIRT_INTERRUPT_PENDING: bool;
        pub const CPUID: bool;
    }
}

impl CpuFlags {
    pub fn read() -> CpuFlags {
        let result: u64;
        unsafe {
            asm!("
                    pushfq
                    pop rax
                ",
                out("rax") result);
        }
        CpuFlags::from_raw(result)
    }
}
