use alloc::collections::btree_set::BTreeSet;
use core::cmp::min;
use hal::{
    bootinfo::MemoryEntry,
    mem::{PAddr, PageTable, PageTableAllocator},
    sync::Spinlock,
};

pub static PMM: Pmm = Pmm(Spinlock::new(BuddyAllocator::new()));

pub struct Pmm(Spinlock<BuddyAllocator>);

impl Pmm {
    pub const BASE_SIZE: usize = BuddyAllocator::BASE_SIZE;

    pub fn initialize(&self, memory_map: &[MemoryEntry]) {
        use hal::bootinfo::MemoryType;

        let mut buddy = self.0.lock();
        for entry in memory_map {
            if entry.typ == MemoryType::Usable {
                buddy.free_arbitrary(
                    PAddr::new(entry.base as usize),
                    entry.length as usize / Self::BASE_SIZE,
                );
            }
        }
    }

    /// Allocate `count * Self::BASE_SIZE` bytes of physical memory, if possible.
    pub fn allocate(&self, count: usize) -> Option<PAddr> {
        self.0.lock().allocate(count.next_power_of_two())
    }

    /// Free `count * Self::BASE_SIZE` bytes starting at `start`. This block must have been
    /// previously allocated by this allocator.
    pub fn free(&self, start: PAddr, count: usize) {
        self.0.lock().free_aligned(start, count.next_power_of_two());
    }
}

impl PageTableAllocator for Pmm {
    fn alloc(&self) -> PAddr {
        self.allocate(1).unwrap()
    }

    fn free(&self, frame: PAddr) {
        self.free(frame, 1)
    }
}

/// A buddy allocator that manages physical memory by maintaining 'bins' of blocks, each of which is
/// a power-of-2 number of frames.
///
/// The blocks in each bin are arranged in pairs - each has a "buddy" where A's buddy is B and B's
/// buddy is A. To find the address of a block's buddy, the bit corresponding to the block's order
/// is simply flipped (and so can easily be calculated with a single XOR - see
/// [`BuddyAllocator::buddy_of`]). This allows blocks to be split and coalesced easily and
/// efficiently.
///
/// Buddy alloctors provide efficient management of physical memory, and are widely used in kernels.
/// Their main disadvantage to be aware of is internal fragmentation - requests for poorly-sized
/// blocks can waste significant memory.
///
///```ignore
///       16                               0       Order       Size of blocks (in frames)
///        |-------------------------------|
///        |               8               |       4           2^^4 = 16
///        |---------------|---------------|
///        |      12       |       4       |       3           2^^3 = 8
///        |-------|-------|-------|-------|
///        |  14   |  10   |   6   |   2   |       2           2^^2 = 4
///        |---|---|---|---|---|---|---|---|
///        |   |   |   |   |   |   |   |   |       1           2^^1 = 2
///        |-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|
///        | | | | | | | | | | | | | | | | |       0           2^^0 = 1
///        |-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|
/// ```
#[derive(Clone, Debug)]
pub struct BuddyAllocator {
    /// The bins of free blocks, where bin `i` contains blocks of size `Self::BASE_SIZE * (2^i)`.
    /// Uses a [`BTreeSet`] for efficient buddy lookups. Each block is stored as the physical
    /// address of the start of the block.
    bins: [BTreeSet<PAddr>; Self::NUM_BINS],
}

impl BuddyAllocator {
    /// The smallest block size tracked by the allocator.
    pub const BASE_SIZE: usize = PageTable::PAGE_SIZE_4KIB;
    /// The largest block stored by the allocator is `2^MAX_ORDER`.
    pub const MAX_ORDER: usize = 12;
    pub const NUM_BINS: usize = Self::MAX_ORDER + 1;

    pub const fn new() -> BuddyAllocator {
        BuddyAllocator {
            bins: [const { BTreeSet::new() }; Self::NUM_BINS],
        }
    }

