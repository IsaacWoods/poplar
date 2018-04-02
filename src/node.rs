/*
 * Copyright (C) 2018, Pebble Developers.
 * See LICENCE.md
 */

use core::mem;
use core::any::Any;
use alloc::{String,Vec,boxed::Box};
use libpebble::node::NodeId;

pub trait Node
{
    type MessageType;

    /// Send a message to this node.
    fn message(&self, sender : NodeId, message : Self::MessageType);
}

pub struct NodeWrapper<M : ?Sized>
{
    // id          : NodeId,
    name        : Option<String>,
    children    : Vec<NodeWrapper<Any>>,
    node        : Box<Node<MessageType=M>>,
}

/// This solves the problem of where we have a node taking a particular `MessageType`, but want to
/// deal with it generically. It uses the dreaded `mem::transmute`, but should be safe because
/// all types implement `Any`, and the bit representation is exactly the same; we're only changing
/// it at the type-level.
// XXX: we should still look to see if we can make this less unsafe
fn upcast_to_any<M>(node : Box<Node<MessageType=M>>) -> Box<Node<MessageType=Any>>
    where M : ?Sized
{
    unsafe
    {
        mem::transmute(node)
    }
}

impl<M> NodeWrapper<M>
    where M : ?Sized
{
    pub fn add_child<Other>(&mut self, name : Option<String>, node : Box<Node<MessageType=Other>>)
        where Other : ?Sized
    {
        self.children.push(NodeWrapper
                           {
                               name,
                               children : Vec::new(),
                               node : upcast_to_any(node),
                           });
    }
}

pub fn make_root_node() -> NodeWrapper<usize>
{
    NodeWrapper
    {
        name        : Some(String::from("/")),
        children    : Vec::new(),
        node        : Box::new(RootNode::new()),
    }
}

struct RootNode
{
}

impl RootNode
{
    fn new() -> RootNode
    {
        RootNode
        {
        }
    }
}

impl Node for RootNode
{
    type MessageType = usize;

    fn message(&self, sender : NodeId, message : usize)
    {
        unimplemented!();
    }
}
