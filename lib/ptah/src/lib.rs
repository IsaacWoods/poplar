#![no_std]
#![feature(decl_macro, maybe_uninit_slice, never_type)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "derive")]
extern crate ptah_derive;
#[cfg(feature = "derive")]
#[doc(hidden)]
pub use ptah_derive::*;

pub mod de;
pub mod ser;

pub use de::{Deserialize, DeserializeOwned, Deserializer};
pub use ser::{Serialize, Serializer};

/// It can sometimes be useful to know the size of a value in its serialized form (e.g. to reserve space for it in
/// a ring buffer). This calculates the number of bytes taken to serialize some `value` of `T` into Ptah's wire
/// format. Note that this size is for the specific `value`, and may differ between values of `T`.
pub fn serialized_size<T>(value: &T) -> ser::Result<usize>
where
    T: Serialize,
{
    let mut size = 0;
    let mut serializer = Serializer::new(SizeCalculator { size: &mut size });

    value.serialize(&mut serializer)?;
    Ok(size)
}

pub fn to_wire<'w, T, W>(value: &T, writer: W) -> ser::Result<usize>
where
    T: Serialize,
    W: Writer,
{
    let mut serializer = Serializer::new(writer);

    value.serialize(&mut serializer)?;
    Ok(serializer.writer.bytes_written())
}

/// Deserialize a `T` from some bytes and, optionally, some handles. If the wire is not able to transport handles,
/// it is fine to produce `&[]` (as long as `T` does not contain any handles, that is).
pub fn from_wire<'a, 'de, T>(bytes: &'a [u8], handles: &'a [Handle]) -> de::Result<T>
where
    'a: 'de,
    T: Deserialize<'de>,
{
    let mut deserializer = Deserializer::from_wire(bytes, handles);
    let value = T::deserialize(&mut deserializer)?;

    if deserializer.bytes.is_empty() {
        Ok(value)
    } else {
        Err(de::Error::TrailingBytes)
    }
}

pub type Handle = u32;
pub type HandleSlot = u8;

/*
 * These are constants that are used in the wire format.
 */
pub(crate) const MARKER_FALSE: u8 = 0x0;
pub(crate) const MARKER_TRUE: u8 = 0x1;
pub(crate) const MARKER_NONE: u8 = 0x0;
pub(crate) const MARKER_SOME: u8 = 0x1;
pub(crate) const HANDLE_SLOT_0: u8 = 0xf0;
pub(crate) const HANDLE_SLOT_1: u8 = 0xf1;
pub(crate) const HANDLE_SLOT_2: u8 = 0xf2;
pub(crate) const HANDLE_SLOT_3: u8 = 0xf3;

pub fn make_handle_slot(index: u8) -> HandleSlot {
    match index {
        0 => HANDLE_SLOT_0,
        1 => HANDLE_SLOT_1,
        2 => HANDLE_SLOT_2,
        3 => HANDLE_SLOT_3,
        _ => panic!("Invalid handle slot index!"),
    }
}

pub fn index_from_handle_slot(slot: HandleSlot) -> u8 {
    match slot {
        HANDLE_SLOT_0 => 0,
        HANDLE_SLOT_1 => 1,
        HANDLE_SLOT_2 => 2,
        HANDLE_SLOT_3 => 3,
        _ => panic!("Invalid handle slot!"),
    }
}

/// A `Writer` represents a consumer of the bytes produced by serializing a message. In cases where you can
/// create a slice to put the bytes in, `CursorWriter` can be used. Custom `Writer`s are useful for more niche
/// uses, such as sending the serialized bytes over a serial port.
pub trait Writer {
    fn write(&mut self, buf: &[u8]) -> ser::Result<()>;
    fn push_handle(&mut self, handle: Handle) -> ser::Result<HandleSlot>;
    fn bytes_written(&self) -> usize;
}

/// This is a `Writer` that can be used to serialize a value into a pre-allocated byte buffer.
pub struct CursorWriter<'a> {
    buffer: &'a mut [u8],
    cursor: usize,
}

impl<'a> CursorWriter<'a> {
    pub fn new(buffer: &'a mut [u8]) -> CursorWriter<'a> {
        CursorWriter { buffer, cursor: 0 }
    }
}

impl<'a> Writer for CursorWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> ser::Result<()> {
        /*
         * Detect if the write will overflow the buffer.
         */
        if (self.cursor + buf.len()) > self.buffer.len() {
            return Err(ser::Error::WriterFullOfBytes);
        }

        self.buffer[self.cursor..(self.cursor + buf.len())].copy_from_slice(buf);
        self.cursor += buf.len();
        Ok(())
    }

    fn push_handle(&mut self, _handle: Handle) -> ser::Result<HandleSlot> {
        unimplemented!()
    }

    fn bytes_written(&self) -> usize {
        self.cursor
    }
}

#[cfg(feature = "alloc")]
impl<'a> Writer for &'a mut alloc::vec::Vec<u8> {
    fn write(&mut self, buf: &[u8]) -> ser::Result<()> {
        self.extend_from_slice(buf);
        Ok(())
    }

    fn push_handle(&mut self, _handle: Handle) -> ser::Result<HandleSlot> {
        unimplemented!()
    }

    fn bytes_written(&self) -> usize {
        self.len()
    }
}

/// This is a writer that can be used to calculate the size of a serialized value. It doesn't actually write the
/// serialized bytes anywhere - it simply tracks how are produced. Because the `Serializer` takes the `Writer` by
/// value, this stores a reference back to the size, so it can be accessed after serialization is complete.
struct SizeCalculator<'a> {
    size: &'a mut usize,
}

impl<'a> Writer for SizeCalculator<'a> {
    fn write(&mut self, buf: &[u8]) -> ser::Result<()> {
        *self.size += buf.len();
        Ok(())
    }

    fn push_handle(&mut self, _handle: Handle) -> ser::Result<HandleSlot> {
        /*
         * When calculating the size, we simply accept as many handles as we're passed. The encoded slot is always
         * the same size, so it doesn't matter what we return.
         */
        Ok(HANDLE_SLOT_0)
    }

    fn bytes_written(&self) -> usize {
        *self.size
    }
}
