use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicU8, Ordering},
};

/// A guard for when you want to store some data in a static, and explicitly initialize it at some point. This
/// is different from `spin::Once` in that you do not need to provide an initialization method every time you
/// access the object (it also uses `MaybeUninit` instead of `Option`). This will only release shared
/// references to the data inside, so if you want to mutate it from mutable threads, you'll need to use a type
/// like `Mutex` or `RwLock` within this.
pub struct InitGuard<T> {
    /// The actual data. We can only assume that this holds initialized data if the state is
    /// `STATE_INITIALIZED`.
    data: UnsafeCell<MaybeUninit<T>>,
    state: AtomicU8,
}

unsafe impl<T: Send + Sync> Sync for InitGuard<T> {}
unsafe impl<T: Send> Send for InitGuard<T> {}

const STATE_UNINIT: u8 = 0;
const STATE_INITIALIZING: u8 = 1;
const STATE_INITIALIZED: u8 = 2;

impl<T> InitGuard<T> {
    pub const fn uninit() -> InitGuard<T> {
        InitGuard { data: UnsafeCell::new(MaybeUninit::uninit()), state: AtomicU8::new(STATE_UNINIT) }
    }

    /// Initialize this `InitGuard`, allowing it to be read from in the future.
    ///
    /// ### Panics
    /// Panics if this `InitGuard` has already been initialized.
    pub fn initialize(&self, value: T) {
        match self.state.compare_exchange(STATE_UNINIT, STATE_INITIALIZING, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(STATE_UNINIT) => {
                unsafe {
                    /*
                     * We make sure to initialize the entire data before marking ourselves as
                     * initialized. If we read from the data before doing this, we cause UB.
                     */
                    (*self.data.get()).as_mut_ptr().write(value);
                }
                self.state.store(STATE_INITIALIZED, Ordering::SeqCst);
            }

            Err(STATE_INITIALIZING) | Err(STATE_INITIALIZED) => panic!("InitGuard has already been initialized!"),
            _ => panic!("InitGuard has invalid state"),
        }
    }

    /// Get a reference to the data, if this guard has been initialized.
    ///
    /// ### Panics
    /// Panics if this guard hasn't been initialized yet. Use `try_get` if you want a fallible
    /// variant.
    pub fn get(&self) -> &T {
        match self.state.load(Ordering::SeqCst) {
            /*
             * Here, we create a reference to the data within the `MaybeUninit`. This causes UB if
             * the data isn't really initialized.
             */
            STATE_INITIALIZED => unsafe { (*self.data.get()).assume_init_ref() },
            STATE_UNINIT | STATE_INITIALIZING => panic!("InitGuard has not been initialized!"),
            _ => panic!("InitGuard has invalid state"),
        }
    }

    /// Get a mutable reference to the data, if this guard has been initialized.
    ///
    /// ### Panics
    /// Panics if this guard hasn't been initialized yet. Use `try_get_mut` if you want a fallible
    /// variant.
    pub fn get_mut(&mut self) -> &mut T {
        match self.state.load(Ordering::SeqCst) {
            /*
             * Here, we create a reference to the data within the `MaybeUninit`. This causes UB if
             * the data isn't really initialized.
             */
            STATE_INITIALIZED => unsafe { (*self.data.get()).assume_init_mut() },
            STATE_UNINIT | STATE_INITIALIZING => panic!("InitGuard has not been initialized!"),
            _ => panic!("InitGuard has invalid state"),
        }
    }

    /// Get a reference to the data, if this guard has been initialized. Returns `None` if it has
    /// not yet been initialized, or is currently being initialized.
    pub fn try_get(&self) -> Option<&T> {
        match self.state.load(Ordering::SeqCst) {
            /*
             * Here, we create a reference to the data within the `MaybeUninit`. This causes UB if
             * the data isn't really initialized.
             */
            STATE_INITIALIZED => Some(unsafe { (*self.data.get()).assume_init_ref() }),
            STATE_UNINIT | STATE_INITIALIZING => None,
            _ => panic!("InitGuard has invalid state"),
        }
    }

    /// Get a mutable reference to the data, if this guard has been initialized. Returns `None` if it has
    /// not yet been initialized, or is currently being initialized.
    pub fn try_get_mut(&mut self) -> Option<&mut T> {
        match self.state.load(Ordering::SeqCst) {
            /*
             * Here, we create a reference to the data within the `MaybeUninit`. This causes UB if
             * the data isn't really initialized.
             */
            STATE_INITIALIZED => Some(unsafe { (*self.data.get()).assume_init_mut() }),
            STATE_UNINIT | STATE_INITIALIZING => None,
            _ => panic!("InitGuard has invalid state"),
        }
    }
}
