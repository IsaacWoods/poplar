use crate::{
    syscall::{self, CreateChannelError, GetMessageError, SendMessageError, CHANNEL_MAX_NUM_HANDLES},
    Handle,
};
use alloc::vec::Vec;
use core::{future::Future, marker::PhantomData, mem, task::Poll};
use ptah::{DeserializeOwned, Serialize};

// TODO: we now have heap-allocated buffers for sending, but still have bounded receives based on
// stack sizes. Is there any way of dealing with larger messages on receive?
const BYTES_BUFFER_SIZE: usize = 2048;

#[derive(Debug)]
pub enum ChannelSendError {
    FailedToSerialize(ptah::ser::Error),
    SendError(SendMessageError),
}

#[derive(Debug)]
pub enum ChannelReceiveError {
    FailedToDeserialize(ptah::de::Error),
    ReceiveError(GetMessageError),
}

pub struct Channel<S, R>(Handle, PhantomData<(S, R)>)
where
    S: Serialize + DeserializeOwned,
    R: Serialize + DeserializeOwned;

impl<S, R> Channel<S, R>
where
    S: Serialize + DeserializeOwned,
    R: Serialize + DeserializeOwned,
{
    pub fn new_from_handle(handle: Handle) -> Channel<S, R> {
        Channel(handle, PhantomData)
    }

    /// Create a new channel. Returns one end as a `Channel`, and a `Handle` for the other end.
    /// Generally, the handle is passed to another task.
    pub fn create() -> Result<(Channel<S, R>, Handle), CreateChannelError> {
        let (this_end, other_end) = syscall::create_channel()?;
        Ok((Self::new_from_handle(this_end), other_end))
    }

    pub fn send(&self, message: &S) -> Result<(), ChannelSendError> {
        let mut writer = ChannelWriter::new();
        ptah::to_wire(message, &mut writer).map_err(|err| ChannelSendError::FailedToSerialize(err))?;
        syscall::send_message(self.0, writer.bytes(), writer.handles())
            .map_err(|err| ChannelSendError::SendError(err))
    }

    /// Receive a message from the channel, if there's one waiting. Returns `Ok(None)` if there are no pending
    /// messages to be received.
    pub fn try_receive(&self) -> Result<Option<R>, ChannelReceiveError> {
        let mut byte_buffer = [0u8; BYTES_BUFFER_SIZE];
        let mut handle_buffer = [Handle::ZERO; CHANNEL_MAX_NUM_HANDLES];

        match syscall::get_message(self.0, &mut byte_buffer, &mut handle_buffer) {
            Ok((bytes, handles)) => {
                // TODO: this looks really bad, but is actually fine (since Handle is just a transparent wrapper
                // around a `u32`). There might be a better way.
                let ptah_handles: &[u32] = unsafe { mem::transmute(handles) };

                let message: R = ptah::from_wire(bytes, ptah_handles)
                    .map_err(|err| ChannelReceiveError::FailedToDeserialize(err))?;
                Ok(Some(message))
            }
            Err(GetMessageError::NoMessage) => Ok(None),
            Err(err) => Err(ChannelReceiveError::ReceiveError(err)),
        }
    }

    /// Wait for a message to arrive via the channel.
    pub fn receive_blocking(&self) -> Result<R, ChannelReceiveError> {
        loop {
            let mut byte_buffer = [0u8; BYTES_BUFFER_SIZE];
            let mut handle_buffer = [Handle::ZERO; CHANNEL_MAX_NUM_HANDLES];

            match syscall::get_message(self.0, &mut byte_buffer, &mut handle_buffer) {
                Ok((bytes, handles)) => {
                    // TODO: this looks really bad, but is actually fine (since Handle is just a transparent wrapper
                    // around a `u32`). There might be a better way.
                    let ptah_handles: &[u32] = unsafe { mem::transmute(handles) };

                    let message: R = ptah::from_wire(bytes, ptah_handles)
                        .map_err(|err| ChannelReceiveError::FailedToDeserialize(err))?;
                    return Ok(message);
                }
                Err(GetMessageError::NoMessage) => {
                    crate::syscall::yield_to_kernel();
                }
                Err(err) => {
                    return Err(ChannelReceiveError::ReceiveError(err));
                }
            }
        }
    }

    pub fn receive(&self) -> impl Future<Output = Result<R, ChannelReceiveError>> + '_ {
        core::future::poll_fn(|context| {
            let mut byte_buffer = [0u8; BYTES_BUFFER_SIZE];
            let mut handle_buffer = [Handle::ZERO; CHANNEL_MAX_NUM_HANDLES];

            match syscall::get_message(self.0, &mut byte_buffer, &mut handle_buffer) {
                Ok((bytes, handles)) => {
                    // TODO: this looks really bad, but is actually fine (since Handle is just a transparent wrapper
                    // around a `u32`). There might be a better way.
                    let ptah_handles: &[u32] = unsafe { mem::transmute(handles) };

                    let message: R = ptah::from_wire(bytes, ptah_handles)
                        .map_err(|err| ChannelReceiveError::FailedToDeserialize(err))?;
                    Poll::Ready(Ok(message))
                }
                Err(GetMessageError::NoMessage) => {
                    crate::rt::RUNTIME.get().reactor.lock().register(self.0, context.waker().clone());
                    Poll::Pending
                }
                Err(err) => Poll::Ready(Err(ChannelReceiveError::ReceiveError(err))),
            }
        })
    }
}

struct ChannelWriter {
    byte_buffer: Vec<u8>,
    handle_buffer: [Handle; CHANNEL_MAX_NUM_HANDLES],
    num_handles: u8,
}

impl ChannelWriter {
    pub fn new() -> ChannelWriter {
        ChannelWriter {
            byte_buffer: Vec::new(),
            handle_buffer: [Handle::ZERO; CHANNEL_MAX_NUM_HANDLES],
            num_handles: 0,
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.byte_buffer
    }

    pub fn handles(&self) -> &[Handle] {
        &self.handle_buffer[0..(self.num_handles as usize)]
    }
}

impl<'a> ptah::Writer for &'a mut ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> ptah::ser::Result<()> {
        self.byte_buffer.extend_from_slice(buf);
        Ok(())
    }

    fn push_handle(&mut self, handle: ptah::Handle) -> ptah::ser::Result<ptah::HandleSlot> {
        /*
         * Check if we're full of handles yet.
         */
        if (self.num_handles as usize + 1) > CHANNEL_MAX_NUM_HANDLES {
            return Err(ptah::ser::Error::WriterFullOfHandles);
        }

        self.handle_buffer[self.num_handles as usize] = Handle(handle);

        let slot = ptah::make_handle_slot(self.num_handles);
        self.num_handles += 1;
        Ok(slot)
    }

    fn bytes_written(&self) -> usize {
        self.byte_buffer.len()
    }
}