    /// Allocate a block of `count * Self::BASE_SIZE`, if possible. `count` must be a power-of-2.
    pub fn allocate(&mut self, count: usize) -> Option<PAddr> {
        assert!(count.is_power_of_two());
        let order = count.trailing_zeros() as usize;
        self.allocate_block(order)
    }

    pub fn allocate_block(&mut self, order: usize) -> Option<PAddr> {
        if order > Self::MAX_ORDER {
            return None;
        }

        if let Some(block) = self.bins[order].pop_first() {
            return Some(block);
        }

        if let Some(block) = self.allocate_block(order + 1) {
            let second_half = Self::buddy_of(block, order);
            self.free_block(second_half, order);
            Some(block)
        } else {
            None
        }
    }

    /// Free a block of addresses, starting at `start` and of size `count * Size::BASE_SIZE`.
    /// Multiple blocks may be created, as needed by size and alignment constraints.
    pub fn free_arbitrary(&mut self, mut start: PAddr, mut count: usize) {
        while count > 0 {
            let max_order_by_size = count.trailing_zeros() as usize;
            let max_order_by_alignment = if usize::from(start) == 0 {
                Self::MAX_ORDER
            } else {
                (usize::from(start) / Self::BASE_SIZE).trailing_zeros() as usize
            };
            let next_block_order = min(
                Self::MAX_ORDER,
                min(max_order_by_size, max_order_by_alignment),
            );
            self.free_block(start, next_block_order);
            start += (1 << next_block_order) * Self::BASE_SIZE;
            count -= 1 << next_block_order;
        }
    }

    /// Free a block of `count * Self::BASE_SIZE` starting at `start`. `count` must be a power-of-2,
    /// and `start` must be well-aligned to the start of a block of the order defined by size.
    pub fn free_aligned(&mut self, start: PAddr, count: usize) {
        assert!(count.is_power_of_two());
        // TODO: assert order is correct given start and count

        let order = count.trailing_zeros() as usize;
        self.free_block(start, order);
    }

    /// Free a block of order `order` starting at `start`. If the block is less than the maximum
    /// order and its buddy is also free, recursively coalesce them into a block of the next order
    /// up.
    pub fn free_block(&mut self, start: PAddr, order: usize) {
        let buddy = Self::buddy_of(start, order);

        if order < Self::MAX_ORDER && self.bins[order].remove(&buddy) {
            self.free_block(min(start, buddy), order + 1);
        } else {
            assert!(!self.bins[order].contains(&start));
            self.bins[order].insert(start);
        }
    }

