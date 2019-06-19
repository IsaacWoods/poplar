pub type Index = u16;
pub type Generation = u16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KernelObjectId {
    pub index: Index,
    pub generation: Generation,
}
