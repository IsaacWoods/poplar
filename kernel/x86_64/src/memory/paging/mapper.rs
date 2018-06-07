use alloc::heap::{GlobalAlloc, Layout};
use core::alloc::Opaque;
use core::ops::{Deref, Range};
use memory::paging::entry::EntryFlags;
use memory::paging::table::{self, Level4, Table};
use memory::paging::ENTRY_COUNT;
use memory::{Frame, FrameAllocator};
use memory::{Page, PhysicalAddress, VirtualAddress, PAGE_SIZE};
use tlb;

pub struct Mapper {
    pub p4: &'static mut Table<Level4>,
}

/// A region of physical memory mapped into the virtual address space, allocated on the heap.
/// Useful for when you want to just map some physical memory, and don't care where in the virtual
/// address space it ends up.
#[derive(Clone, Debug)]
pub struct PhysicalMapping<T> {
    pub start: PhysicalAddress,
    pub end: PhysicalAddress,

    pub start_page: Page,
    pub end_page: Page,
    pub region_ptr: *mut Opaque,
    pub layout: Layout,
    pub ptr: *mut T,
    pub size: usize,
}

impl<T> Deref for PhysicalMapping<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl Mapper {
    pub unsafe fn new() -> Mapper {
        Mapper {
            p4: &mut *table::P4,
        }
    }

    pub fn translate(&self, virtual_address: VirtualAddress) -> Option<PhysicalAddress> {
        let maybe_frame = self.translate_page(Page::containing_page(virtual_address));
        maybe_frame
            .map(|frame| (frame.number * PAGE_SIZE + virtual_address.offset_into_page()).into())
    }

    pub fn translate_page(&self, page: Page) -> Option<Frame> {
        let p3 = self.p4.next_table(page.p4_index());

        let huge_page = || {
            p3.and_then(|p3| {
                let p3_entry = &p3[page.p3_index()];
                // 1GiB page?
                if let Some(start_frame) = p3_entry.pointed_frame() {
                    if p3_entry.flags().contains(EntryFlags::HUGE_PAGE) {
                        assert!(start_frame.number % (ENTRY_COUNT * ENTRY_COUNT) == 0);
                        return Some(Frame {
                            number: start_frame.number
                                + usize::from(page.p2_index()) * ENTRY_COUNT
                                + usize::from(page.p1_index()),
                        });
                    }
                }

                if let Some(p2) = p3.next_table(page.p3_index()) {
                    let p2_entry = &p2[page.p2_index()];
                    // 2MiB page?
                    if let Some(start_frame) = p2_entry.pointed_frame() {
                        if p2_entry.flags().contains(EntryFlags::HUGE_PAGE) {
                            // address must be 2MiB aligned
                            assert!(start_frame.number % ENTRY_COUNT == 0);
                            return Some(Frame {
                                number: start_frame.number + usize::from(page.p1_index()),
                            });
                        }
                    }
                }
                None
            })
        };

        p3.and_then(|p3| p3.next_table(page.p3_index()))
            .and_then(|p2| p2.next_table(page.p2_index()))
            .and_then(|p1| p1[page.p1_index()].pointed_frame())
            .or_else(huge_page)
    }

    pub fn map(&mut self, page: Page, flags: EntryFlags, allocator: &mut FrameAllocator) {
        let frame = allocator.allocate_frame().expect("out of memory");
        self.map_to(page, frame, flags, allocator)
    }

    pub fn map_physical_region<T>(
        &mut self,
        start: PhysicalAddress,
        end: PhysicalAddress,
        flags: EntryFlags,
        allocator: &mut FrameAllocator,
    ) -> PhysicalMapping<T> {
        assert!(
            end > start,
            "End address must be higher in memory than start"
        );

        let start_frame = Frame::containing_frame(start);
        let end_frame = Frame::containing_frame(end);
        let region_size = usize::from(end_frame.end_address() - start_frame.start_address());

        let layout = Layout::from_size_align(region_size, PAGE_SIZE).unwrap();
        let ptr = unsafe { ::kernel::ALLOCATOR.alloc(layout) } as *mut T;
        assert!(
            !ptr.is_null(),
            "Failed to allocate memory for physical mapping"
        );
        let start_page = Page::containing_page(VirtualAddress::from(ptr));
        let end_page =
            Page::containing_page(VirtualAddress::from(ptr).offset((region_size - 1) as isize));

        for i in 0..(region_size / PAGE_SIZE + 1) {
            self.unmap(start_page + i, allocator);

            self.map_to(start_page + i, start_frame + i, flags, allocator);
        }

        PhysicalMapping {
            start,
            end,
            start_page,
            end_page,
            region_ptr: ptr as *mut Opaque,
            layout,
            ptr: VirtualAddress::from(ptr)
                .offset(usize::from(start - start_frame.start_address()) as isize)
                .mut_ptr(),
            size: usize::from(end - start),
        }
    }

