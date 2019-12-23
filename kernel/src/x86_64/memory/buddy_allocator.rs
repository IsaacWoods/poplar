//! This module implements a buddy allocator, an efficient method for managing physical memory. In
//! this allocator, memory is broken up into a number of blocks, each of which is a power-of-2 in
//! size. A block of size `2^^n` frames is said to be of order `n`:
//!
//!       16                               0       Order       Size of blocks
//!        |-------------------------------|
//!        |               8               |       4           2^^4 = 16
//!        |---------------|---------------|
//!        |      12       |       4       |       3           2^^3 = 8
//!        |-------|-------|-------|-------|
//!        |  14   |  10   |   6   |   2   |       2           2^^2 = 4
//!        |---|---|---|---|---|---|---|---|
//!        |   |   |   |   |   |   |   |   |       1           2^^1 = 2
//!        |-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|
//!        | | | | | | | | | | | | | | | | |       0           2^^0 = 1
//!        |-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|
//!
//! The blocks at each order are arranged in pairs - each has a "buddy" where A's buddy is B and
//! B's buddy is A. The address of a block's buddy can be efficiently calculated using XOR (see the
//! `buddy_of` function). Blocks are stored in bins, where each bin contains blocks of a single
//! order.
//!
//! When an allocation is requested, the allocator looks in the bin of the correct size. If a block
//! is available, that block is removed from the bin and returned. If no blocks of the correct size
//! are available, a block of the order above the one requested is removed from its bin, split into
//! two blocks of the order requested (which are "buddies" of each other), of which one is returned
//! and one is added to the correct bin. If no blocks of the larger order are available, this
//! process continues recursively upwards to larger and larger block sizes, until a free block is
//! found. If no block is found, an "out of memory" error is issued.
//!
//! When a block is "freed" (returned to the allocator so that it can be allocated again), the
//! allocator checks to see if its "buddy" is also free. If it is, the two blocks of order `x` are
//! merged into a single block of order `x + 1`. This process continues upwards recursively until a
//! block's buddy is allocated (this can also occur before the first merging, and the freed block
//! is immediately added to the bin for order `x`), at which point no further merging can happen
//! and the block is added to the correct bin.
//!
//! Overall, the buddy allocator is an efficient allocator that has a much lower cost than other
//! algorithms such as first-fit. It also helps reduce external fragmentation, but can suffer from
//! internal fragmentation in the case of allocations that are slightly larger than a block size
//! (e.g. allocating 17 frames actually returns 32, wasting 15 frames). If the kernel needs to make
//! many allocations of a constant, "known bad" size (e.g. 3 frames at a time), it would be better
//! served allocating a larger block of frames at a time, and using a slab allocator to make the
//! individual allocations.

use alloc::{collections::BTreeSet, vec::Vec};
use core::{cmp::min, ops::Range};
use pebble_util::math::{ceiling_log2, flooring_log2};
use x86_64::memory::{Frame, FrameSize, PhysicalAddress, Size4KiB};

// TODO: make this generic over the frame size - it should monomorphise and generate good code I
// think
// TODO: don't make the no. of bins dynamic - just set of constant and use an array (or at least
// switch to const generics)
pub struct BuddyAllocator {
    /// The bins of free blocks, where bin `i` contains blocks of size `2^i`. Uses `BTreeSet` to
    /// store the blocks in each bin, for efficient buddy location. The `Frame` stored for each
    /// block is the first frame in that block - the end frame can be calculated from the start
    /// frame and the order of the bin the block is in.
    // TODO: when generic constants are a thing, we might be able to use `[BTreeSet; N]` here.
    bins: Vec<BTreeSet<Frame<Size4KiB>>>,
}

impl BuddyAllocator {
    /// Create a new `BuddyAllocator`, with a maximum block size of `2^max_order`.
    pub fn new(max_order: usize) -> BuddyAllocator {
        BuddyAllocator { bins: vec![BTreeSet::new(); max_order + 1] }
    }

    /// Add a range of `Frame`s to this allocator, marking them free to allocate.
    pub fn add_range(&mut self, range: Range<Frame>) {
        /*
         * Break the frame area into a set of blocks with power-of-2 sizes, and register each
         * block as free to allocate.
         */
        let mut block_start = range.start;

        while block_start < range.end {
            /*
             * Pick the largest order block that fits in the remaining area, but cap it at the
             * largest order the allocator can manage.
             */
            let order = min(self.max_order(), flooring_log2((block_start..range.end).count() as u64) as usize);

            self.free_n(block_start, 1 << order);
            block_start += 1 << order;
        }
    }

