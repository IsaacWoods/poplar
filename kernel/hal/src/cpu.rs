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
