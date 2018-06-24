use Message;

#[derive(Serialize, Deserialize)]
pub enum KernelMessage {
    /*
     * These are just random test messages for now
     */
    A,
    B,
    C,
}

impl<'a> Message<'a> for KernelMessage {}
