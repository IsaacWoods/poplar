use super::registers::{read_control_reg, write_control_reg};
use core::arch::asm;
use hal::memory::VirtualAddress;

#[rustfmt::skip]
pub fn invalidate_page(address: VirtualAddress) {
    unsafe {
        asm!("invlpg [{}]", in(reg) usize::from(address));
    }
}

pub fn flush() {
    let current_cr3 = read_control_reg!(cr3);
    unsafe {
        write_control_reg!(cr3, current_cr3);
    }
}
