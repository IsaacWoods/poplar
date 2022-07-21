use core::{marker::PhantomPinned, pin::Pin};
use hal::memory::VirtualAddress;

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

    /// The memory pointed to by a task register will be used as the TSS until the task register
    /// contents is replaced. This means the memory must never be moved, because then the task
    /// register would no longer point to the TSS. To enforce this using the type system, we pin
    /// the type.
    _pin: PhantomPinned,
}

impl Tss {
    pub fn new() -> Tss {
        Tss {
            _reserved_1: 0,
            privilege_stack_table: [VirtualAddress::new(0x0); 3],
            _reserved_2: 0,
            interrupt_stack_table: [VirtualAddress::new(0x0); 7],
            _reserved_3: 0,
            _reserved_4: 0,
            iomap_base: 0,
            _pin: PhantomPinned,
        }
    }

    pub fn set_kernel_stack(self: Pin<&mut Self>, address: VirtualAddress) {
        // Safety: this does not move out of the structure. We have to get an inner pointer like this because these
        // fields are unaligned, and so creating a reference to them (like we normally would with a pin projection)
        // would be UB
        unsafe {
            Pin::into_inner_unchecked(self).privilege_stack_table[0] = address;
        }
    }
}
