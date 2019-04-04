use super::Arch;
use crate::{util::bitmap::Bitmap, x86_64::memory::physical::LockedPhysicalMemoryManager};
use core::mem;
use log::info;
use x86_64::memory::{
    kernel_map::KERNEL_P4_ENTRY,
    paging::{
        entry::EntryFlags,
        table::RecursiveMapping,
        ActivePageTable,
        FrameAllocator,
        InactivePageTable,
        Mapper,
        Page,
    },
    VirtualAddress,
};

enum TableState {
    /// An `AddressSpace` is put in the `Poisoned` state while we move it between real states
    /// (which involves doing stuff that can cause a fault). This makes sure we can detect when
    /// something went wrong when transistioning between states, and don't trust invalid address
    /// spaces.
    Poisoned,
    NotActive(InactivePageTable<RecursiveMapping>),
    Active(ActivePageTable<RecursiveMapping>),
}

/// Contains information about what is mapped in an `AddressSpace`. The methods on this type, given
/// the `Mapper` from the table, alter the mappings of this address space.
///
/// Outside of `AddressSpace`, the only safe way to work with this struct is to get a reference to
/// one from `AddressSpace::modify`, which makes sure the correct set of page tables are installed
/// to be modified.
#[derive(Clone)]
pub struct AddressSpaceState {
    // TODO: at the moment, this only allows us to allocate 64 stacks. When const generics land,
    // implement Bitmap for [u64; N] and use one of those instead.
    /// Bitmap of allocated stacks in this address space. Each bit in this bitmap represents the
    /// corresponding stack slot for both the usermode and kernel stacks.
    stack_bitmap: u64,
}

impl AddressSpaceState {
    pub fn add_stack<A>(
        &mut self,
        mapper: &mut Mapper<RecursiveMapping>,
        allocator: &A,
        size: usize,
    ) -> Option<StackSet>
    where
        A: FrameAllocator,
    {
        use super::memory::userspace_map::*;

        // Get a free stack slot. If there isn't one free, we can't allocate any more stacks.
        let index = self.stack_bitmap.alloc(1)?;

        /*
         * Construct the addresses of the kernel and user stacks. We use `index + 1` because we
         * want the top of the stack, so we add an extra stack's worth.
         */
        let kernel_stack_top = KERNEL_STACKS_START + (index + 1) * MAX_STACK_SIZE;
        let user_stack_top = USER_STACKS_START + (index + 1) * MAX_STACK_SIZE;

        // Map the stacks
        mapper.map_range(
            Page::contains(kernel_stack_top - size)..Page::contains(kernel_stack_top),
            EntryFlags::PRESENT | EntryFlags::WRITABLE,
            allocator,
        );
        mapper.map_range(
            Page::contains(user_stack_top - size)..Page::contains(user_stack_top),
            EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::USER_ACCESSIBLE,
            allocator,
        );

        Some(StackSet { kernel_stack_top, user_stack_top })
    }
}

pub struct AddressSpace {
    table: TableState,
    state: AddressSpaceState,
}

pub struct StackSet {
    pub kernel_stack_top: VirtualAddress,
    pub user_stack_top: VirtualAddress,
}

impl AddressSpace {
    pub fn from_page_table(
        arch: &Arch,
        mut page_table: InactivePageTable<RecursiveMapping>,
    ) -> AddressSpace {
        /*
         * Get the frame that backs the kernel's P3. This is safe to unwrap because we wouldn't
         * be able to fetch these instructions if the kernel wasn't mapped.
         */
        let kernel_p3_frame =
            arch.kernel_page_table.lock().p4[KERNEL_P4_ENTRY].pointed_frame().unwrap();

        arch.kernel_page_table.lock().with(
            &mut page_table,
            &arch.physical_memory_manager,
            |mapper, allocator| {
                /*
                 * We map the kernel into every address space by stealing the address of the
                 * kernel's P3, and putting it into the address space's P4.
                 */
                mapper.p4[KERNEL_P4_ENTRY]
                    .set(kernel_p3_frame, EntryFlags::PRESENT | EntryFlags::WRITABLE);
            },
        );

        AddressSpace {
            table: TableState::NotActive(page_table),
            state: AddressSpaceState { stack_bitmap: 0 },
        }
    }

    pub fn switch_to(&mut self) {
        self.table = match mem::replace(&mut self.table, TableState::Poisoned) {
            TableState::NotActive(inactive_table) => {
                /*
                 * The currently active table will always have a `RecursiveMapping` because we'll
                 * always be switching from either the kernel's or another `AddressSpace`'s
                 * tables.
                 */
                TableState::Active(unsafe { inactive_table.switch_to::<RecursiveMapping>().0 })
            }

            TableState::Active(_) => panic!("Tried to switch to already-active address space!"),
            TableState::Poisoned => panic!("Tried to switch to poisoned address space!"),
        };
    }

    pub fn modify<F, R>(&mut self, arch: &Arch, f: F) -> R
    where
        F: FnOnce(
            &mut Mapper<RecursiveMapping>,
            &LockedPhysicalMemoryManager, // TODO: it makes me sad we have to hardcode this type
            // &dyn FrameAllocator,
            &mut AddressSpaceState,
        ) -> R,
    {
        let mut state = self.state.clone();
        let result = match self.table {
            TableState::Active(ref mut page_table) => {
                // TODO
                unimplemented!();
            }

            TableState::NotActive(ref mut page_table) => arch.kernel_page_table.lock().with(
                page_table,
                &arch.physical_memory_manager,
                |mapper, allocator| f(mapper, allocator, &mut state),
            ),

            TableState::Poisoned => panic!("Tried to modify poisoned address space"),
        };
        self.state = state;

        result
    }
}
