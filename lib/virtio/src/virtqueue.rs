use alloc::collections::VecDeque;
use core::{
    mem,
    mem::MaybeUninit,
    ptr,
    ptr::{NonNull, Pointee},
};

/// A virtqueue is the mechanism used for bulk data transport to and from Virtio devices. We use the split
/// virtqueue representation - the first format of virtqueue supported by Virtio.
///
/// Buffers can be added to the virtqueue to make requests of the device. Devices then execute these requests, and
/// when complete, mark the buffers as 'used' by the device.
///
/// A split virtqueue is comprised of three "areas", each of which can be separately allocated:
///    - The Descriptor Table (of size `16 * Queue Size`)
///    - The Available Ring (of size `6 + 2 * Queue Size`)
///    - The Used Ring (of size `6 + 8 * Queue Size`)
/// The queue size is found in a transport-specific way (and is a maximum of `32768`).
pub struct Virtqueue {
    free_entries: VecDeque<u16>,
    pub descriptor_table: Mapped<[Descriptor]>,
    pub available_ring: Mapped<AvailableRing>,
    pub used_ring: Mapped<UsedRing>,
    used_ring_last_seen: u16,
}

impl Virtqueue {
    pub fn new<M>(queue_size: u16, mapper: &M) -> Virtqueue
    where
        M: Mapper,
    {
        let free_entries = (0..queue_size).collect();
        let queue_size = queue_size as usize;
        // let descriptor_table = mapper.create_slice(queue_size, 16 * queue_size);
        // let available_ring = mapper.create_slice(queue_size, 6 + 2 * queue_size);
        // let used_ring = mapper.create_slice(queue_size, 7 + 8 * queue_size);
        let descriptor_table = unsafe { Mapped::new_slice(queue_size as usize, mapper).assume_init() };
        let available_ring = unsafe { Mapped::new(queue_size as usize, mapper) };
        let used_ring = unsafe { Mapped::new(queue_size as usize, mapper) };

        Virtqueue { free_entries, descriptor_table, available_ring, used_ring, used_ring_last_seen: 0 }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Descriptor {
    /// The guest-physical address of the buffer
    pub address: u64,
    pub len: u32,
    pub flags: DescriptorFlags,
    pub next: u16,
}

bitflags::bitflags! {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    #[repr(transparent)]
    pub struct DescriptorFlags: u16 {
        /// Marks a buffer as continuing in the next chained descriptor.
        const NEXT = 0b1;
        /// Marks a buffer as device write-only (if not set, the buffer is device read-only).
        const WRITE = 0b10;
        /// Marks a buffer as containing a list of buffer descriptors.
        const INDIRECT = 0b100;
    }
}

#[repr(C)]
pub struct AvailableRing {
    pub flags: u16,
    /// Where to put the next descriptor entry in the ring (modulo the queue size).
    pub index: u16,
    pub ring: [u16],
}

#[repr(C)]
pub struct UsedRing {
    pub flags: u16,
    pub index: u16,
    pub ring: [UsedRingElement],
}

#[repr(C)]
pub struct UsedRingElement {
    /// The index of the first element of the used descriptor chain.
    pub start: u32,
    /// The number of bytes written into the device-writable portion of the buffer described by the descriptor
    /// chain.
    pub length: u32,
}

/// Represents an area of physical memory that has been mapped into the virtual address space (if relevant).
// TODO: could this be some kind of common abstraction?? I feel we're really kneecapped by needing to use the same
// code from the bootloader and userspace (in the future)...
pub struct Mapped<T>
where
    T: ?Sized,
{
    pub physical: usize,
    pub mapped: NonNull<T>,
}

impl<T> Mapped<T>
where
    T: ?Sized,
{
    pub unsafe fn new<M: Mapper>(metadata: <T as Pointee>::Metadata, mapper: &M) -> Mapped<T> {
        let size = unsafe { mem::size_of_val_raw::<T>(ptr::from_raw_parts(ptr::null(), metadata)) };
        let (physical, virt) = mapper.alloc(size);

        Mapped { physical, mapped: NonNull::from_raw_parts(NonNull::new(virt as *mut _).unwrap(), metadata) }
    }
}

impl<T> Mapped<[MaybeUninit<T>]> {
    pub fn new_slice<M: Mapper>(num_elements: usize, mapper: &M) -> Mapped<[MaybeUninit<T>]> {
        let (physical, virt) = mapper.alloc(mem::size_of::<T>() * num_elements);

        Mapped {
            physical,
            mapped: NonNull::slice_from_raw_parts(
                NonNull::new(virt as *mut MaybeUninit<T>).unwrap(),
                num_elements,
            ),
        }
    }

    pub unsafe fn assume_init(self) -> Mapped<[T]> {
        let Mapped { physical, mapped } = self;
        core::mem::forget(self);

        Mapped { physical, mapped: NonNull::slice_from_raw_parts(mapped.cast(), mapped.len()) }
    }
}

impl<T> Mapped<[T]> {
    pub fn get(&mut self, index: usize) -> Option<NonNull<T>> {
        if index >= self.mapped.len() {
            return None;
        }

        Some(unsafe { NonNull::new_unchecked(self.mapped.as_ptr().get_unchecked_mut(index)) })
    }
}

pub trait Mapper {
    // fn create_slice<T>(&self, num_elements: usize, size: usize) -> Mapped<[MaybeUninit<T>]>;

    /// Allocate `size` bytes of **zeroed** memory, returning its physical and virtual addresses.
    fn alloc(&self, size: usize) -> (usize, usize);
}
