use super::{KernelObject, KernelObjectId, KernelObjectType};
use crate::Platform;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug)]
pub struct Interrupt {
    pub id: KernelObjectId,
    pub triggered: AtomicBool,

    /// The vector that this interrupt is triggered by. The value of this is determined by the
    /// platform-specific interrupt layer, and is effectively opaque to the common kernel.
    pub rearm_irq: Option<usize>,
}

impl Interrupt {
    pub fn new(rearm_irq: Option<usize>) -> Arc<Interrupt> {
        Arc::new(Interrupt { id: super::alloc_kernel_object_id(), triggered: AtomicBool::new(false), rearm_irq })
    }

    pub fn trigger(&self) {
        // TODO: ordering?
        self.triggered.store(true, Ordering::SeqCst);
    }

    pub fn rearm<P>(&self)
    where
        P: Platform,
    {
        // TODO: ordering?
        self.triggered.store(false, Ordering::SeqCst);

        if let Some(irq) = self.rearm_irq {
            P::rearm_interrupt(irq);
        }
    }
}

impl KernelObject for Interrupt {
    fn id(&self) -> KernelObjectId {
        self.id
    }

    fn typ(&self) -> KernelObjectType {
        KernelObjectType::Interrupt
    }
}
