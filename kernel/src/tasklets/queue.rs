use alloc::sync::Arc;
use bipqueue::BipQueue;
use core::ops::{Deref, DerefMut};
use maitake::sync::WaitCell;

/// A SPSC (Single Producer, Single Consumer) queue built on top of a [bi-partite
/// buffer](bipqueue::BipQueue), with added asynchronous support. This is useful for things that
/// need to produce a stream of bytes somewhere (e.g. an interrupt handler, one tasklet) and then
/// consume them within another `async` tasklet.
pub struct SpscQueue {
    // TODO: rework BipQueue to have dynamically allocated storage
    storage: BipQueue<512>,
    /// This is woken when bytes are committed to the queue. It is useful for tasklets that consume
    /// bytes from the queue - if there are no bytes to consume, this can be used to wake your
    /// tasklet when there are new bytes to process.
    commit_wait: WaitCell,
    /// This is woken when bytes are released (consumed) from the queue. It is useful for tasklets
    /// that add bytes to the queue - if there is no space for the next entry, this can be used to
    /// wake your tasklet when there more space has been created.
    release_wait: WaitCell,
}

impl SpscQueue {
    pub fn new() -> (QueueProducer, QueueConsumer) {
        let queue = Arc::new(SpscQueue {
            storage: BipQueue::new(),
            commit_wait: WaitCell::new(),
            release_wait: WaitCell::new(),
        });
        let producer = QueueProducer { queue: queue.clone() };
        let consumer = QueueConsumer { queue };

        (producer, consumer)
    }
}

pub struct QueueProducer {
    queue: Arc<SpscQueue>,
}

impl QueueProducer {
    pub fn grant_sync(&self, length: usize) -> Result<WriteGrant<'_>, ()> {
        match self.queue.storage.grant(length) {
            Ok(grant) => Ok(WriteGrant { inner: grant, queue: self.queue.clone() }),
            Err(_) => Err(()),
        }
    }

    pub async fn grant(&self, length: usize) -> WriteGrant<'_> {
        loop {
            let wait = self.queue.release_wait.subscribe().await;
            match self.queue.storage.grant(length) {
                Ok(grant) => return WriteGrant { inner: grant, queue: self.queue.clone() },
                Err(_) => {
                    /*
                     * There's not enough space in the buffer - wait for some bytes to be consumed
                     * and have another go.
                     */
                    wait.await.unwrap();
                }
            }
        }
    }
}

pub struct WriteGrant<'a> {
    inner: bipqueue::WriteGrant<'a, 512>,
    queue: Arc<SpscQueue>,
}

impl<'a> WriteGrant<'a> {
    pub fn commit(self, written: usize) {
        self.inner.commit(written);
        self.queue.commit_wait.wake();
    }
}

impl<'a> Deref for WriteGrant<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl<'a> DerefMut for WriteGrant<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.deref_mut()
    }
}

pub struct QueueConsumer {
    queue: Arc<SpscQueue>,
}

impl QueueConsumer {
    pub fn read_sync(&self) -> Result<ReadGrant<'_>, ()> {
        match self.queue.storage.read() {
            Ok(grant) => Ok(ReadGrant { inner: grant, queue: self.queue.clone() }),
            Err(_) => Err(()),
        }
    }

    pub async fn read(&self) -> ReadGrant<'_> {
        loop {
            let wait = self.queue.commit_wait.subscribe().await;
            match self.queue.storage.read() {
                Ok(grant) => return ReadGrant { inner: grant, queue: self.queue.clone() },
                Err(_) => {
                    wait.await.unwrap();
                }
            }
        }
    }
}

pub struct ReadGrant<'a> {
    inner: bipqueue::ReadGrant<'a, 512>,
    queue: Arc<SpscQueue>,
}

impl<'a> ReadGrant<'a> {
    pub fn release(self, read: usize) {
        self.inner.release(read);
        self.queue.release_wait.wake();
    }
}

impl<'a> Deref for ReadGrant<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}
