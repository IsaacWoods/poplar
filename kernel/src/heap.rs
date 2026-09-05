//! This module implements the generic heap allocator for the kernel. It currently is a very basic
//! allocator based on a linked-list of free memory regions, with a header for each region with the
//! size of the free region and a link to the next hole. Allocation uses a first-fit strategy.
//!
//! Future advancements could use a better data-structure to track holes (e.g. a red-black tree to
//! enable best-fit allocation) and could use a sharded approach to enable per-CPU allocation
//! without a global lock. There may be better allocation strategies for small allocations, such as
//! separately maintaining slab-like regions for small bucket sizes. Separate slabs could be used
//! for common kernel data structures, as in other kernel designs.

use crate::bootinfo::{BootFrameAllocator, BootInfo};
use core::{
    alloc::{GlobalAlloc, Layout},
    mem,
    ptr::{self, NonNull},
};
use hal::{
    align_up,
    mem::{MemFlags, PageTable, VAddr},
    sync::Spinlock,
};
use tracing::debug;

#[cfg_attr(not(test), global_allocator)]
pub static ALLOCATOR: Allocator = Allocator::new();

pub fn bootstrap(kernel_page_table: &mut PageTable, boot_info: &mut BootInfo) {
    // TODO: should we put the heap after the kernel or at the base of the kernel dynamic area?
    let heap_start = VAddr::new(boot_info.header().kernel_free_start as usize);
    let early_frame_allocator = BootFrameAllocator::new(boot_info);
    const INITIAL_HEAP_SIZE: usize = hal::mem::kib(256);
    let initial_heap =
        early_frame_allocator.allocate(INITIAL_HEAP_SIZE / PageTable::PAGE_SIZE_4KIB);
    debug!(
        "Creating initial heap of {} bytes at {:#x}",
        INITIAL_HEAP_SIZE, heap_start
    );
    kernel_page_table
        .map(
            heap_start,
            initial_heap,
            INITIAL_HEAP_SIZE,
            MemFlags {
                writable: true,
                ..Default::default()
            },
            &early_frame_allocator,
        )
        .unwrap();
    unsafe {
        ALLOCATOR.init(heap_start.mut_ptr(), INITIAL_HEAP_SIZE);
    }
}

pub struct Allocator(Spinlock<Heap>);

impl Allocator {
    pub const fn new() -> Allocator {
        Allocator(Spinlock::new(Heap::new()))
    }

    pub unsafe fn init(&self, start: *mut u8, size: usize) {
        unsafe {
            self.0.lock().init(start, size);
        }
    }
}

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0.lock().allocate(layout).unwrap().as_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe {
            self.0.lock().free(NonNull::new(ptr).unwrap(), layout);
        }
    }
}

pub struct Heap {
    first: Hole,
}

unsafe impl Send for Heap {}

impl Heap {
    pub const fn new() -> Heap {
        Heap {
            first: Hole {
                size: 0,
                next: None,
            },
        }
    }

    /// Initialize the `Heap` with an initial hole of `size` bytes at `start`.
    ///
    /// ### Safety
    ///    - `start` must be well-aligned to `mem::align_of::<Hole>()`.
    ///    - size must be a multiple of `mem::size_of::<Hole>()`
    pub unsafe fn init(&mut self, start: *mut u8, size: usize) {
        unsafe {
            ptr::write(start as *mut Hole, Hole { size, next: None });
        }
        self.first.next = Some(NonNull::new(start as *mut Hole).unwrap());
    }

    /// Search the heap for a hole large enough to allocate the given `layout`. If one is found,
    /// modifies the heap to mark that region as unavailable, and returns a pointer to the
    /// allocated memory, as well as the layout that was actually allocated (this may differ from
    /// the passed layout as the allocator's internal metadata adds sizing and alignment
    /// requirements).
    pub fn allocate(&mut self, layout: Layout) -> Option<NonNull<u8>> {
        let layout = normalise_layout(layout);

        let mut hole = self.first.next;
        let mut prev = NonNull::new(&mut self.first as *mut Hole).unwrap();
        loop {
            let Some(some_hole) = hole else {
                return None;
            };
            match unsafe { self.try_allocate_from(some_hole, prev, layout) } {
                Some(addr) => {
                    return Some(addr);
                }
                None => {
                    prev = some_hole;
                    hole = unsafe { some_hole.as_ref().next };
                }
            }
        }
    }

