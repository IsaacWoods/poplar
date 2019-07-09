//! One of the allocators we use to manage physical memory is the buddy allocator. With this
//! allocator, memory is broken up into a number of blocks, each of which is a power-of-2 in size.
//! The allocator maintains a set of bins, each with an order `n`, where each bin contains blocks
//! blocks of size `2^n`. When an allocation is requested, and a block of the correct size is not
//! available to fulfil that allocation, a larger block is split into two *buddy* blocks of half
//! the size, one of which is used to satisfy the allocation, or is split recursively until it's
//! the correct size. When a block is freed, the buddy is queried, and if it's free, the blocks are
//! coalesced again into a larger block.
//!
//! TODO: talk about the advantages of this allocator and how the allocator scheme could be
//! improved in the future

use crate::util::math::{ceiling_log2, flooring_log2};
use alloc::{collections::BTreeSet, vec::Vec};
use core::{cmp::min, ops::Range};
use x86_64::memory::{Frame, PhysicalAddress};

pub struct BuddyAllocator {
    /// The bins of free blocks, where bin `i` contains blocks of size `2^i`. Uses `BTreeSet` to
    /// store the blocks in each bin, for efficient buddy location. The `Frame` stored for each
    /// block is the first frame in that block - the end frame can be calculated from the start
    /// frame and the order of the bin the block is in.
    // TODO: when generic constants are a thing, we might be able to use `[BTreeSet; N]` here.
    bins: Vec<BTreeSet<Frame>>,
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
            let order = min(
                self.max_order(),
                flooring_log2((block_start..range.end).count() as u64) as usize,
            );

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
        Frame::contains(PhysicalAddress::new(usize::from(x.start_address) ^ (1 << order)).unwrap())
    }

    /// Get the order of the largest block this allocator can track.
    fn max_order(&self) -> usize {
        self.bins.len() - 1
    }
}