    fn buddy_of(start: PAddr, order: usize) -> PAddr {
        PAddr::new(usize::from(start) ^ ((1 << order) * Self::BASE_SIZE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused)]
    use alloc::vec;
    use alloc::vec::Vec;

    #[test]
    fn test_buddy_of() {
        macro_rules! test {
            ($order: expr, $first: expr, $second: expr) => {
                assert_eq!(
                    BuddyAllocator::buddy_of(PAddr::new($first), $order),
                    PAddr::new($second)
                );
            };
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
        fn new(order: usize, address: usize) -> Block {
            Block {
                order,
                address: PAddr::new(address),
            }
        }
    }

    /// Helper to check the content of a `BuddyAllocator`'s bins.
    fn check_bins(mut allocator: BuddyAllocator, expected_blocks: Vec<Block>) {
        /*
         * First, try to remove all the expected blocks from the correct bins. Panic if a block isn't there.
         */
        for block in expected_blocks {
            if !allocator.bins[block.order].remove(&block.address) {
                panic!(
                    "Allocator does not have block of order {} starting at {:#x}",
                    block.order, block.address
                );
            }
        }

        /*
         * Next, assert that all the bins are empty.
         */
        for i in 0..BuddyAllocator::NUM_BINS {
            if !allocator.bins[i].is_empty() {
                panic!("Bin of order {} is not empty", i);
            }
        }
    }

    #[test]
    fn test_single_frame_binning() {
        let mut allocator = BuddyAllocator::new();
        allocator.free_arbitrary(PAddr::new(0x0), 1);
        allocator.free_arbitrary(PAddr::new(0x2000), 1);
        allocator.free_arbitrary(PAddr::new(0x16000), 1);
        allocator.free_arbitrary(PAddr::new(0xf480000), 1);
        check_bins(
            allocator,
            vec![
                Block::new(0, 0x0),
                Block::new(0, 0x2000),
                Block::new(0, 0x16000),
                Block::new(0, 0xf480000),
            ],
        );
    }

    #[test]
    fn test_bigger_block_binning() {
        let mut allocator = BuddyAllocator::new();
        allocator.free_arbitrary(PAddr::new(0x2000), 1);
        allocator.free_arbitrary(PAddr::new(0x6000), 4);
        allocator.free_arbitrary(PAddr::new(0x10000), 64);
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
        allocator.free_arbitrary(PAddr::new(0x0), 3);
        check_bins(allocator, vec![Block::new(1, 0x0), Block::new(0, 0x2000)]);

        /*
         * Split 523 frames.
         */
        let mut allocator = BuddyAllocator::new();
        allocator.free_arbitrary(PAddr::new(0x40000), 523);
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
        allocator.free_arbitrary(PAddr::new(0x1000), 1);
        allocator.free_arbitrary(PAddr::new(0x3000), 1);
        allocator.free_arbitrary(PAddr::new(0x2000), 1);
        check_bins(
            allocator,
            vec![Block::new(0, 0x1000), Block::new(1, 0x2000)],
        );

        /*
         * Start with four order-0 blocks that can be coalesced into a single order-2 block.
         */
        let mut allocator = BuddyAllocator::new();
        allocator.free_arbitrary(PAddr::new(0x0), 1);
        allocator.free_arbitrary(PAddr::new(0x2000), 1);
        allocator.free_arbitrary(PAddr::new(0x3000), 1);
        allocator.free_arbitrary(PAddr::new(0x1000), 1);
        check_bins(allocator, vec![Block::new(2, 0x0)]);

        /*
         * Add 1024 single frames, which should be coalesced into a single order-10 block.
         */
        let mut allocator = BuddyAllocator::new();
        for i in 0..1024 {
            allocator.free_arbitrary(PAddr::new(i * BuddyAllocator::BASE_SIZE), 1);
        }
        check_bins(allocator, vec![Block::new(10, 0x0)]);
    }

    #[test]
    fn test_empty_allocator() {
        let mut allocator = BuddyAllocator::new();

        assert_eq!(allocator.allocate(1), None);
        assert_eq!(allocator.allocate_block(0), None);
        assert_eq!(allocator.allocate_block(BuddyAllocator::MAX_ORDER), None);
    }

    #[test]
    fn test_block_larger_than_max_order() {
        /*
         * Currently, if we try and allocate a block greater than the current maximum block size, we return
         * `None` even if we could service the request overall.
         */
        let mut allocator = BuddyAllocator::new();
        allocator.free_arbitrary(PAddr::new(0x0), 8192); // Allocate 4 blocks of the maximum order (currently 12)
        assert_eq!(allocator.allocate_block(13), None);
    }

    #[test]
    fn test_allocation() {
        let mut allocator = BuddyAllocator::new();
        allocator.free_arbitrary(PAddr::new(0x2000), 1);
        allocator.free_arbitrary(PAddr::new(0x6000), 4);
        allocator.free_arbitrary(PAddr::new(0x10000), 64);
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
        assert_eq!(allocator.allocate(2), Some(PAddr::new(0x6000)));

        // Allocate 1 frame - should come from 0x2000
        assert_eq!(allocator.allocate(1), Some(PAddr::new(0x2000)));

        // Allocate another frame - this should force a larger block to split
        assert_eq!(allocator.allocate(1), Some(PAddr::new(0x8000)));
    }
}
