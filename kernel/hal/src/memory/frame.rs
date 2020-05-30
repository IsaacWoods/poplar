use super::{FrameSize, PhysicalAddress, Size4KiB};
use core::{
    iter::Step,
    marker::PhantomData,
    ops::{Add, AddAssign},
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Frame<S = Size4KiB>
where
    S: FrameSize,
{
    pub start: PhysicalAddress,
    _phantom: PhantomData<S>,
}

impl<S> Frame<S>
where
    S: FrameSize,
{
    pub fn starts_with(address: PhysicalAddress) -> Frame<S> {
        assert!(address.is_aligned(S::SIZE));
        Frame { start: address, _phantom: PhantomData }
    }

    pub fn contains(address: PhysicalAddress) -> Frame<S> {
        Frame { start: address.align_down(S::SIZE), _phantom: PhantomData }
    }
}

impl<S> Add<usize> for Frame<S>
where
    S: FrameSize,
{
    type Output = Frame<S>;

    fn add(self, num_frames: usize) -> Self::Output {
        assert!(PhysicalAddress::new(usize::from(self.start) + num_frames * S::SIZE).is_some());
        Frame { start: self.start + num_frames * S::SIZE, _phantom: PhantomData }
    }
}

impl<S> AddAssign<usize> for Frame<S>
where
    S: FrameSize,
{
    fn add_assign(&mut self, num_frames: usize) {
        assert!(PhysicalAddress::new(usize::from(self.start) + num_frames * S::SIZE).is_some());
        self.start += num_frames * S::SIZE;
    }
}

unsafe impl<S> Step for Frame<S>
where
    S: FrameSize,
{
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        let address_difference = usize::from(end.start).checked_sub(usize::from(start.start))?;
        assert!(address_difference % S::SIZE == 0);
        Some(address_difference / S::SIZE)
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        Some(Frame { start: start.start.checked_add(S::SIZE.checked_mul(count)?)?, _phantom: PhantomData })
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        Some(Frame { start: start.start.checked_sub(S::SIZE.checked_mul(count)?)?, _phantom: PhantomData })
    }
}
