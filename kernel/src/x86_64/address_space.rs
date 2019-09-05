use super::{memory::userspace_map, Arch, ARCH};
use crate::object::WrappedKernelObject;
use alloc::vec::Vec;
use pebble_util::bitmap::{Bitmap, BitmapArray};
use x86_64::memory::{kernel_map, EntryFlags, Frame, FrameAllocator, Page, PageTable, VirtualAddress};

#[derive(PartialEq, Eq, Debug)]
enum State {
    NotActive,
    Active,
}

pub struct AddressSpace {
    pub table: PageTable,
    state: State,
    memory_objects: Vec<WrappedKernelObject<Arch>>,

    /// Bitmap of allocated stacks in this address space. Each bit in this bitmap represents the
    /// corresponding stack slot for both the usermode and kernel stacks. Each address space can
    /// contain 64 tasks, so a `u64` is the perfect size.
    stack_bitmap: u64,
    /// This is the area of the kernel address space that this address space is free to allocate
    /// for use as kernel stacks for tasks. It is up to the address space to manage mappings in
    /// this area of virtual memory. Its use is tracked by `stack_bitmap` in the same way as user
    /// stacks are in the userspace address space.
    kernel_stack_area_base: VirtualAddress,
}

/// A pair of stacks - one for the kernel and one for userspace. Both addresses will start aligned
/// to 16 bytes.
pub struct StackSet {
    pub kernel_slot_top: VirtualAddress,
    pub kernel_slot_bottom: VirtualAddress,
    pub kernel_stack_bottom: VirtualAddress,

    pub user_slot_top: VirtualAddress,
    pub user_slot_bottom: VirtualAddress,
    pub user_stack_bottom: VirtualAddress,
}

impl AddressSpace {
    pub fn new(arch: &Arch) -> AddressSpace {
        let frame = arch.physical_memory_manager.allocate();
        let mut table = PageTable::new(frame, kernel_map::PHYSICAL_MAPPING_BASE);

        /*
         * Install a copy of the kernel's P3 in each address space. This means the kernel is
         * always mapped, so system calls and interrupts don't page-fault. It's always
         * safe to unwrap the kernel address - if it wasn't there, we wouldn't be able to
         * fetch these instructions.
         */
        let kernel_p3_address =
            arch.kernel_page_table.lock().mapper().p4[kernel_map::KERNEL_P4_ENTRY].address().unwrap();
        table.mapper().p4[kernel_map::KERNEL_P4_ENTRY].set(kernel_p3_address, EntryFlags::WRITABLE);

        let kernel_stack_slot = arch
            .kernel_stack_bitmap
            .lock()
            .alloc(1)
            .expect("Failed to allocate kernel stack slot for address space");

        AddressSpace {
            table,
            state: State::NotActive,
            memory_objects: Vec::new(),
            stack_bitmap: 0,
            kernel_stack_area_base: kernel_map::kernel_stack_area_base(kernel_stack_slot),
        }
    }

    // TODO: return a Result from here with success or failure
    pub fn map_memory_object(&mut self, memory_object: WrappedKernelObject<Arch>) {
        {
            let mut mapper = self.table.mapper();
            let memory_obj_info = memory_object.object.memory_object().expect("Not a Memory Object").read();

            let start_page = Page::starts_with(memory_obj_info.virtual_address);
            let pages = start_page..(start_page + memory_obj_info.num_pages);

            let start_frame = Frame::starts_with(memory_obj_info.physical_address);
            let frames = start_frame..(start_frame + memory_obj_info.num_pages);

            for (page, frame) in pages.zip(frames) {
                mapper
                    .map_to(page, frame, memory_obj_info.flags, &ARCH.get().physical_memory_manager)
                    .unwrap();
            }
        }

        self.memory_objects.push(memory_object);
    }

    pub fn switch_to(&mut self) {
        assert_eq!(self.state, State::NotActive, "Tried to switch to already-active address space!");
        self.table.switch_to();
        self.state = State::Active;
    }

    /// Tell the address space that we are switching to another address space, so it is no longer
    /// the active address space
    // TODO: do we even want to track this state?
    pub fn switch_away_from(&mut self) {
        assert_eq!(self.state, State::Active);
        self.state = State::NotActive;
    }

    pub fn add_stack_set<A>(&mut self, size: usize, allocator: &A) -> Option<StackSet>
    where
        A: FrameAllocator,
    {
        // TODO: atm, this is pretty inefficient in how it lays stacks out in memory (page wise). We
        // should probably do something about that.

        // Get a free stack slot. If there isn't one, we can't allocate any more stacks on the AddressSpace.
        let index = self.stack_bitmap.alloc(1)?;

        let kernel_slot_bottom = self.kernel_stack_area_base + index * kernel_map::STACK_SLOT_SIZE;
        let kernel_slot_top = kernel_slot_bottom + kernel_map::STACK_SLOT_SIZE - 1;
        let kernel_stack_bottom = kernel_slot_top - userspace_map::INITIAL_STACK_SIZE;

        let user_slot_bottom = userspace_map::USER_STACKS_START + index * kernel_map::STACK_SLOT_SIZE;
        let user_slot_top = user_slot_bottom + kernel_map::STACK_SLOT_SIZE - 1;
        let user_stack_bottom = user_slot_top - userspace_map::INITIAL_STACK_SIZE;

        // Map the stacks
        let mut mapper = self.table.mapper();
        for page in Page::contains(kernel_stack_bottom)..=Page::contains(kernel_slot_top) {
            mapper.map(page, EntryFlags::WRITABLE, allocator).unwrap();
        }
        for page in Page::contains(user_stack_bottom)..=Page::contains(user_slot_top) {
            mapper.map(page, EntryFlags::WRITABLE | EntryFlags::USER_ACCESSIBLE, allocator).unwrap();
        }

        Some(StackSet {
            kernel_slot_top,
            kernel_slot_bottom,
            kernel_stack_bottom,
            user_slot_top,
            user_slot_bottom,
            user_stack_bottom,
        })
    }
}
