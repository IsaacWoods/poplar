//! This module implements a buddy allocator, an efficient scheme for managing physical memory. In
//! this allocator, memory is broken up into a number of blocks, each of which is a power-of-2 frames in
//! size. A block of size `2^^n` frames is said to be of order `n`:
//! ```ignore
//!       16                               0       Order       Size of blocks (in frames)
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
//! ```
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

use alloc::collections::BTreeSet;
use core::{cmp::min, ops::Range};
use hal::memory::{Bytes, Frame, FrameSize, PAddr, Size4KiB};

/// The largest block stored by the buddy allocator is `2^MAX_ORDER`.
const MAX_ORDER: usize = 12;
const NUM_BINS: usize = MAX_ORDER + 1;

/// The "base" block size - the smallest block size this allocator tracks. This is chosen at the moment to be
/// `4096` bytes - the size of the smallest physical frame for all the architectures we wish to support at this
/// point of time.
const BASE_SIZE: usize = Size4KiB::SIZE;

#[derive(Clone, Debug)]
pub struct BuddyAllocator {
    /// The bins of free blocks, where bin `i` contains blocks of size `BASE_SIZE * (2^i)`. Uses `BTreeSet` to
    /// store the blocks in each bin, for efficient buddy location. Each block is stored as the physical address
    /// of the start of the block. The actual frames can be constructed for each block using the start address and
    /// the order of the block.
    bins: [BTreeSet<PAddr>; NUM_BINS],
}

impl BuddyAllocator {
    pub fn new() -> BuddyAllocator {
        // The `Default` implementation for `BTreeSet` is an empty set, so this works nicely
        BuddyAllocator { bins: Default::default() }
    }

    /// Free a range of `Frame`s into this allocator, marking them free to allocate.
    pub fn free_range(&mut self, range: Range<Frame<Size4KiB>>) {
        /*
         * Add each frame in the range to the allocator, allowing it to coalesce as it goes.
         */
        for frame in range {
            self.free_block(frame.start, 0);
        }
    }

    #[allow(dead_code)]
    pub fn available_bytes(&self) -> Bytes {
        let mut bytes = 0;
        for i in 0..NUM_BINS {
            bytes += self.bins[i].len() * ((1 << i) * BASE_SIZE);
        }
        bytes
    }

    /// Allocate a block of `count` base-blocks from this allocator. Returns `None` if the allocator can't satisfy
    /// the allocation.
    pub fn alloc(&mut self, count: usize) -> Option<PAddr> {
        /*
         * TODO: this requirement wastes a lot of space on larger allocations that aren't sized
         * nicely. We should be allocate a larger block, and then split the remaining space into
         * smaller blocks (this is the same problem as the initial block allocation - the blocks
         * need to be well-aligned for their sizes. Work out the maths at some point).
         */
        let count = count.next_power_of_two();

        let order = count.trailing_zeros() as usize;
        self.allocate_block(order)
    }

    /// Free a block starting at `base` of `count` base-blocks. `count` must be a power-of-2.
    pub fn free(&mut self, base: PAddr, count: usize) {
        assert!(count.is_power_of_two());

        let order = count.trailing_zeros() as usize;
        self.free_block(base, order);
    }

