//! This module implements a buddy allocator, an efficient scheme for managing physical memory. In
//! this allocator, memory is broken up into a number of blocks, each of which is a power-of-2 frames in
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
//! The blocks in each order are arranged in pairs - each has a "buddy" where A's buddy is B and B's
//! buddy is A. To find the address of a block's buddy, the bit corresponding to the order is simply
//! flipped (and so is easily calculated with XOR - see the `buddy_of` method). Blocks are tracked in
//! bins, where each bin contains blocks of a single order.
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
//! allocator checks to see if its "buddy" is also free. If it's not, the block is just added to the
//! bin corresponding to its order. However, if its buddy is also free, the two can be merged to form
//! a block of an order one greater - this process happens recursively until the block's buddy is not
//! free, at which point the block is added to the correct bin.
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
use hal::memory::{Frame, FrameSize, PhysicalAddress, Size4KiB};
use pebble_util::math::{ceiling_integer_divide, ceiling_log2, flooring_log2};

/// The largest block stored by the buddy allocator is `2^MAX_ORDER`.
const MAX_ORDER: usize = 10;

/// The "base" block size - the smallest block size this allocator tracks. This is chosen at the moment to be 4096
/// bytes - the size of the smallest physical frame for all the architectures we wish to support at this point of
/// time.
const BASE_SIZE: usize = Size4KiB::SIZE;

// TODO: don't make the no. of bins dynamic - just set of constant and use an array (or at least
// switch to const generics)
pub struct BuddyAllocator {
    /// The bins of free blocks, where bin `i` contains blocks of size `2^i`. Uses `BTreeSet` to
    /// store the blocks in each bin, for efficient buddy location. Each block is stored as the physical address
    /// of the start of the block. The actual frames can be constructed for each block using the start address and
    /// the order of the block.
    // TODO: when generic constants are a thing, we might be able to use `[BTreeSet; N]` here.
    bins: Vec<BTreeSet<PhysicalAddress>>,
}

impl BuddyAllocator {
    pub fn new() -> BuddyAllocator {
        BuddyAllocator { bins: vec![BTreeSet::new(); MAX_ORDER + 1] }
    }

    /// Add a range of `Frame`s to this allocator, marking them free to allocate.
    pub fn add_range(&mut self, range: Range<Frame>) {
        // XXX: if we ever change BASE_SIZE, this needs to be adjusted, so we assert here
        assert_eq!(BASE_SIZE, Size4KiB::SIZE);

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
            let num_frames = (block_start..range.end).count();
            let order = min(MAX_ORDER, flooring_log2(num_frames));

            self.free_block(block_start.start, order);
            block_start += 1 << order;
        }
    }

    /// Allocate (at least) `n` contiguous bytes from this allocator. Returns `None` if this
    /// allocator can't satisfy the allocation. The requested number of bytes must be a power-of-two.
    pub fn allocate_n(&mut self, num_bytes: usize) -> Option<PhysicalAddress> {
        assert!(num_bytes.is_power_of_two());

        /*
         * Find the minimum block order that will satisfy `num_bytes`.
         */
        let order = ceiling_log2(ceiling_integer_divide(num_bytes, BASE_SIZE));
        self.allocate_block(order)
    }

    /// Free the given block (starting at `start` and of size `n` frames). `n` must be a
    /// power-of-2.
    pub fn free_n(&mut self, start: PhysicalAddress, num_bytes: usize) {
        assert!(num_bytes.is_power_of_two());
        let order = flooring_log2(num_bytes / BASE_SIZE);
        self.free_block(start, order);
    }

    /// Tries to allocate a block of the given order. If no blocks of the correct size are
    /// available, tries to recursively split a larger block to form a block of the requested size.
    fn allocate_block(&mut self, order: usize) -> Option<PhysicalAddress> {
        /*
         * We've been asked for a block larger than the largest blocks we track, so we won't be
         * able to allocate a single block large enough.
         */
        if order > MAX_ORDER {
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

    /// Free a block starting at `start` of order `order`.
    fn free_block(&mut self, start: PhysicalAddress, order: usize) {
        if order == MAX_ORDER {
            /*
             * Blocks of the maximum order can't be coalesced, because there isn't a bin to put them in, so we
             * just add them to the largest bin.
             */
            assert!(!self.bins[order].contains(&start));
            self.bins[order].insert(start);
        } else {
            /*
             * Check if this block's buddy is also free. If it is, remove it from its bin and coalesce the
             * blocks into one of the order above this one. We then recursively free that block.
             */
            let buddy = Self::buddy_of(start, order);
            if self.bins[order].remove(&buddy) {
                self.free_block(min(start, buddy), order + 1);
            } else {
                /*
                 * The buddy isn't free, so just insert the block at this order.
                 */
                assert!(!self.bins[order].contains(&start));
                self.bins[order].insert(start);
            }
        }
    }

    /// Finds the starting frame of the block that is the buddy of the block of order `order`, starting at
    /// `block_start`.
    fn buddy_of(block_start: PhysicalAddress, order: usize) -> PhysicalAddress {
        PhysicalAddress::new(usize::from(block_start) ^ ((1 << order) * BASE_SIZE)).unwrap()
    }
}

// TODO: actually test the allocator as well
#[cfg(test)]
mod tests {
    use super::*;
    use hal::memory::{Frame, PhysicalAddress};

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
