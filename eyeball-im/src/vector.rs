use std::{
    fmt, ops,
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::Stream;
use im::Vector;
use tokio::sync::broadcast::{self, Sender};
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};

/// An ordered list of elements that broadcasts any changes made to it.
pub struct ObservableVector<T: Clone> {
    values: Vector<T>,
    sender: Sender<BroadcastMessage<T>>,
}

impl<T: Clone + Send + Sync + 'static> ObservableVector<T> {
    /// Create a new `ObservableVector`.
    ///
    /// As of the time of writing, this is equivalent to
    /// `ObservableVector::with_capacity(16)`, but the internal buffer capacity
    /// is subject to change in non-breaking releases.
    ///
    /// See [`with_capacity`][Self::with_capacity] for details about the buffer
    /// capacity.
    pub fn new() -> Self {
        Self::with_capacity(16)
    }

    /// Create a new `ObservableVector` with the given capacity for the inner
    /// buffer.
    ///
    /// Up to `capacity` updates that have not been received by all of the
    /// subscribers yet will be retained in the inner buffer. If an update
    /// happens while the buffer is at capacity, the oldest update is discarded
    /// from it and all subscribers that have not yet received it will instead
    /// see [`VectorDiff::Reset`] as the next update.
    ///
    /// # Panics
    ///
    /// Panics if the capacity is `0`, or larger than `usize::MAX / 2`.
    pub fn with_capacity(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { values: Vector::new(), sender }
    }

    /// Turn the `ObservableVector` back into a regular `Vector`.
    pub fn into_inner(self) -> Vector<T> {
        self.values
    }

    /// Obtain a new subscriber.
    ///
    /// If you put the `ObservableVector` behind a lock, it is highly
    /// recommended to make access of the elements and subscribing one
    /// operation. Otherwise, the values could be altered in between the
    /// reading of the values and subscribing to changes.
    pub fn subscribe(&self) -> VectorSubscriber<T> {
        let stream = BroadcastStream::new(self.sender.subscribe());
        VectorSubscriber::new(stream)
    }

    /// Append the given elements at the end of the `Vector` and notify
    /// subscribers.
    pub fn append(&mut self, values: Vector<T>) {
        self.values.append(values.clone());
        self.broadcast_diff(VectorDiff::Append { values });
    }

    /// Clear out all of the elements in this `Vector` and notify subscribers.
    pub fn clear(&mut self) {
        self.values.clear();
        self.broadcast_diff(VectorDiff::Clear);
    }

    /// Add an element at the front of the list and notify subscribers.
    pub fn push_front(&mut self, value: T) {
        self.values.push_front(value.clone());
        self.broadcast_diff(VectorDiff::PushFront { value });
    }

    /// Add an element at the back of the list and notify subscribers.
    pub fn push_back(&mut self, value: T) {
        self.values.push_back(value.clone());
        self.broadcast_diff(VectorDiff::PushBack { value });
    }

    /// Remove the first element, notify subscribers and return the element.
    ///
    /// If there are no elements, subscribers will not be notified and this
    /// method will return `None`.
    pub fn pop_front(&mut self) -> Option<T> {
        let value = self.values.pop_front();
        if value.is_some() {
            self.broadcast_diff(VectorDiff::PopFront);
        }
        value
    }

    /// Remove the last element, notify subscribers and return the element.
    ///
    /// If there are no elements, subscribers will not be notified and this
    /// method will return `None`.
    pub fn pop_back(&mut self) -> Option<T> {
        let value = self.values.pop_back();
        if value.is_some() {
            self.broadcast_diff(VectorDiff::PopBack);
        }
        value
    }

    /// Insert an element at the given position and notify subscribers.
    ///
    /// # Panics
    ///
    /// Panics if `index > len`.
    #[track_caller]
    pub fn insert(&mut self, index: usize, value: T) {
        let len = self.values.len();
        if index <= len {
            self.values.insert(index, value.clone());
            self.broadcast_diff(VectorDiff::Insert { index, value });
        } else {
            panic!("index out of bounds: the length is {len} but the index is {index}");
        }
    }

    /// Replace the element at the given position, notify subscribers and return
    /// the previous element at that position.
    ///
    /// # Panics
    ///
    /// Panics if `index > len`.
    #[track_caller]
    pub fn set(&mut self, index: usize, value: T) -> T {
        let len = self.values.len();
        if index < len {
            let old_value = self.values.set(index, value.clone());
            self.broadcast_diff(VectorDiff::Set { index, value });
            old_value
        } else {
            panic!("index out of bounds: the length is {len} but the index is {index}");
        }
    }

    /// Remove the element at the given position, notify subscribers and return
    /// the element.
    ///
    /// # Panics
    ///
    /// Panics if `index >= len`.
    #[track_caller]
    pub fn remove(&mut self, index: usize) -> T {
        let len = self.values.len();
        if index < len {
            let value = self.values.remove(index);
            self.broadcast_diff(VectorDiff::Remove { index });
            value
        } else {
            panic!("index out of bounds: the length is {len} but the index is {index}");
        }
    }

    fn broadcast_diff(&self, diff: VectorDiff<T>) {
        if self.sender.receiver_count() != 0 {
            let msg = BroadcastMessage { diff, state: self.values.clone() };
            let _num_receivers = self.sender.send(msg).unwrap_or(0);
            #[cfg(feature = "tracing")]
            tracing::debug!("New observable value broadcast to {_num_receivers} receivers");
        }
    }
}

