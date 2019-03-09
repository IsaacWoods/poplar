use core::mem;
use x86_64::memory::paging::{table::RecursiveMapping, ActivePageTable, InactivePageTable};

enum State {
    /// An `AddressSpace` is put in the `Poisoned` state while we move it between real states
    /// (which involves doing stuff that can cause a fault). This makes sure we can detect when
    /// something went wrong when transistioning between states, and don't trust invalid address
    /// spaces.
    Poisoned,
    NotActive(InactivePageTable<RecursiveMapping>),
    Active(ActivePageTable<RecursiveMapping>),
}

pub struct AddressSpace {
    state: State,
}

impl AddressSpace {
    pub fn from_page_table(page_table: InactivePageTable<RecursiveMapping>) -> AddressSpace {
        AddressSpace { state: State::NotActive(page_table) }
    }

    pub fn switch_to(&mut self) {
        self.state = match mem::replace(&mut self.state, State::Poisoned) {
            State::NotActive(inactive_table) => {
                /*
                 * The currently active table will always have a `RecursiveMapping` because we'll
                 * always be switching from either the kernel's or another `AddressSpace`'s
                 * tables.
                 */
                State::Active(unsafe { inactive_table.switch_to::<RecursiveMapping>().0 })
            }

            State::Active(_) => panic!("Tried to switch to already-active address space!"),
            State::Poisoned => panic!("Tried to switch to poisoned address space!"),
        };
    }
}
