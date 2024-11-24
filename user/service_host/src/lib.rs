//! `service_host` is an implementation of a Poplar bootstrap task (the first task to run in
//! userspace) that spawns other tasks loaded by Seed, and provides userspace service discovery.

use ptah::{Deserialize, DeserializeOwned, Serialize};
use std::poplar::{channel::Channel, Handle};

/// A request sent by a client task to `service_host`
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum ServiceHostRequest {
    RegisterService { name: String },
    SubscribeService(String),
    // TODO: should this be typed, stringy, or something else?
    RequestResource(String),
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum ServiceHostResponse {
    ServiceRegistered(Handle),
    SubscribedToService(Handle),
    NoSuchService,
    Resource(Handle),
    ResourceRefused,
}

/// A message sent by `service_host` to a service provider when another task subscribes to a
/// service.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum ServiceChannelMessage {
    NewClient { name: String, channel: Handle },
}

/// Represents a channel connected to `service_host` for a client task to make requests through.
pub struct ServiceHostClient {
    channel: Channel<ServiceHostRequest, ServiceHostResponse>,
}

impl ServiceHostClient {
    /// Find the channel to `service_host` and create a `ServiceHostClient`.
    // TODO: how should we find the right handle from a random task? Manifest? Just guess?
    pub fn new() -> ServiceHostClient {
        /*
         * TODO: this is very janky for now, but we basically abuse our knowledge of how
         * `service_host` constructs tasks for now. We only add the address space and the task
         * channel to the handle set for now, so we know the channel is going to be handle `2`.
         */
        let channel = Channel::new_from_handle(Handle(2));
        ServiceHostClient { channel }
    }

    // TODO: probs need async and blocking versions of these? (actually it's quite a lot simpler to
    // just allow blocking here I think. Probs what we'll want in the clients anyway.)
    pub fn register_service(&self, name: impl ToString) -> Result<Channel<(), ServiceChannelMessage>, ()> {
        self.channel.send(&ServiceHostRequest::RegisterService { name: name.to_string() }).unwrap();
        match self.channel.receive_blocking().unwrap() {
            ServiceHostResponse::ServiceRegistered(channel) => Ok(Channel::new_from_handle(channel)),
            _ => {
                panic!("Received incorrect response to RegisterService request");
            }
        }
    }

    pub fn subscribe_service<S, R>(&self, name: impl ToString) -> Result<Channel<S, R>, ()>
    where
        S: Serialize + DeserializeOwned,
        R: Serialize + DeserializeOwned,
    {
        self.channel.send(&ServiceHostRequest::SubscribeService(name.to_string())).unwrap();
        match self.channel.receive_blocking().unwrap() {
            ServiceHostResponse::SubscribedToService(channel) => Ok(Channel::new_from_handle(channel)),
            _ => {
                panic!("Received incorrect response to SubscribeService request");
            }
        }
    }

    pub fn request_resource(&self, name: impl ToString) -> Result<Handle, ()> {
        todo!()
    }
}
