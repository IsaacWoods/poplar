use super::{memory::userspace_map, Arch};
use crate::{object::WrappedKernelObject, util::bitmap::Bitmap};
use alloc::vec::Vec;
use x86_64::memory::{kernel_map, EntryFlags, Frame, FrameAllocator, Page, PageTable, VirtualAddress};

#[derive(PartialEq, Eq, Debug)]
enum State {
    NotActive,
    Active,
}

pub struct AddressSpace {
    table: PageTable,
    state: State,
    // TODO: at the moment, this only allows us to allocate 64 stacks. When const generics land,
    // implement Bitmap for [u64; N] and use one of those instead.
    /// Bitmap of allocated stacks in this address space. Each bit in this bitmap represents the
    /// corresponding stack slot for both the usermode and kernel stacks.
    stack_bitmap: u64,
    memory_objects: Vec<WrappedKernelObject<Arch>>,
}

pub struct StackSet {
    pub kernel_stack_top: VirtualAddress,
    pub user_stack_top: VirtualAddress,
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

        AddressSpace { table, state: State::NotActive, stack_bitmap: 0, memory_objects: Vec::new() }
    }

    pub fn map_memory_object(&mut self, arch: &Arch, memory_object: WrappedKernelObject<Arch>) {
        let mut mapper = self.table.mapper();
        let memory_obj_info = memory_object.object.memory_object().expect("Not a Memory Object").read();

        let start_page = Page::starts_with(memory_obj_info.virtual_address);
        let pages = start_page..(start_page + memory_obj_info.num_pages);

        let start_frame = Frame::starts_with(memory_obj_info.physical_address);
        let frames = start_frame..(start_frame + memory_obj_info.num_pages);

        for (page, frame) in pages.zip(frames) {
            mapper.map_to(page, frame, memory_obj_info.flags, &arch.physical_memory_manager).unwrap();
        }
    }

    pub fn switch_to(&mut self) {
        assert_eq!(self.state, State::NotActive, "Tried to switch to already-active address space!");
        self.table.switch_to();
        self.state = State::Active;
    }

    pub fn add_stack_set<A>(&mut self, size: usize, allocator: &A) -> Option<StackSet>
    where
        A: FrameAllocator,
    {
        // Get a free stack slot. If there isn't one, we can't allocate any more stacks on the AddressSpace.
        let index = self.stack_bitmap.alloc(1)?;

        /*
         * Construct the addresses of the kernel and user stacks. We use `index + 1` because we
         * want the top of the stack, so we add an extra stack's worth.
         */
        let kernel_stack_top =
            userspace_map::KERNEL_STACKS_START + (index + 1) * userspace_map::MAX_STACK_SIZE;
        let user_stack_top = userspace_map::USER_STACKS_START + (index + 1) * userspace_map::MAX_STACK_SIZE;

        // Map the stacks
        let mut mapper = self.table.mapper();
        for page in Page::starts_with(kernel_stack_top - size)..Page::starts_with(kernel_stack_top) {
            mapper.map(page, EntryFlags::WRITABLE, allocator).unwrap();
        }
        for page in Page::starts_with(user_stack_top - size)..Page::starts_with(user_stack_top) {
            mapper.map(page, EntryFlags::WRITABLE | EntryFlags::USER_ACCESSIBLE, allocator).unwrap();
        }

        Some(StackSet { kernel_stack_top, user_stack_top })
    }
}
