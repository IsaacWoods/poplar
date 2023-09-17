use core::mem;
use hal::memory::VAddr;

/// Hardware task switching isn't supported on x86_64, so the TSS is just used as a vestigal place
/// to stick stuff. It's used to store kernel-level stacks that should be used if interrupts occur
/// (this is used to prevent triple-faults from occuring if we overflow the kernel stack).
#[derive(Clone, Copy, Debug)]
#[repr(C, packed(4))]
pub struct Tss {
    _reserved_1: u32,
    pub privilege_stack_table: [VAddr; 3],
    _reserved_2: u64,
    pub interrupt_stack_table: [VAddr; 7],
    _reserved_3: u64,
    _reserved_4: u16,
    pub iomap_base: u16,
}

impl Tss {
    pub fn new() -> Tss {
        Tss {
            _reserved_1: 0,
            privilege_stack_table: [VAddr::new(0x0); 3],
            _reserved_2: 0,
            interrupt_stack_table: [VAddr::new(0x0); 7],
            _reserved_3: 0,
            _reserved_4: 0,
            iomap_base: mem::size_of::<Tss>() as u16,
        }
    }

    pub fn set_kernel_stack(&mut self, stack_pointer: VAddr) {
        self.privilege_stack_table[0] = stack_pointer;
    }
}
