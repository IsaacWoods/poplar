use crate::memory::VirtualAddress;
use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "kernel")] {
        pub mod gdt;
        pub mod tss;
        pub mod idt;
        pub mod i8259_pic;
    }
}

pub mod port;
pub mod registers;
pub mod serial;
pub mod tlb;

#[repr(C, packed)]
pub struct DescriptorTablePointer {
    /// `base + limit` is the last addressable byte of the descriptor table.
    pub limit: u16,

    /// Virtual address of the start of the descriptor table.
    pub base: VirtualAddress,
}