    /// Tries to allocate a block of the given order. If no blocks of the correct size are
    /// available, tries to recursively split a larger block to form a block of the requested size.
    fn allocate_block(&mut self, order: usize) -> Option<PAddr> {
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
            self.free_block(second_half, order);
            Some(block)
        } else {
            /*
             * This allocator doesn't have any blocks that are able to satify the request.
             */
            None
        }
    }

    /// Free a block starting at `start` of order `order`.
    fn free_block(&mut self, start: PAddr, order: usize) {
        if order == MAX_ORDER {
            /*
             * Blocks of the maximum order can't be coalesced, because there isn't a bin to put the result in,
             * so we just add them to the largest bin.
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
    fn buddy_of(block_start: PAddr, order: usize) -> PAddr {
        PAddr::new(usize::from(block_start) ^ ((1 << order) * BASE_SIZE)).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn test_buddy_of() {
        macro test($order: expr, $first: expr, $second: expr) {
            assert_eq!(
                BuddyAllocator::buddy_of(PAddr::new($first).unwrap(), $order),
                PAddr::new($second).unwrap()
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

    struct Block {
        order: usize,
        address: PAddr,
    }

    impl Block {
        /// Construct a `Block`. Takes the address as a `usize` for ease of test writing, panics here if it's not
        /// valid.
        fn new(order: usize, address: usize) -> Block {
            Block { order, address: PAddr::new(address).unwrap() }
        }
    }

    /// Helper to check the content of a `BuddyAllocator`'s bins.
    fn check_bins(mut allocator: BuddyAllocator, expected_blocks: Vec<Block>) {
        /*
         * First, try to remove all the expected blocks from the correct bins. Panic if a block isn't there.
         */
        for block in expected_blocks {
            if !allocator.bins[block.order].remove(&block.address) {
                panic!("Allocator does not have block of order {} starting at {:#x}", block.order, block.address);
            }
        }

        /*
         * Next, assert that all the bins are empty.
         */
        for i in 0..NUM_BINS {
            if !allocator.bins[i].is_empty() {
                panic!("Bin of order {} is not empty", i);
            }
        }
    }

    /// Helper to construct a frame range. Takes the address as a `usize` for ease of test writing - panics here if
    /// it's not valid.
    fn n_frames_at(start: usize, n: usize) -> Range<Frame> {
        Frame::starts_with(PAddr::new(start).unwrap())
            ..Frame::starts_with(PAddr::new(start + n * Size4KiB::SIZE).unwrap())
    }

    #[test]
    fn test_single_frame_binning() {
        let mut allocator = BuddyAllocator::new();
        allocator.free_range(n_frames_at(0x0, 1));
        allocator.free_range(n_frames_at(0x2000, 1));
        allocator.free_range(n_frames_at(0x16000, 1));
        allocator.free_range(n_frames_at(0xf480000, 1));
        assert_eq!(allocator.available_bytes(), 0x4000);
        check_bins(
            allocator,
            vec![Block::new(0, 0x0), Block::new(0, 0x2000), Block::new(0, 0x16000), Block::new(0, 0xf480000)],
        );
    }

    #[test]
    fn test_bigger_block_binning() {
        let mut allocator = BuddyAllocator::new();
        allocator.free_range(n_frames_at(0x2000, 1));
        allocator.free_range(n_frames_at(0x6000, 4));
        allocator.free_range(n_frames_at(0x10000, 64));
        assert_eq!(allocator.available_bytes(), (1 + 4 + 64) * BASE_SIZE);
        check_bins(
            allocator,
            vec![
                Block::new(0, 0x2000),
                Block::new(1, 0x6000),
                Block::new(1, 0x8000),
                Block::new(4, 0x10000),
                Block::new(4, 0x40000),
                Block::new(5, 0x20000),
            ],
        );
    }

    /// Test the splitting of weird-sized ranges into blocks.
    #[test]
    fn test_complex_range_binning() {
        /*
         * Split 3 frames into an order-1 block and an order-0 block.
         */
        let mut allocator = BuddyAllocator::new();
        allocator.free_range(n_frames_at(0x0, 3));
        check_bins(allocator, vec![Block::new(1, 0x0), Block::new(0, 0x2000)]);

        /*
         * Split 523 frames.
         */
        let mut allocator = BuddyAllocator::new();
        allocator.free_range(n_frames_at(0x40000, 523));
        assert_eq!(allocator.available_bytes(), 523 * BASE_SIZE);
        check_bins(
            allocator,
            vec![
                Block::new(8, 0x100000),
                Block::new(7, 0x80000),
                Block::new(6, 0x200000),
                Block::new(6, 0x40000),
                Block::new(3, 0x240000),
                Block::new(1, 0x248000),
                Block::new(0, 0x24a000),
            ],
        );
    }

    #[test]
    fn test_block_coalescing() {
        /*
         * Test the coalescing of two order-0 blocks into a single order-1 block, with a neighbour that can't
         * be.
         */
        let mut allocator = BuddyAllocator::new();
        allocator.free_range(n_frames_at(0x1000, 1));
        allocator.free_range(n_frames_at(0x3000, 1));
        allocator.free_range(n_frames_at(0x2000, 1));
        assert_eq!(allocator.available_bytes(), 0x3000);
        check_bins(allocator, vec![Block::new(0, 0x1000), Block::new(1, 0x2000)]);

        /*
         * Start with four order-0 blocks that can be coalesced into a single order-2 block.
         */
        let mut allocator = BuddyAllocator::new();
        allocator.free_range(n_frames_at(0x0, 1));
        allocator.free_range(n_frames_at(0x2000, 1));
        allocator.free_range(n_frames_at(0x3000, 1));
        allocator.free_range(n_frames_at(0x1000, 1));
        assert_eq!(allocator.available_bytes(), 0x4000);
        check_bins(allocator, vec![Block::new(2, 0x0)]);

        /*
         * Add 1024 single frames, which should be coalesced into a single order-10 block.
         */
        let mut allocator = BuddyAllocator::new();
        for i in 0..1024 {
            allocator.free_range(n_frames_at(i * Size4KiB::SIZE, 1));
        }
        assert_eq!(allocator.available_bytes(), 0x400000);
        check_bins(allocator, vec![Block::new(10, 0x0)]);
    }

    #[test]
    fn test_empty_allocator() {
        let mut allocator = BuddyAllocator::new();
        assert_eq!(allocator.available_bytes(), 0);

        assert_eq!(allocator.alloc(1), None);
        assert_eq!(allocator.allocate_block(0), None);
        assert_eq!(allocator.allocate_block(MAX_ORDER), None);
    }

    #[test]
    fn test_block_larger_than_max_order() {
        /*
         * Currently, if we try and allocate a block greater than the current maximum block size, we return
         * `None` even if we could service the request overall.
         */
        let mut allocator = BuddyAllocator::new();
        allocator.free_range(n_frames_at(0x0, 8192)); // Allocate 4 blocks of the maximum order (currently 12)
        assert_eq!(allocator.allocate_block(13), None);
    }

    #[test]
    fn test_allocation() {
        let mut allocator = BuddyAllocator::new();
        allocator.free_range(n_frames_at(0x2000, 1));
        allocator.free_range(n_frames_at(0x6000, 4));
        allocator.free_range(n_frames_at(0x10000, 64));
        assert_eq!(allocator.available_bytes(), (1 + 4 + 64) * BASE_SIZE);
        check_bins(
            allocator.clone(),
            vec![
                Block::new(0, 0x2000),
                Block::new(1, 0x6000),
                Block::new(1, 0x8000),
                Block::new(4, 0x10000),
                Block::new(4, 0x40000),
                Block::new(5, 0x20000),
            ],
        );

        // Allocate 2 frames - should come from 0x6000
        assert_eq!(allocator.alloc(2), Some(PAddr::new(0x6000).unwrap()));

        // Allocate 1 frame - should come from 0x2000
        assert_eq!(allocator.alloc(1), Some(PAddr::new(0x2000).unwrap()));

        // Allocate another frame - this should force a larger block to split
        assert_eq!(allocator.alloc(1), Some(PAddr::new(0x8000).unwrap()));
    }
}
