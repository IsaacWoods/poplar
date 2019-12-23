use super::{BootServices, Status};
use bitflags::bitflags;
use core::mem;

#[derive(Debug)]
pub struct Event(());

unsafe impl Sync for Event {}

bitflags! {
    pub struct EventType: u32 {
        const TIMER = 0x8000_0000;
        const RUNTIME = 0x4000_0000;
        const NOTIFY_WAIT = 0x0000_0100;
        const NOTIFY_SIGNAL = 0x0000_0200;
        const SIGNAL_EXIT_BOOT_SERVICES = 0x0000_0201;
        const SIGNAL_VIRTUAL_ADDRESS_CHANGE = 0x6000_0202;
    }
}

#[repr(C)]
pub enum TimerDelay {
    Cancel,
    Periodic,
    Relative,
}

#[derive(Clone, Copy, Debug)]
#[repr(usize)]
pub enum TaskPriorityLevel {
    Application = 4,
    Callback = 8,
    Notify = 16,
    HightLevel = 31,
}

impl BootServices {
    pub fn create_event<T>(
        &self,
        event_type: EventType,
        notify_tpl: TaskPriorityLevel,
        notify_function: extern "win64" fn(&Event, &T),
        notify_context: &T,
    ) -> Result<&Event, Status>
    where
        T: ?Sized,
    {
        // It's safe to cast notify_function to a different signature as long as the UEFI system
        // upholds its side of the spec and passes notify_context unmodified
        let notify_function: extern "win64" fn(&Event, *const ()) = unsafe { mem::transmute(notify_function) };
        let notify_context = notify_context as *const T as *const ();

        let mut event = &Event(());
        (self._create_event)(event_type, notify_tpl, notify_function, notify_context, &mut event)
            .as_result()
            .map(|_| event)
    }

    pub fn close_event(&self, event: &Event) -> Result<(), Status> {
        (self._close_event)(event).as_result().map(|_| ())
    }

    pub fn signal_event(&self, event: &Event) -> Result<(), Status> {
        (self._signal_event)(event).as_result().map(|_| ())
    }

    pub fn wait_for_event_signalled(&self, events: &[&Event]) -> Result<usize, Status> {
        let mut index: usize = 0;
        (self._wait_for_event)(events.len(), events.as_ptr(), &mut index).as_result().map(|_| index)
    }

    pub fn is_event_signalled(&self, event: &Event) -> Result<(), Status> {
        (self._check_event)(event).as_result().map(|_| ())
    }

    pub fn set_timer(&self, event: &Event, timer_type: TimerDelay, trigger_time: u64) -> Result<(), Status> {
        (self._set_timer)(event, timer_type, trigger_time).as_result().map(|_| ())
    }

    pub fn raise_tpl(&self, new_tpl: TaskPriorityLevel) -> Result<(), Status> {
        (self._raise_tpl)(new_tpl).as_result().map(|_| ())
    }

    pub fn restore_tpl(&self, old_tpl: TaskPriorityLevel) -> Result<(), Status> {
        (self._restore_tpl)(old_tpl).as_result().map(|_| ())
    }
}
