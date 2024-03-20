use super::{alloc_kernel_object_id, KernelObject, KernelObjectId, KernelObjectType};
use alloc::{
    collections::VecDeque,
    fmt,
    sync::{Arc, Weak},
    vec::Vec,
};
use poplar::syscall::{GetMessageError, SendMessageError, CHANNEL_MAX_NUM_HANDLES};
use spinning_top::Spinlock;
use tracing::warn;

#[derive(Debug)]
pub struct ChannelEnd {
    pub id: KernelObjectId,
    pub owner: KernelObjectId,
    messages: Spinlock<VecDeque<Message>>,
    /// The other end of the channel. If this is `None`, the channel's messages come from the kernel.
    other_end: Option<Weak<ChannelEnd>>,
}

impl ChannelEnd {
    pub fn new_channel(owner: KernelObjectId) -> (Arc<ChannelEnd>, Arc<ChannelEnd>) {
        let mut end_a = Arc::new(ChannelEnd {
            id: alloc_kernel_object_id(),
            owner,
            messages: Spinlock::new(VecDeque::new()),
            other_end: Some(Weak::default()),
        });

        let end_b = Arc::new(ChannelEnd {
            id: alloc_kernel_object_id(),
            owner,
            messages: Spinlock::new(VecDeque::new()),
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
            messages: Spinlock::new(VecDeque::new()),
            other_end: None,
        })
    }

    /// Add a message *to* this `ChannelEnd`. Use `send` if you want to send a message *through* this
    /// `ChannelEnd` (i.e. to the other end of the Channel).
    pub fn add_message(&self, message: Message) {
        self.messages.lock().push_back(message);
    }

    /// Send a message through this `ChannelEnd`, to be received by the other end. If this is a kernel channel, the
    /// message is discarded.
    pub fn send(&self, message: Message) -> Result<(), SendMessageError> {
        if let Some(ref other_end) = self.other_end {
            match other_end.upgrade() {
                Some(other_end) => {
                    other_end.add_message(message);
                    Ok(())
                }
                None => Err(SendMessageError::OtherEndDisconnected),
            }
        } else {
            warn!("Discarding message sent down kernel channel");
            Ok(())
        }
    }

    /// Try to "receive" a message from this `ChannelEnd`, potentially removing it from the queue. Note that this
    /// keeps a lock over the message queue while the passed function is called - if the handling of the message
    /// fails (for example, the buffer to put it into is too small), the passed function can return it with
    /// `Err((message, some_error))`, and the message will be placed back into the queue (preserving message
    /// order), and the error will be returned.
    pub fn receive<F, R>(&self, f: F) -> Result<R, GetMessageError>
    where
        F: FnOnce(Message) -> Result<R, (Message, GetMessageError)>,
    {
        let mut message_queue = self.messages.lock();
        match f(message_queue.pop_front().ok_or(GetMessageError::NoMessage)?) {
            Ok(value) => Ok(value),
            Err((message, err)) => {
                message_queue.push_front(message);
                Err(err)
            }
        }
    }
}

impl KernelObject for ChannelEnd {
    fn id(&self) -> KernelObjectId {
        self.id
    }

    fn typ(&self) -> KernelObjectType {
        KernelObjectType::Channel
    }
}

pub struct Message {
    pub bytes: Vec<u8>,
    /// The actual objects extracted from the handles transferred by a message. When a task receives this message,
    /// these objects are added to that task, and the new handles are put into the message. The non-`None` entries
    /// of this array must be contiguous - there cannot be a `None` entry before more non-`None` entries.
    pub handle_objects: [Option<Arc<dyn KernelObject>>; CHANNEL_MAX_NUM_HANDLES],
}

impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Message").field("bytes", &self.bytes).finish_non_exhaustive()
    }
}

impl Message {
    pub fn num_handles(&self) -> usize {
        self.handle_objects.iter().fold(0, |n, ref handle| if handle.is_some() { n + 1 } else { n })
    }
}
