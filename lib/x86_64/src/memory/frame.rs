use super::{FrameSize, PhysicalAddress, Size4KiB};
use core::{
    iter::Step,
    marker::PhantomData,
    ops::{Add, AddAssign},
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Frame<S: FrameSize = Size4KiB> {
    pub start_address: PhysicalAddress,
    _phantom: PhantomData<S>,
}

impl<S> Frame<S>
where
    S: FrameSize,
{
    pub fn starts_with(address: PhysicalAddress) -> Frame<S> {
        assert!(address.is_frame_aligned::<S>());
        Frame { start_address: address, _phantom: PhantomData }
    }

    pub fn contains(address: PhysicalAddress) -> Frame<S> {
        Frame { start_address: address.align_down(S::SIZE), _phantom: PhantomData }
    }
}

impl<S> Add<usize> for Frame<S>
where
    S: FrameSize,
{
    type Output = Frame<S>;

    fn add(self, num_pages: usize) -> Self::Output {
        assert!(PhysicalAddress::new(usize::from(self.start_address) + num_pages * S::SIZE).is_some());
        Frame { start_address: self.start_address + num_pages * S::SIZE, _phantom: PhantomData }
    }
}

impl<S> AddAssign<usize> for Frame<S>
where
    S: FrameSize,
{
    fn add_assign(&mut self, num_frames: usize) {
        assert!(PhysicalAddress::new(usize::from(self.start_address) + num_frames * S::SIZE).is_some());
        self.start_address += num_frames * S::SIZE;
    }
}

impl<S> Step for Frame<S>
where
    S: FrameSize,
{
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        let address_difference = usize::from(end.start_address).checked_sub(usize::from(start.start_address))?;
        assert!(address_difference % S::SIZE == 0);
        Some(address_difference / S::SIZE)
    }

    fn replace_one(&mut self) -> Self {
        self.start_address = unsafe { PhysicalAddress::new_unchecked(S::SIZE) };
        *self
    }

    fn replace_zero(&mut self) -> Self {
        self.start_address = unsafe { PhysicalAddress::new_unchecked(0x0) };
        *self
    }

    fn add_one(&self) -> Self {
        Frame { start_address: self.start_address + S::SIZE, _phantom: PhantomData }
    }

    fn sub_one(&self) -> Self {
        Frame { start_address: self.start_address - S::SIZE, _phantom: PhantomData }
    }

    fn add_usize(&self, n: usize) -> Option<Self> {
        Some(Frame {
            start_address: PhysicalAddress::new(usize::from(self.start_address).checked_add(n * S::SIZE)?)
                .unwrap(),
            _phantom: PhantomData,
        })
    }
}