impl<T: Clone + Send + Sync + 'static> Default for ObservableVector<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> fmt::Debug for ObservableVector<T>
where
    T: Clone + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObservableVector").field("values", &self.values).finish_non_exhaustive()
    }
}

// Note: No DerefMut because all mutating must go through inherent methods that
// notify subscribers
impl<T: Clone> ops::Deref for ObservableVector<T> {
    type Target = Vector<T>;

    fn deref(&self) -> &Self::Target {
        &self.values
    }
}

impl<T: Clone + Send + Sync + 'static> From<Vector<T>> for ObservableVector<T> {
    fn from(values: Vector<T>) -> Self {
        let mut this = Self::new();
        this.append(values);
        this
    }
}

#[derive(Clone)]
struct BroadcastMessage<T: Clone> {
    diff: VectorDiff<T>,
    state: Vector<T>,
}

/// A subscriber for updates of a [`Vector`].
///
/// Use its [`Stream`] implementation to interact with it (futures-util and
/// other futures-related crates have extension traits with convenience
/// methods).
#[derive(Debug)]
pub struct VectorSubscriber<T: Clone> {
    inner: BroadcastStream<BroadcastMessage<T>>,
    must_reset: bool,
}

impl<T: Clone> VectorSubscriber<T> {
    const fn new(inner: BroadcastStream<BroadcastMessage<T>>) -> Self {
        Self { inner, must_reset: false }
    }
}

impl<T: Clone + Send + Sync + 'static> Stream for VectorSubscriber<T> {
    type Item = VectorDiff<T>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            let poll = match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(msg))) => {
                    let diff = if self.must_reset {
                        self.must_reset = false;
                        VectorDiff::Reset { values: msg.state }
                    } else {
                        msg.diff
                    };
                    Poll::Ready(Some(diff))
                }
                Poll::Ready(None) => Poll::Ready(None),
                Poll::Ready(Some(Err(BroadcastStreamRecvError::Lagged(_)))) => {
                    self.must_reset = true;
                    continue;
                }
                Poll::Pending => Poll::Pending,
            };

            return poll;
        }
    }
}

/// A change to a [`ObservableVector`].
#[derive(Clone, Debug, PartialEq, Eq)]
// FIXME: Clone bound currently needed for derived `impl Debug for Vector<T>`
pub enum VectorDiff<T: Clone> {
    /// Multiple elements were appended.
    Append {
        /// The appended elements.
        values: Vector<T>,
    },
    /// The vector was cleared.
    Clear,
    /// An element was added at the front.
    PushFront {
        /// The new element.
        value: T,
    },
    /// An element was added at the back.
    PushBack {
        /// The new element.
        value: T,
    },
    /// The element at the front was removed.
    PopFront,
    /// The element at the back was removed.
    PopBack,
    /// An element was inserted at the given position.
    Insert {
        /// The index of the new element.
        ///
        /// The element that was previously at that index as well as all the
        /// ones after it were shifted to the right.
        index: usize,
        /// The new element.
        value: T,
    },
    /// A replacement of the previous value at the given position.
    Set {
        /// The index of the element that was replaced.
        index: usize,
        /// The new element.
        value: T,
    },
    /// Removal of an element.
    Remove {
        /// The index that the removed element had.
        index: usize,
    },
    /// The subscriber lagged too far behind, and the next update that should
    /// have been received has already been discarded from the internal buffer.
    Reset {
        /// The full list of elements.
        values: Vector<T>,
    },
}
