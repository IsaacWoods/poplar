use core::{mem,fmt::Debug};
use core::any::Any;
use alloc::{String,Vec,boxed::Box,BTreeMap};
use libpebble::node::NodeId;

pub struct NodeManager
{
    next_id : NodeId,
    nodes   : BTreeMap<NodeId, NodeWrapper<Any>>,
    at_root : Vec<NodeId>,
}

impl NodeManager
{
    pub fn new() -> NodeManager
    {
        NodeManager
        {
            next_id : NodeId(0),
            nodes   : BTreeMap::new(),
            at_root : Vec::new(),
        }
    }

    fn allocate_node_id(&mut self) -> NodeId
    {
        let id = self.next_id;
        self.next_id.advance(1);
        id
    }

    pub(self) fn add_node<M>(&mut self,
                             name   : Option<String>,
                             node   : Box<Node<MessageType=M>>) -> NodeId
        where M : ?Sized
    {
        let id = self.allocate_node_id();

        self.nodes.insert(id, NodeWrapper
                              {
                                  id,
                                  name,
                                  children    : Vec::new(),
                                  node        : unsafe { upcast_message_type(node) },
                              });

        id
    }

    pub fn add_root_node<M>(&mut self,
                            name : Option<String>,
                            node : Box<Node<MessageType=M>>) -> NodeId
        where M : ?Sized
    {
        let id = self.add_node(name, node);
        self.at_root.push(id);
        id
    }

    pub fn get<M>(&mut self, id : NodeId) -> &mut Box<Node<MessageType=M>>
    {
        let any_node = self.nodes.get_mut(&id).unwrap();
        unsafe
        {
            downcast_message_type_ref(&mut any_node.node)
        }
    }
}

pub trait Node : Debug
{
    type MessageType;

    /// Send a message to this node.
    fn message(&mut self, sender : NodeId, message : Self::MessageType);
}

#[derive(Debug)]
pub struct NodeWrapper<M : ?Sized>
{
    id          : NodeId,
    name        : Option<String>,
    children    : Vec<NodeId>,
    node        : Box<Node<MessageType=M>>,
}

impl<M> NodeWrapper<M>
    where M : ?Sized
{
    pub fn add_child<Other>(&mut self,
                            name            : Option<String>,
                            node            : Box<Node<MessageType=Other>>,
                            node_manager    : &mut NodeManager) -> NodeId
        where Other : ?Sized
    {
        let id = node_manager.add_node(name, node);
        self.children.push(id);
        id
    }
}

/// Upcast a trait-object Node with a specific message type `M` to one with a message type of
/// `Any`. Unsafe because it is not enforcable that the message type is `Any` - it must not contain
/// any non-`'static` references.
unsafe fn upcast_message_type<M>(node : Box<Node<MessageType=M>>) -> Box<Node<MessageType=Any>>
    where M : ?Sized
{
    mem::transmute(node)
}

/// Downcast a trait-object Node with a message type of `Any` to one with a specific message type
/// `M`. Unsafe because you must be sure that the node is actually of the correct message type, or
/// you'll send it the wrong one later. Also unsafe because the conditions of `Any` aren't
/// enforcable - MessageType must not include non-`'static` references or this may produce UB.
unsafe fn downcast_message_type_ref<M>(node : &mut Box<Node<MessageType=Any>>) -> &mut Box<Node<MessageType=M>>
    where M : ?Sized
{
    mem::transmute(node)
}
