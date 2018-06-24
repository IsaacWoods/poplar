use libmessage::Message;

#[derive(Debug, Serialize, Deserialize)]
pub enum ProcessMessage {
    /// This drops to usermode and starts executing the process this message is sent to. The call
    /// to `message` is diverging for this message.
    DropToUsermode,
}

impl<'a> Message<'a> for ProcessMessage {}
