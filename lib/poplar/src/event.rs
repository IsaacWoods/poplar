use crate::Handle;

pub struct Event(Handle);

impl Event {
    pub fn new_from_handle(handle: Handle) -> Event {
        Event(handle)
    }

    pub fn wait_for_event_blocking(&self) {
        crate::syscall::wait_for_event(self.0).unwrap();
    }
}
