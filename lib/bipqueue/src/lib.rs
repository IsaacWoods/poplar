//! A `BipQueue` is a SPSC lockless queue based on bi-partite circular buffers (commonly referred
//! to as Bip-Buffers). It is inspired by James Munns' `bbqueue` crate, but implemented from
//! scratch as a learning exercise for me.
//!
//! Useful resources:
//!    - [The `bbqueue` crate](https://github.com/jamesmunns/bbqueue/)
//!    - [This blog post](https://ferrous-systems.com/blog/lock-free-ring-buffer/)
//!    - [This article](https://www.codeproject.com/articles/3479/the-bip-buffer-the-circular-buffer-with-a-twist)

#![no_std]

#[cfg(test)]
extern crate std;

use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
    ptr::NonNull,
    slice,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

pub struct BipQueue<const N: usize> {
    storage: UnsafeCell<MaybeUninit<[u8; N]>>,
    read: AtomicUsize,
    write: AtomicUsize,
    /// Used to mark a region as reserved for writing, but before its grant is committed.
    reserve: AtomicUsize,
    /// In the two-region scenario, this marks the end of the upper region. Data after this point
    /// is invalid and should not be read, and the reader should instead start again from the
    /// bigging of the buffer. If there is only one active region, this should be set to the size
    /// of the buffer.
    watermark: AtomicUsize,

    read_granted: AtomicBool,
    write_granted: AtomicBool,
}

unsafe impl<const N: usize> Send for BipQueue<N> {}
unsafe impl<const N: usize> Sync for BipQueue<N> {}

impl<const N: usize> BipQueue<N> {
    pub const fn new() -> BipQueue<N> {
        BipQueue {
            storage: UnsafeCell::new(MaybeUninit::uninit()),
            read: AtomicUsize::new(0),
            write: AtomicUsize::new(0),
            reserve: AtomicUsize::new(0),
            watermark: AtomicUsize::new(N),

            read_granted: AtomicBool::new(false),
            write_granted: AtomicBool::new(false),
        }
    }

    pub fn grant(&self, length: usize) -> Result<WriteGrant<'_, N>, Error> {
        if self.write_granted.swap(true, Ordering::AcqRel) {
            return Err(Error::AlreadyGranted);
        }

        let read = self.read.load(Ordering::Acquire);
        let write = self.write.load(Ordering::Acquire);

        let start = if write < read {
            /*
             * There are already two active regions. Check if there is still space available -
             * write must never catch up with read.
             */
            if (write + length) < read {
                write
            } else {
                self.write_granted.store(false, Ordering::Release);
                return Err(Error::NotEnoughSpace);
            }
        } else {
            /*
             * There is only one active region. See if we can fit it on the end, otherwise go back
             * round to the beginning.
             */
            if (write + length) <= N {
                write
            } else if length < read {
                /*
                 * There's space to create a second active region at the front of the buffer. We
                 * must make sure here not to let `write == read`, or we won't be able to tell how
                 * many regions we have active.
                 */
                0
            } else {
                self.write_granted.store(false, Ordering::Release);
                return Err(Error::NotEnoughSpace);
            }
        };

        self.reserve.store(start + length, Ordering::Release);

        /*
         * Create a slice of the granted part of the buffer. Casting through the `MaybeUninit` is
         * safe because it's `repr(transparent)`.
         */
        let grant_buffer =
            unsafe { slice::from_raw_parts_mut(self.storage.get().cast::<u8>().add(start), length) };
        Ok(WriteGrant {
            buffer: grant_buffer,
            queue: unsafe { NonNull::new_unchecked(self as *const Self as *mut Self) },
        })
    }

    pub fn read(&self) -> Result<ReadGrant<'_, N>, Error> {
        if self.read_granted.swap(true, Ordering::AcqRel) {
            return Err(Error::AlreadyGranted);
        }

        let read = self.read.load(Ordering::Acquire);
        let write = self.write.load(Ordering::Acquire);
        let watermark = self.watermark.load(Ordering::Acquire);

        if (write < read) && (read == watermark) {
            /*
             * We have two active regions, and we've finished the second one (read up to the
             * watermark). We move read to the front of the buffer.
             */
            self.read.store(0, Ordering::Release);
        }

        let length = if write < read {
            // Two active regions - read til the watermark
            watermark - read
        } else {
            // One active region - read til the point we've written up to
            write - read
        };

        if length == 0 {
            self.read_granted.store(false, Ordering::Release);
            return Err(Error::NoBytes);
        }

        /*
         * Create a slice of the readable part of the buffer. Casting through `MaybeUninit` is safe
         * because it's `repr(transparent)`.
         */
        let grant_buffer = unsafe { slice::from_raw_parts(self.storage.get().cast::<u8>().add(read), length) };
        Ok(ReadGrant {
            buffer: grant_buffer,
            queue: unsafe { NonNull::new_unchecked(self as *const Self as *mut Self) },
        })
    }
}

