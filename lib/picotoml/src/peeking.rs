use core::iter::FusedIterator;

/// Default stack size for ArrayVec.
#[cfg(not(feature = "alloc"))]
const DEFAULT_STACK_SIZE: usize = 8;

pub struct PeekingIterator<I: Iterator> {
    /// The underlying iterator. Consumption of this inner iterator does not represent consumption of the
    /// `PeekingIterator`.
    iterator: I,

    /// The queue represents the items of our iterator which have not been consumed, but can be peeked
    /// at without consuming them. Once an element has been consumed by the iterator, the element will
    /// be dequeued and it will no longer be possible to peek at this element.
    #[cfg(feature = "alloc")]
    queue: alloc::vec::Vec<Option<I::Item>>,
    #[cfg(not(feature = "alloc"))]
    queue: heapless::Vec<Option<I::Item>, DEFAULT_STACK_SIZE>,

    /// The cursor points to the element we are currently peeking at.
    ///
    /// The cursor will point to the first unconsumed element if the value is `0`, the second if it is
    /// `1`, and so forth. Peeking at the 0th cursor element is equivalent to peeking with
    /// [`core::iter::Peekable::peek`].
    ///
    /// [`core::iter::Peekable::peek`]: https://doc.rust-lang.org/core/iter/struct.Peekable.html#method.peek
    cursor: usize,
}

impl<I: Iterator> PeekingIterator<I> {
    pub fn new(iterator: I) -> PeekingIterator<I> {
        PeekingIterator { iterator, queue: Default::default(), cursor: 0 }
    }

    #[inline]
    pub fn peek(&mut self) -> Option<&I::Item> {
        self.fill_queue(self.cursor);
        self.queue.get(self.cursor).and_then(|v| v.as_ref())
    }

    /// Advance the cursor to the next element and return a reference to that value.
    #[inline]
    pub fn peek_next(&mut self) -> Option<&I::Item> {
        let this = self.advance_cursor();
        this.peek()
    }

    /// Advance the cursor to the next peekable element.
    ///
    /// This method does not advance the iterator itself. To advance the iterator, call [`next()`]
    /// instead.
    ///
    /// A mutable reference to the iterator is returned, which allows the operation to be chained.
    ///
    /// [`next()`]: struct.PeekingIterator.html#impl-Iterator
    #[inline]
    pub fn advance_cursor(&mut self) -> &mut PeekingIterator<I> {
        self.increment_cursor();
        self
    }

    /// Reset the position of the cursor.
    ///
    /// If [`peek`] is called just after a reset, it will return a reference to the first element.
    ///
    /// [`peek`]: struct.PeekingIterator.html#method.peek
    #[inline]
    pub fn reset_cursor(&mut self) {
        self.cursor = 0;
    }

    /// Fills the queue up to (and including) the cursor.
    #[inline]
    fn fill_queue(&mut self, required_elements: usize) {
        let stored_elements = self.queue.len();

        if stored_elements <= required_elements {
            for _ in stored_elements..=required_elements {
                self.push_next_to_queue()
            }
        }
    }

    /// Consume the underlying iterator and push an element to the queue.
    #[inline]
    fn push_next_to_queue(&mut self) {
        let item = self.iterator.next();
        #[cfg(feature = "alloc")]
        self.queue.push(item);
        #[cfg(not(feature = "alloc"))]
        self.queue.push(item).ok();
    }

    /// Increment the cursor which points to the current peekable item.
    /// Note: if the cursor is [core::usize::MAX], it will not increment any further.
    ///
    /// [core::usize::MAX]: https://doc.rust-lang.org/core/usize/constant.MAX.html
    #[inline]
    fn increment_cursor(&mut self) {
        self.cursor = self.cursor.saturating_add(1);
    }

    #[inline]
    fn decrement_cursor(&mut self) {
        if self.cursor > core::usize::MIN {
            self.cursor -= 1;
        }
    }

    #[inline]
    pub fn inner(&self) -> &I {
        &self.iterator
    }
}

impl<I: Iterator> Iterator for PeekingIterator<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let res = if self.queue.is_empty() { self.iterator.next() } else { self.queue.remove(0) };
        self.decrement_cursor();
        res
    }
}

impl<I: ExactSizeIterator> ExactSizeIterator for PeekingIterator<I> {}
impl<I: FusedIterator> FusedIterator for PeekingIterator<I> {}
