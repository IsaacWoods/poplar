use super::{alloc_kernel_object_id, KernelObject, KernelObjectId};
use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};
use libpebble::syscall::CHANNEL_MAX_NUM_HANDLES;

pub struct ChannelEnd {
    pub id: KernelObjectId,
    pub owner: KernelObjectId,
    messages: Vec<Message>,
    other_end: Weak<ChannelEnd>,
}

impl ChannelEnd {
    pub fn new_channel(owner: KernelObjectId) -> (Arc<ChannelEnd>, Arc<ChannelEnd>) {
        let mut end_a = Arc::new(ChannelEnd {
            id: alloc_kernel_object_id(),
            owner,
            messages: Vec::new(),
            other_end: Weak::default(),
        });

        let end_b = Arc::new(ChannelEnd {
            id: alloc_kernel_object_id(),
            owner,
            messages: Vec::new(),
            other_end: Arc::downgrade(&end_a),
        });

        Arc::get_mut(&mut end_a).unwrap().other_end = Arc::downgrade(&end_b);

        (end_a, end_b)
    }
}

impl KernelObject for ChannelEnd {
    fn id(&self) -> KernelObjectId {
        self.id
    }
}

pub struct Message {
    pub bytes: Vec<u8>,
    pub handle_objects: [Arc<dyn KernelObject>; CHANNEL_MAX_NUM_HANDLES],
}
