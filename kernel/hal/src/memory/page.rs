use super::{FrameSize, Size4KiB, VirtualAddress};
use core::{
    iter::Step,
    marker::PhantomData,
    ops::{Add, AddAssign},
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Page<S: FrameSize = Size4KiB> {
    pub start: VirtualAddress,
    _phantom: PhantomData<S>,
}

impl<S> Page<S>
where
    S: FrameSize,
{
    pub fn starts_with(address: VirtualAddress) -> Page<S> {
        assert!(usize::from(address) % S::SIZE == 0, "Address is not at the start of a page");
        Page { start: address, _phantom: PhantomData }
    }

    pub fn contains(address: VirtualAddress) -> Page<S> {
        Page { start: address.align_down(S::SIZE), _phantom: PhantomData }
    }
}

impl<S> Add<usize> for Page<S>
where
    S: FrameSize,
{
    type Output = Page<S>;

    fn add(self, num_pages: usize) -> Self::Output {
        Page::contains(self.start + num_pages * S::SIZE)
    }
}

impl<S> AddAssign<usize> for Page<S>
where
    S: FrameSize,
{
    fn add_assign(&mut self, num_pages: usize) {
        *self = *self + num_pages;
    }
}

unsafe impl<S> Step for Page<S>
where
    S: FrameSize,
{
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        let address_difference = usize::from(end.start).checked_sub(usize::from(start.start))?;
        assert!(address_difference % S::SIZE == 0);
        Some(address_difference / S::SIZE)
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        Some(Page { start: start.start.checked_add(S::SIZE.checked_mul(count)?)?, _phantom: PhantomData })
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        Some(Page { start: start.start.checked_sub(S::SIZE.checked_mul(count)?)?, _phantom: PhantomData })
    }
}
