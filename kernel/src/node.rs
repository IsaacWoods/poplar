use alloc::{boxed::Box, BTreeMap};
use core::any::Any;
use core::mem;
use libmessage::{Message, NodeId};

pub trait Node {
    type MessageType: Message;

    fn message(&mut self, sender: NodeId, message: Self::MessageType) -> Result<(), ()>;
}

pub struct NodeManager {
    next_id: NodeId,
    nodes: BTreeMap<NodeId, Box<Node<MessageType = Any>>>,
}

impl NodeManager {
    pub fn new() -> NodeManager {
        NodeManager {
            next_id: NodeId(0),
            nodes: BTreeMap::new(),
        }
    }

    fn allocate_node_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id.0 += 1;
        id
    }

    pub fn add_node<M>(&mut self, node: Box<Node<MessageType = M>>) -> NodeId
    where
        M: Message,
    {
        let id = self.allocate_node_id();
        self.nodes.insert(id, unsafe { upcast_message_type(node) });
        id
    }

    pub fn get_node<M>(&mut self, id: NodeId) -> Option<&mut Box<Node<MessageType = M>>>
    where
        M: Message,
    {
        Some(unsafe { downcast_message_type_ref(self.nodes.get_mut(&id)?) })
    }
}

/// Upcast a trait-object Node with a specific message type `M` to one with a message type of
/// `Any`. Unsafe because it is not enforcable that the message type is `Any` - it must not contain
/// any non-`'static` references.
unsafe fn upcast_message_type<M>(node: Box<Node<MessageType = M>>) -> Box<Node<MessageType = Any>>
where
    M: ?Sized,
{
    mem::transmute(node)
}

/// Downcast a trait-object Node with a message type of `Any` to one with a specific message type
/// `M`. Unsafe because you must be sure that the node is actually of the correct message type, or
/// you'll send it the wrong one later. Also unsafe because the conditions of `Any` aren't
/// enforcable - MessageType must not include non-`'static` references or this may produce UB.
unsafe fn downcast_message_type_ref<M>(
    node: &mut Box<Node<MessageType = Any>>,
) -> &mut Box<Node<MessageType = M>>
where
    M: ?Sized,
{
    mem::transmute(node)
}