pub struct WriteGrant<'a, const N: usize> {
    pub buffer: &'a mut [u8],
    queue: NonNull<BipQueue<N>>,
}

impl<'a, const N: usize> WriteGrant<'a, N> {
    pub fn commit(self, written: usize) {
        let written = usize::min(written, self.buffer.len());

        let queue = unsafe { self.queue.as_ref() };
        let write = queue.write.load(Ordering::Acquire);

        // If less than the entire region was written into, reduce the reserved area.
        queue.reserve.fetch_sub(self.buffer.len() - written, Ordering::AcqRel);

        let watermark = queue.watermark.load(Ordering::Acquire);
        let new_write = queue.reserve.load(Ordering::Acquire);

        if (new_write < write) && (write != N) {
            /*
             * This write is creating a second active region, leaving bytes at the end of the
             * buffer invalid. Mark the watermark here to prevent a read from reading those bytes.
             */
            queue.watermark.store(write, Ordering::Release);
        } else if new_write > watermark {
            /*
             * We're going to write past the previous watermark. This means the lower active region
             * has cleared the higher, so we can push the watermark back to the end of the buffer.
             * A read will stop at the new `write` marker.
             */
            queue.watermark.store(N, Ordering::Release);
        }

        queue.write.store(new_write, Ordering::Release);
        queue.write_granted.store(false, Ordering::Release);
    }
}

impl<'a, const N: usize> Deref for WriteGrant<'a, N> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.buffer
    }
}

impl<'a, const N: usize> DerefMut for WriteGrant<'a, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.buffer
    }
}

impl<'a, const N: usize> Drop for WriteGrant<'a, N> {
    fn drop(&mut self) {
        let queue = unsafe { self.queue.as_ref() };
        queue.write_granted.store(false, Ordering::Release);
    }
}

unsafe impl<'a, const N: usize> Send for WriteGrant<'a, N> {}

pub struct ReadGrant<'a, const N: usize> {
    pub buffer: &'a [u8],
    queue: NonNull<BipQueue<N>>,
}

impl<'a, const N: usize> ReadGrant<'a, N> {
    pub fn release(self, read: usize) {
        let read = usize::min(read, self.buffer.len());
        let queue = unsafe { self.queue.as_ref() };

        queue.read.fetch_add(read, Ordering::Release);
        queue.read_granted.store(false, Ordering::Release);
    }
}

impl<'a, const N: usize> Deref for ReadGrant<'a, N> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.buffer
    }
}

impl<'a, const N: usize> Drop for ReadGrant<'a, N> {
    fn drop(&mut self) {
        let queue = unsafe { self.queue.as_ref() };
        queue.read_granted.store(false, Ordering::Release);
    }
}

unsafe impl<'a, const N: usize> Send for ReadGrant<'a, N> {}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    AlreadyGranted,
    NotEnoughSpace,
    NoBytes,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn basic() {
        let queue: BipQueue<16> = BipQueue::new();

        {
            let write = queue.grant(4).unwrap();
            write.buffer.copy_from_slice(&[1, 2, 3, 4]);
            write.commit(4);
        }
        {
            let write = queue.grant(6).unwrap();
            write.buffer.copy_from_slice(&[5, 6, 7, 8, 9, 10]);
            write.commit(6);
        }
        {
            let read = queue.read().unwrap();
            assert_eq!(read.buffer.len(), 10);
            assert_eq!(read.buffer[0..2], [1, 2]);
            read.release(2);
        }
        {
            let write = queue.grant(1).unwrap();
            write.buffer.copy_from_slice(&[11]);
            write.commit(1);
        }
        {
            let read = queue.read().unwrap();
            assert_eq!(read.buffer.len(), 9);
            assert_eq!(read.buffer, [3, 4, 5, 6, 7, 8, 9, 10, 11]);
            read.release(9);
        }
    }
}