    /// Allocate (at least) `n` contiguous frames from this allocator. Returns `None` if this
    /// allocator can't satisfy the allocation.
    pub fn allocate_n(&mut self, n: usize) -> Option<Frame> {
        /*
         * Work out the size of block we need to fit `n` frames by rounding up to the next
         * power-of-2, then recursively try and allocate a block of that order.
         */
        let block_order = ceiling_log2(n as u64);
        self.allocate_block(block_order as usize)
    }

    /// Free the given block (starting at `start` and of size `n` frames). `n` must be a
    /// power-of-2.
    pub fn free_n(&mut self, start_frame: Frame, n: usize) {
        assert!(n.is_power_of_two());
        let order = flooring_log2(n as u64) as usize;

        if order == self.max_order() {
            /*
             * Blocks of the maximum order can't be coalesced, because there isn't a bigger bin
             * to put them into.
             */
            assert!(!self.bins[order].contains(&start_frame));
            self.bins[order].insert(start_frame);
            return;
        }

        /*
         * Check if this block's buddy is also free. If it is, remove it and coalesce the blocks
         * by recursively freeing at the order above this one.
         */
        let buddy = BuddyAllocator::buddy_of(start_frame, order);
        if self.bins[order].remove(&buddy) {
            self.free_n(min(start_frame, buddy), 1 << (order + 1));
        } else {
            /*
             * The buddy isn't free, insert the block at this order.
             */
            assert!(!self.bins[order].contains(&start_frame));
            self.bins[order].insert(start_frame);
        }
    }

    /// Tries to allocate a block of the given order. If no blocks of the correct size are
    /// available, tries to recursively split a larger block to form a block of the requested size.
    fn allocate_block(&mut self, order: usize) -> Option<Frame> {
        /*
         * We've been asked for a block larger than the largest blocks we track, so we won't be
         * able to allocate a single block large enough.
         */
        if order > self.max_order() {
            return None;
        }

        /*
         * If that order's bin has any free blocks, use one of those.
         */
        if let Some(&block) = self.bins[order].iter().next() {
            return self.bins[order].take(&block);
        }

        /*
         * Otherwise, try to allocate a block of the order one larger, and split it in two.
         */
        if let Some(block) = self.allocate_block(order + 1) {
            let second_half = BuddyAllocator::buddy_of(block, order);
            self.free_n(second_half, 1 << order);
            Some(block)
        } else {
            /*
             * This allocator doesn't have any blocks that are able to satify the request.
             */
            None
        }
    }

    /// Finds the starting frame of the block that is the buddy of the block of order `order`,
    /// starting at `x`.
    fn buddy_of(x: Frame, order: usize) -> Frame {
        /*
         * TODO: describe what this does and how
         *
         * We add `LOG2_SIZE` to the order as more efficient version of `(address ^ (1 << order)) * SIZE`,
         * because we're dealing with frame **addresses**, whereas the buddy algorithm works in
         * block numbers.
         */
        Frame::contains(
            PhysicalAddress::new(usize::from(x.start_address) ^ (1 << (order + Size4KiB::LOG2_SIZE))).unwrap(),
        )
    }

    /// Get the order of the largest block this allocator can track.
    fn max_order(&self) -> usize {
        self.bins.len() - 1
    }
}

// TODO: actually test the allocator as well
#[cfg(test)]
mod tests {
    use super::*;
    use x86_64::memory::{Frame, PhysicalAddress};

    #[test]
    fn test_buddy_of() {
        macro test($order: expr, $first: expr, $second: expr) {
            assert_eq!(
                BuddyAllocator::buddy_of(Frame::starts_with(PhysicalAddress::new($first).unwrap()), $order),
                Frame::starts_with(PhysicalAddress::new($second).unwrap())
            );
        }

        test!(0, 0x0, 0x1000);
        test!(0, 0x1000, 0x0);
        test!(0, 0x2000, 0x3000);
        test!(0, 0x3000, 0x2000);
        test!(0, 0x170000, 0x171000);

        test!(1, 0x0, 0x2000);
        test!(1, 0x2000, 0x0);

        test!(2, 0x0, 0x4000);
        test!(2, 0x4000, 0x0);

        test!(3, 0x0, 0x8000);
        test!(3, 0x8000, 0x0);

        test!(4, 0x0, 0x10000);
        test!(4, 0x10000, 0x0);
        test!(4, 0x160000, 0x170000);
        test!(4, 0x170000, 0x160000);
    }
}