    unsafe fn try_allocate_from(
        &mut self,
        hole: NonNull<Hole>,
        mut prev: NonNull<Hole>,
        layout: Layout,
    ) -> Option<NonNull<u8>> {
        let hole_size = unsafe { hole.as_ref().size };
        if hole_size < layout.size() {
            return None;
        }

        /*
         * We've established that the hole may be large enough before we do much maths, but we may
         * still not be able to accomodate the allocation once we consider alignment requirements.
         * To avoid leaking heap memory, we want to be able to fit new holes in the unused portions
         * of the hole.
         */
        let hole_addr = usize::from(hole.addr());
        let (aligned_addr, front_padding) = if align_up(hole_addr, layout.align()) > hole_addr {
            let aligned_addr = align_up(hole_addr + mem::size_of::<Hole>(), layout.align());
            (aligned_addr, Some(aligned_addr - hole_addr))
        } else {
            (hole_addr, None)
        };

        let allocation_end = aligned_addr + layout.size();
        if allocation_end > (hole_addr + hole_size) {
            return None;
        }

        let remaining_hole = hole_addr + unsafe { hole.as_ref().size } - allocation_end;

        let (back_padding_addr, back_padding) = if remaining_hole == 0 {
            (None, None)
        } else if remaining_hole < mem::size_of::<Hole>() {
            // We can't fit the allocation without leaking memory after it
            return None;
        } else {
            /*
             * We leave the unaligned sliver between the allocation and aligned hole - it will be
             * mopped up when we normalise the layout of the allocation when it's freed.
             */
            let back_padding_addr = align_up(allocation_end, mem::align_of::<Hole>());
            (
                Some(back_padding_addr),
                Some(remaining_hole - (back_padding_addr - allocation_end)),
            )
        };

        /*
         * We can make an allocation out of this hole. We now need to alter the linked list to
         * include the front and back padding (if any) and remove the allocation.
         */
        let original_next = unsafe { hole.as_ref().next };
        let mut new_prev = if let Some(front_padding) = front_padding {
            let hole_ptr = hole_addr as *mut Hole;
            unsafe {
                ptr::write(
                    hole_ptr,
                    Hole {
                        size: front_padding,
                        next: original_next,
                    },
                );
                prev.as_mut().next = Some(NonNull::new(hole_ptr).unwrap());
            }
            NonNull::new(hole_ptr).unwrap()
        } else {
            prev
        };
        if let Some(back_padding) = back_padding {
            let back_padding_ptr = back_padding_addr.unwrap() as *mut Hole;
            unsafe {
                ptr::write(
                    back_padding_ptr,
                    Hole {
                        size: back_padding,
                        next: original_next,
                    },
                );
                new_prev.as_mut().next = Some(NonNull::new(back_padding_ptr).unwrap());
            }
        }

        Some(NonNull::new(aligned_addr as *mut u8).unwrap())
    }

    /// Frees an allocation previously allocated from this heap. The `addr` passed must have
    /// originally come from the heap, and have been allocated with an identical layout to the one
    /// passed.
    pub unsafe fn free(&mut self, addr: NonNull<u8>, layout: Layout) {
        let layout = normalise_layout(layout);
        let mut addr = addr.cast::<Hole>();
        unsafe {
            ptr::write(
                addr.as_ptr(),
                Hole {
                    size: layout.size(),
                    next: None,
                },
            );
        }

        let mut prev = NonNull::new(&mut self.first as *mut Hole).unwrap();
        let Some(mut hole) = self.first.next else {
            // There are no real holes, so we can become the first
            self.first.next = Some(addr);
            return;
        };
        loop {
            if addr > hole {
                if let Some(next) = unsafe { hole.as_ref().next } {
                    prev = hole;
                    hole = next;
                    continue;
                } else {
                    // We've reached the end and we're still the highest address - add the new hole
                    unsafe {
                        hole.as_mut().next = Some(addr);
                        break;
                    }
                }
            }

            /*
             * We've found a hole at a higher address than ours. We need to stick the new hole in
             * the middle.
             */
            unsafe {
                prev.as_mut().next = Some(addr);
                addr.as_mut().next = Some(hole);
            }

            // See if we can merge any of the holes together
            unsafe {
                merge_if_able(prev, addr);
                merge_if_able(prev.as_ref().next.unwrap(), hole)
            }

            break;
        }
    }
}

