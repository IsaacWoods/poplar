use bit_field::BitField;

pub type Index = u16;
pub type Generation = u16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KernelObjectId {
    pub index: Index,
    pub generation: Generation,
}

impl KernelObjectId {
    /// Used to convert between the form the kernel represents kernel object IDs with for system
    /// calls, and `KernelObjectId`. Should not be used from normal usercode (unless you're trying
    /// to make a raw system call).
    pub fn from_syscall_repr(repr: usize) -> KernelObjectId {
        /*
         * Index is in bits 0..16
         * Generation is in bits 16..32
         */
        let index = repr.get_bits(0..16) as Index;
        let generation = repr.get_bits(16..32) as Generation;

        KernelObjectId { index, generation }
    }

    /// Convert this `KernelObjectId` to the form used in the system call interface. Should not be
    /// used from normal usercode (unless you're trying to make a raw system call).
    pub fn to_syscall_repr(self) -> usize {
        let mut repr: usize = 0;
        repr.set_bits(0..16, self.index as usize);
        repr.set_bits(16..32, self.generation as usize);
        repr
    }
}
