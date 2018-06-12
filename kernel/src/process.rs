use alloc::boxed::Box;
use libmessage::{Message, MessageHeader};

#[derive(Debug)]
pub enum ProcessMessage {
    /// This drops to usermode and starts executing the process this message is sent to. The call
    /// to `message` is diverging for this message.
    DropToUsermode,
}

impl Message for ProcessMessage {
    fn encode<'a>(self) -> &'a [u8] {
        unimplemented!();
    }

    fn decode(header: &MessageHeader, payload: &[u8]) -> Option<Box<Self>> {
        unimplemented!();
    }
}
