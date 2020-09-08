use super::{alloc_kernel_object_id, KernelObject, KernelObjectId};
use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};
use libpebble::syscall::{GetMessageError, SendMessageError, CHANNEL_MAX_NUM_HANDLES};
use spin::Mutex;

pub struct ChannelEnd {
    pub id: KernelObjectId,
    pub owner: KernelObjectId,
    messages: Mutex<Vec<Message>>,
    /// The other end of the channel. If this is `None`, the channel's messages come from the kernel.
    other_end: Option<Weak<ChannelEnd>>,
}

impl ChannelEnd {
    pub fn new_channel(owner: KernelObjectId) -> (Arc<ChannelEnd>, Arc<ChannelEnd>) {
        let mut end_a = Arc::new(ChannelEnd {
            id: alloc_kernel_object_id(),
            owner,
            messages: Mutex::new(Vec::new()),
            other_end: Some(Weak::default()),
        });

        let end_b = Arc::new(ChannelEnd {
            id: alloc_kernel_object_id(),
            owner,
            messages: Mutex::new(Vec::new()),
            other_end: Some(Arc::downgrade(&end_a)),
        });

        // TODO: is there a nicer way of doing this?
        unsafe {
            Arc::get_mut_unchecked(&mut end_a).other_end = Some(Arc::downgrade(&end_b));
        }

        (end_a, end_b)
    }

    pub fn new_kernel_channel(owner: KernelObjectId) -> Arc<ChannelEnd> {
        Arc::new(ChannelEnd {
            id: alloc_kernel_object_id(),
            owner,
            messages: Mutex::new(Vec::new()),
            other_end: None,
        })
    }

    /// Send a message through this `ChannelEnd`, to be received by the other end. If this is a kernel channel, the
    /// message is discarded.
    pub fn send(&self, message: Message) -> Result<(), SendMessageError> {
        if let Some(ref other_end) = self.other_end {
            match other_end.upgrade() {
                Some(other_end) => {
                    other_end.messages.lock().push(message);
                    Ok(())
                }
                None => Err(SendMessageError::OtherEndDisconnected),
            }
        } else {
            Ok(())
        }
    }
}

impl KernelObject for ChannelEnd {
    fn id(&self) -> KernelObjectId {
        self.id
    }
}

pub struct Message {
    pub bytes: Vec<u8>,
    pub handle_objects: [Option<Arc<dyn KernelObject>>; CHANNEL_MAX_NUM_HANDLES],
}
