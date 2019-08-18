use super::{FrameSize, Size4KiB, VirtualAddress};
use core::{
    iter::Step,
    marker::PhantomData,
    ops::{Add, AddAssign},
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Page<S: FrameSize = Size4KiB> {
    pub start_address: VirtualAddress,
    _phantom: PhantomData<S>,
}

impl<S> Page<S>
where
    S: FrameSize,
{
    pub fn starts_with(address: VirtualAddress) -> Page<S> {
        assert!(usize::from(address) % S::SIZE == 0, "Address is not at the start of a page");
        Page { start_address: address, _phantom: PhantomData }
    }

    pub fn contains(address: VirtualAddress) -> Page<S> {
        Page { start_address: address.align_down(S::SIZE), _phantom: PhantomData }
    }
}

impl<S> Add<usize> for Page<S>
where
    S: FrameSize,
{
    type Output = Page<S>;

    fn add(self, num_pages: usize) -> Self::Output {
        assert!(VirtualAddress::new(usize::from(self.start_address) + num_pages * S::SIZE).is_some());
        Page { start_address: self.start_address + num_pages * S::SIZE, _phantom: PhantomData }
    }
}

impl<S> AddAssign<usize> for Page<S>
where
    S: FrameSize,
{
    fn add_assign(&mut self, num_pages: usize) {
        assert!(VirtualAddress::new(usize::from(self.start_address) + num_pages * S::SIZE).is_some());
        self.start_address += num_pages * S::SIZE;
    }
}

impl<S> Step for Page<S>
where
    S: FrameSize,
{
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        let address_difference =
            usize::from(end.start_address).checked_sub(usize::from(start.start_address))?;
        assert!(address_difference % S::SIZE == 0);
        Some(address_difference / S::SIZE)
    }

    fn replace_one(&mut self) -> Self {
        self.start_address = unsafe { VirtualAddress::new_unchecked(S::SIZE) };
        *self
    }

    fn replace_zero(&mut self) -> Self {
        self.start_address = unsafe { VirtualAddress::new_unchecked(0x0) };
        *self
    }

    fn add_one(&self) -> Self {
        Page { start_address: self.start_address + S::SIZE, _phantom: PhantomData }
    }

    fn sub_one(&self) -> Self {
        Page { start_address: self.start_address - S::SIZE, _phantom: PhantomData }
    }

    fn add_usize(&self, n: usize) -> Option<Self> {
        Some(Page {
            start_address: VirtualAddress::new(usize::from(self.start_address).checked_add(n * S::SIZE)?)
                .unwrap(),
            _phantom: PhantomData,
        })
    }
}
