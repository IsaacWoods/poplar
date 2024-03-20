use super::{KernelObject, KernelObjectId, KernelObjectType};
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug)]
pub struct Event {
    pub id: KernelObjectId,
    pub signalled: AtomicBool,
}

impl Event {
    pub fn new() -> Arc<Event> {
        Arc::new(Event { id: super::alloc_kernel_object_id(), signalled: AtomicBool::new(false) })
    }

    pub fn signal(&self) {
        // TODO: ordering?
        self.signalled.store(true, Ordering::SeqCst);
    }

    pub fn clear(&self) {
        // TODO: ordering?
        self.signalled.store(false, Ordering::SeqCst);
    }
}

impl KernelObject for Event {
    fn id(&self) -> KernelObjectId {
        self.id
    }

    fn typ(&self) -> KernelObjectType {
        KernelObjectType::Event
    }
}