    pub fn unmap_physical_region<T>(
        &mut self,
        region: PhysicalMapping<T>,
        allocator: &mut FrameAllocator,
    ) {
        unsafe {
            ::kernel::ALLOCATOR.dealloc(region.region_ptr as *mut Opaque, region.layout);
        }

        // TODO: We should remap this into the correct physical memory in the heap, otherwise we'll
        // page fault when we hit it again!
        for page in Page::range_inclusive(region.start_page, region.end_page) {
            self.unmap(page, allocator);
        }
    }

    pub fn unmap(&mut self, page: Page, allocator: &mut FrameAllocator) {
        assert!(self.translate(page.start_address()).is_some());

        let p1 = self
            .p4
            .next_table_mut(page.p4_index())
            .and_then(|p3| p3.next_table_mut(page.p3_index()))
            .and_then(|p2| p2.next_table_mut(page.p2_index()))
            .expect("we don't support huge pages");
        let frame = p1[page.p1_index()].pointed_frame().unwrap();
        p1[page.p1_index()].set_unused();

        tlb::invalidate_page(page.start_address());

        // TODO free p(1,2,3) table if it has become empty
        allocator.deallocate_frame(frame);
    }

    /*
     * This maps a range of physical addresses (aligned to their respective page boundaries) to the
     * same range of virtual addresses, offsetted by the KERNEL_VMA.
     *
     * If any of the pages in the range are already mapped: they are left alone (and so the range
     * is still effectively correly mapped) if the mapped physical address is the same, and the
     * requested flags are the same or less permissive, and we panic otherwise.
     *
     * NOTE: This behaviour is required for the page table creation in `remap_kernel`, as the
     * Multiboot structure can overlap with pages previously mapped for modules.
     * TODO: Is this still required if we don't map modules like that? And now that modules are
     * page-aligned?
     */
    pub fn identity_map_range(
        &mut self,
        range: Range<PhysicalAddress>,
        flags: EntryFlags,
        allocator: &mut FrameAllocator,
    ) {
        for frame in Frame::range_inclusive(
            Frame::containing_frame(range.start),
            Frame::containing_frame(range.end.offset(-1)),
        ) {
            let virtual_address = frame.start_address().in_kernel_space();
            let page = Page::containing_page(virtual_address);

            let user_accessible = flags.contains(EntryFlags::USER_ACCESSIBLE);
            let p3 = self
                .p4
                .next_table_create(page.p4_index(), user_accessible, allocator);
            let p2 = p3.next_table_create(page.p3_index(), user_accessible, allocator);
            let p1 = p2.next_table_create(page.p2_index(), user_accessible, allocator);

            if p1[page.p1_index()].is_unused()
                || (p1[page.p1_index()]
                    .flags()
                    .is_compatible(flags | EntryFlags::default()))
            {
                p1[page.p1_index()].set(frame, flags | EntryFlags::default());
                tlb::invalidate_page(page.start_address());
            } else {
                panic!("Tried to map a range in which a page is already mapped, but with more permissive flags: {:#x}->{:#x}", page.start_address(), frame.start_address());
            }
        }
    }

    /*
     * This maps a given page to a given frame, with the specified flags.
     */
    pub fn map_to(
        &mut self,
        page: Page,
        frame: Frame,
        flags: EntryFlags,
        allocator: &mut FrameAllocator,
    ) {
        /*
         * If the page to be mapped is user-accessible, all the previous paging structures must be
         * too, otherwise we'll still page-fault.
         */
        let user_accessible = flags.contains(EntryFlags::USER_ACCESSIBLE);
        let p3 = self
            .p4
            .next_table_create(page.p4_index(), user_accessible, allocator);
        let p2 = p3.next_table_create(page.p3_index(), user_accessible, allocator);
        let p1 = p2.next_table_create(page.p2_index(), user_accessible, allocator);

        assert!(
            p1[page.p1_index()].is_unused(),
            "Tried to map a page that has already been mapped: {:#x}",
            page.start_address()
        );
        p1[page.p1_index()].set(frame, flags | EntryFlags::default());
        tlb::invalidate_page(page.start_address());
    }
}