pub struct Hole {
    pub size: usize,
    pub next: Option<NonNull<Hole>>,
}

/// Normalise a `Layout` for use with this allocator - ensuring the resulting allocation is large
/// enough for at least the hole metadata, and that it leaves the following hole well-aligned.
fn normalise_layout(layout: Layout) -> Layout {
    let size = align_up(
        usize::max(layout.size(), mem::size_of::<Hole>()),
        mem::align_of::<Hole>(),
    );
    Layout::from_size_align(size, layout.align()).unwrap()
}

unsafe fn merge_if_able(mut a: NonNull<Hole>, b: NonNull<Hole>) {
    if unsafe { a.as_ptr().byte_add(a.as_ref().size) == b.as_ptr() } {
        unsafe {
            a.as_mut().size += b.as_ref().size;
            a.as_mut().next = b.as_ref().next;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloc::vec;
    use std::println;

    #[allow(unused)]
    fn debug_holes(hole: NonNull<Hole>) {
        unsafe {
            println!(
                "First hole: {:?} of size {} --> {:?}",
                hole,
                hole.as_ref().size,
                hole.as_ref().next
            );

            let mut next = hole.as_ref().next;
            loop {
                let Some(next_some) = next else {
                    break;
                };
                println!(
                    "Hole: {:?} of size {} --> {:?}",
                    next_some,
                    next_some.as_ref().size,
                    next_some.as_ref().next
                );
                next = next_some.as_ref().next;
            }
        }
    }

    fn make_test_heap(size: usize, misalign: Option<usize>) -> Heap {
        let backing = vec![0u64; size / 8];
        let ptr = unsafe { backing.leak().as_ptr().byte_add(misalign.unwrap_or(0)) };

        let mut heap = Heap::new();
        println!("Base of heap at: {:?}", ptr);
        unsafe {
            heap.init(ptr as *mut u8, size);
        }
        heap
    }

    #[test]
    fn alloc_some_types() {
        let mut heap = make_test_heap(512, None);

        let a_layout = Layout::from_size_align(mem::size_of::<usize>(), 1).unwrap();
        let a = heap.allocate(a_layout).unwrap();

        let b_layout = Layout::from_size_align(mem::size_of::<usize>() * 8, 8).unwrap();
        let b = heap.allocate(b_layout).unwrap();

        let _b2 = heap.allocate(b_layout).unwrap();

        unsafe {
            heap.free(a, a_layout);
            heap.free(b, b_layout);
        }
    }

    #[test]
    fn oom() {
        let mut heap = make_test_heap(256, None);

        let a_layout = Layout::from_size_align(200, 1).unwrap();
        assert!(heap.allocate(a_layout).is_some());

        // We shouldn't be able to allocate another 200 bytes because not enough space
        assert!(heap.allocate(a_layout).is_none());

        // Enough space but insufficient alignment - should also fail
        let b_layout = Layout::from_size_align(40, 16).unwrap();
        assert!(heap.allocate(b_layout).is_none());
    }

    #[test]
    fn allocate_multiple_unaligned() {
        for offset in 0..=Layout::new::<Hole>().size() {
            let mut heap = make_test_heap(256, Some(offset));
            let base_size = size_of::<usize>();
            let base_align = align_of::<usize>();

            let layout_1 = Layout::from_size_align(base_size * 2, base_align).unwrap();
            let layout_2 = Layout::from_size_align(base_size * 7, base_align).unwrap();
            let layout_3 = Layout::from_size_align(base_size * 3, base_align * 4).unwrap();
            let layout_4 = Layout::from_size_align(base_size * 4, base_align).unwrap();

            let x = heap.allocate(layout_1.clone()).unwrap();
            let y = heap.allocate(layout_2.clone()).unwrap();
            assert_eq!(y.as_ptr() as usize, x.as_ptr() as usize + base_size * 2);
            let z = heap.allocate(layout_3.clone()).unwrap();
            assert_eq!(z.as_ptr() as usize % (base_size * 4), 0);

            unsafe {
                heap.free(x, layout_1.clone());
            }

            let a = heap.allocate(layout_4.clone()).unwrap();
            let b = heap.allocate(layout_1.clone()).unwrap();
            assert_eq!(b, x);

            unsafe {
                heap.free(y, layout_2);
                heap.free(z, layout_3);
                heap.free(a, layout_4);
                heap.free(b, layout_1);
            }
        }
    }
}
