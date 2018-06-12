mod frame_allocator;
pub mod map;
pub mod paging;
mod stack_allocator;

pub use self::frame_allocator::FrameAllocator;
pub use self::paging::{entry::EntryFlags, Page, PhysicalAddress, PhysicalMapping, VirtualAddress};

use self::map::{HEAP_SIZE, HEAP_START};
use self::paging::PAGE_SIZE;
use self::stack_allocator::{Stack, StackAllocator};
use alloc::BTreeMap;
use core::ops::Add;
use multiboot2::BootInformation;

extern "C" {
    /*
     * The ADDRESS of this symbol is the top of the kernel stack
     */
    static _kernel_stack_top: u8;
}

pub fn get_kernel_stack_top() -> VirtualAddress {
    VirtualAddress::new(unsafe { (&_kernel_stack_top) } as *const u8 as usize)
}

pub fn init(boot_info: &BootInformation) -> MemoryController {
    assert_first_call!("memory::init() should only be called once");
    let memory_map_tag = boot_info.memory_map().expect("Can't find memory map tag");

    /*
     */
    extern "C" {
        /*
         * The ADDRESSES of these are the start of the higher-half part of the kernel, and the end
         * of it, respectively. The symbols are defined in the linker script.
         */
        static _higher_start: u8;
        static _end: u8;
    }

    /*
     * We only want to map sections that appear in the higher-half, because we should never need
     * any of the bootstrap stuff again.
     */
    let kernel_start: VirtualAddress = unsafe { (&_higher_start as *const u8).into() };
    let kernel_end: VirtualAddress = unsafe { (&_end as *const u8).into() };
    trace!(
        "Loading kernel to: ({:#x})---({:#x})",
        kernel_start,
        kernel_end
    );

    let mut frame_allocator = FrameAllocator::new(
        boot_info.physical_start(),
        boot_info.physical_end(),
        PhysicalAddress::from_kernel_space(kernel_start),
        PhysicalAddress::from_kernel_space(kernel_end),
        memory_map_tag.memory_areas(),
    );

    /*
     * We can now replace the bootstrap paging structures with better ones that actually map the
     * structures with the correct permissions.
     */
    let mut active_table = paging::remap_kernel(boot_info, &mut frame_allocator);

    /*
     * Map the pages used by the heap, then create it
     */
    let heap_start_page = Page::containing_page(HEAP_START);
    let heap_end_page = Page::containing_page(HEAP_START.offset((HEAP_SIZE - 1) as isize));

    trace!(
        "Mapping heap pages in range: {:#x} to {:#x}",
        heap_start_page.start_address(),
        heap_end_page.start_address()
    );
    for page in Page::range_inclusive(heap_start_page, heap_end_page) {
        active_table.map(
            page,
            paging::entry::EntryFlags::WRITABLE,
            &mut frame_allocator,
        );
    }

    unsafe {
        ::kernel::ALLOCATOR
            .lock()
            .init(HEAP_START.into(), HEAP_SIZE);
    }

    /*
     * We can now map each module into the virtual address space
     */
    let mut loaded_modules = BTreeMap::new();
    for module_tag in boot_info.modules() {
        let physical_mapping = active_table.map_physical_region(
            module_tag.start_address(),
            module_tag.end_address(),
            EntryFlags::PRESENT,
            &mut frame_allocator,
        );
        loaded_modules.insert(module_tag.name(), physical_mapping);
    }
    info!("Loaded {} modules", loaded_modules.len());

    /*
     * Create a StackAllocator that allocates in the 100 pages directly following the heap
     */
    let stack_allocator = StackAllocator::new(map::STACK_SPACE_TOP, map::STACK_SPACE_BOTTOM);

    MemoryController {
        kernel_page_table: active_table,
        frame_allocator,
        stack_allocator,
        loaded_modules,
    }
}

pub struct FrameIter {
    start: Frame,
    end: Frame,
}

impl Iterator for FrameIter {
    type Item = Frame;

    fn next(&mut self) -> Option<Frame> {
        if self.start <= self.end {
            let frame = self.start;
            self.start.number += 1;
            Some(frame)
        } else {
            None
        }
    }
}

pub(self) const FRAME_SIZE: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Frame {
    number: usize,
}

impl Add<usize> for Frame {
    type Output = Frame;

    fn add(self, rhs: usize) -> Self {
        Frame {
            number: self.number + rhs,
        }
    }
}

impl Frame {
    pub fn containing_frame(address: PhysicalAddress) -> Frame {
        /*
         * A physical address must be smaller than 2^52 to be valid
         */
        debug_assert!(
            usize::from(address) < 2usize.pow(52),
            "{:#x} is not a valid physical address",
            address
        );

        Frame {
            number: usize::from(address) / FRAME_SIZE,
        }
    }

    pub fn start_address(&self) -> PhysicalAddress {
        (self.number * FRAME_SIZE).into()
    }

    pub fn end_address(&self) -> PhysicalAddress {
        self.start_address().offset((FRAME_SIZE - 1) as isize)
    }

    pub fn range_inclusive(start: Frame, end: Frame) -> FrameIter {
        FrameIter { start, end }
    }

    /// Calculate the number of frames needed to store a structure of size `size` bytes.
    pub fn needed_frames(size: usize) -> usize {
        (size / FRAME_SIZE) + if size % FRAME_SIZE > 0 {
            1 // Needs part of another frame, add an extra one
        } else {
            0 // Lies on a frame boundary, does not need extra frame
        }
    }
}

pub struct MemoryController {
    pub kernel_page_table: paging::ActivePageTable,
    pub frame_allocator: FrameAllocator,
    pub stack_allocator: StackAllocator,
    pub loaded_modules: BTreeMap<&'static str, PhysicalMapping<u8>>,
}

impl MemoryController {
    pub fn alloc_stack(&mut self, size_in_pages: usize) -> Option<Stack> {
        self.stack_allocator.alloc_stack(
            &mut self.kernel_page_table,
            &mut self.frame_allocator,
            size_in_pages,
        )
    }
}
