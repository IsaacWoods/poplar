pub mod cpu;
pub mod gdt;
pub mod i8259_pic;
pub mod idt;
pub mod io_apic;
pub mod local_apic;
pub mod port;
pub mod registers;
pub mod serial;
pub mod tlb;
pub mod tss;

#[cfg(feature = "qemu")]
pub mod qemu;

use hal::memory::VAddr;

#[repr(C, packed)]
pub struct DescriptorTablePointer {
    /// `base + limit` is the last addressable byte of the descriptor table.
    pub limit: u16,

    /// Virtual address of the start of the descriptor table.
    pub base: VAddr,
}
