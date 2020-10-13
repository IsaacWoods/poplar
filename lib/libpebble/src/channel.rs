use crate::{
    syscall::{self, GetMessageError, RegisterServiceError, SendMessageError, CHANNEL_MAX_NUM_HANDLES},
    Handle,
};
use core::{marker::PhantomData, mem};
use ptah::{DeserializeOwned, Serialize};

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
    pub fn register_service(name: &str) -> Result<Channel<S, R>, RegisterServiceError> {
        Ok(Self::from_handle(syscall::register_service(name)?))
    }

    pub fn from_handle(handle: Handle) -> Channel<S, R> {
        Channel(handle, PhantomData)
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
        let mut handle_buffer = [crate::ZERO_HANDLE; CHANNEL_MAX_NUM_HANDLES];

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
}

const BYTES_BUFFER_SIZE: usize = 512;

struct ChannelWriter {
    byte_buffer: [u8; BYTES_BUFFER_SIZE],
    handle_buffer: [Handle; CHANNEL_MAX_NUM_HANDLES],
    num_bytes: usize,
    num_handles: u8,
}

impl ChannelWriter {
    pub fn new() -> ChannelWriter {
        ChannelWriter {
            byte_buffer: [0u8; BYTES_BUFFER_SIZE],
            handle_buffer: [crate::ZERO_HANDLE; CHANNEL_MAX_NUM_HANDLES],
            num_bytes: 0,
            num_handles: 0,
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.byte_buffer[0..self.num_bytes]
    }

    pub fn handles(&self) -> &[Handle] {
        &self.handle_buffer[0..(self.num_handles as usize)]
    }
}

impl<'a> ptah::Writer for &'a mut ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> ptah::ser::Result<()> {
        /*
         * Detect if the write will overflow the buffer.
         */
        if (self.num_bytes + buf.len()) > BYTES_BUFFER_SIZE {
            return Err(ptah::ser::Error::WriterFullOfBytes);
        }

        self.byte_buffer[self.num_bytes..(self.num_bytes + buf.len())].copy_from_slice(buf);
        self.num_bytes += buf.len();
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
}
