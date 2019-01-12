use crate::memory::VirtualAddress;
use alloc::boxed::Box;
use core::pin::Pin;

/// Hardware task switching isn't supported on x86_64, so the TSS is just used as a vestigal place
/// to stick stuff. It's used to store kernel-level stacks that should be used if interrupts occur
/// (this is used to prevent triple-faults from occuring if we overflow the kernel stack).
#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct Tss {
    _reserved_1: u32,
    pub privilege_stack_table: [VirtualAddress; 3],
    _reserved_2: u64,
    pub interrupt_stack_table: [VirtualAddress; 7],
    _reserved_3: u64,
    _reserved_4: u16,
    pub iomap_base: u16,
}

impl Tss {
    pub fn new() -> Pin<Box<Tss>> {
        Pin::new(box Tss {
            _reserved_1: 0,
            privilege_stack_table: [unsafe { VirtualAddress::new_unchecked(0) }; 3],
            _reserved_2: 0,
            interrupt_stack_table: [unsafe { VirtualAddress::new_unchecked(0) }; 7],
            _reserved_3: 0,
            _reserved_4: 0,
            iomap_base: 0,
        })
    }

    pub fn set_kernel_stack(&mut self, address: VirtualAddress) {
        self.privilege_stack_table[0] = address;
    }
}
