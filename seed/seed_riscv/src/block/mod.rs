pub mod virtio;

use core::ptr::NonNull;

pub trait BlockDevice {
    type ReadTokenMetadata;

    fn read(&mut self, block: u64) -> ReadToken<Self::ReadTokenMetadata>;
    fn free_read_block(&mut self, token: ReadToken<Self::ReadTokenMetadata>);
}

/// Represents a block that has been read from a `BlockDevice`. Must be freed using
/// `BlockDevice::free_read_block`.
pub struct ReadToken<M> {
    pub data: NonNull<[u8; 512]>,
    meta: M,
}
