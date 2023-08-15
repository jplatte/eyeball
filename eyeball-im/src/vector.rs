use std::{
    fmt, ops,
    pin::Pin,
    task::{ready, Context, Poll},
};

use futures_core::Stream;
use imbl::Vector;
use tokio::sync::broadcast::{
    self,
    error::{RecvError, TryRecvError},
    Receiver, Sender,
};
use tokio_util::sync::ReusableBoxFuture;
#[cfg(feature = "tracing")]
use tracing::info;

mod entry;

pub use self::entry::{ObservableVectorEntries, ObservableVectorEntry};

/// An ordered list of elements that broadcasts any changes made to it.
pub struct ObservableVector<T> {
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
        let rx = self.sender.subscribe();
        VectorSubscriber::new(rx)
    }

    /// Append the given elements at the end of the `Vector` and notify
    /// subscribers.
    pub fn append(&mut self, values: Vector<T>) {
        #[cfg(feature = "tracing")]
        tracing::debug!(target: "eyeball_im::vector::update", "append(len = {})", values.len());

        self.values.append(values.clone());
        self.broadcast_diff(VectorDiff::Append { values });
    }

    /// Clear out all of the elements in this `Vector` and notify subscribers.
    pub fn clear(&mut self) {
        #[cfg(feature = "tracing")]
        tracing::debug!(target: "eyeball_im::vector::update", "clear");

        self.values.clear();
        self.broadcast_diff(VectorDiff::Clear);
    }

    /// Add an element at the front of the list and notify subscribers.
    pub fn push_front(&mut self, value: T) {
        #[cfg(feature = "tracing")]
        tracing::debug!(target: "eyeball_im::vector::update", "push_front");

        self.values.push_front(value.clone());
        self.broadcast_diff(VectorDiff::PushFront { value });
    }

    /// Add an element at the back of the list and notify subscribers.
    pub fn push_back(&mut self, value: T) {
        #[cfg(feature = "tracing")]
        tracing::debug!(target: "eyeball_im::vector::update", "push_back");

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
            #[cfg(feature = "tracing")]
            tracing::debug!(target: "eyeball_im::vector::update", "pop_front");

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
            #[cfg(feature = "tracing")]
            tracing::debug!(target: "eyeball_im::vector::update", "pop_back");

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
            #[cfg(feature = "tracing")]
            tracing::debug!(target: "eyeball_im::vector::update", "insert(index = {index})");

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
            #[cfg(feature = "tracing")]
            tracing::debug!(target: "eyeball_im::vector::update", "set(index = {index})");

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
            #[cfg(feature = "tracing")]
            tracing::debug!(target: "eyeball_im::vector::update", "remove(index = {index})");

            let value = self.values.remove(index);
            self.broadcast_diff(VectorDiff::Remove { index });
            value
        } else {
            panic!("index out of bounds: the length is {len} but the index is {index}");
        }
    }

    /// Gets an [`ObservableVectorEntry`] for the given index, through which
    /// only the element at that index alone can be updated or removed.
    ///
    /// # Panics
    ///
    /// Panics if `index >= len`.
    #[track_caller]
    pub fn entry(&mut self, index: usize) -> ObservableVectorEntry<'_, T> {
        let len = self.values.len();
        if index < len {
            ObservableVectorEntry::new(self, index)
        } else {
            panic!("index out of bounds: the length is {len} but the index is {index}");
        }
    }

    /// Call the given closure for every element in this `ObservableVector`,
    /// with an entry struct that allows updating or removing that element.
    ///
    /// Iteration happens in order, i.e. starting at index `0`.
    pub fn for_each(&mut self, mut f: impl FnMut(ObservableVectorEntry<'_, T>)) {
        let mut entries = self.entries();
        while let Some(entry) = entries.next() {
            f(entry);
        }
    }

    /// Get an iterator over all the entries in this `ObservableVector`.
    ///
    /// This is a more flexible, but less convenient alternative to
    /// [`for_each`][Self::for_each]. If you don't need to use special control
    /// flow like `.await` or `break` when iterating, it's recommended to use
    /// that method instead.
    ///
    /// Because `std`'s `Iterator` trait does not allow iterator items to borrow
    /// from the iterator itself, the returned typed does not implement the
    /// `Iterator` trait and can thus not be used with a `for` loop. Instead,
    /// you have to call its `.next()` method directly, as in:
    ///
    /// ```rust
    /// # use eyeball_im::ObservableVector;
    /// # let mut ob = ObservableVector::<u8>::new();
    /// let mut entries = ob.entries();
    /// while let Some(entry) = entries.next() {
    ///     // use entry
    /// }
    /// ```
    pub fn entries(&mut self) -> ObservableVectorEntries<'_, T> {
        ObservableVectorEntries::new(self)
    }

    fn broadcast_diff(&self, diff: VectorDiff<T>) {
        if self.sender.receiver_count() != 0 {
            let msg = BroadcastMessage { diff, state: self.values.clone() };
            let _num_receivers = self.sender.send(msg).unwrap_or(0);
            #[cfg(feature = "tracing")]
            tracing::debug!(
                target: "eyeball_im::vector::broadcast",
                "New observable value broadcast to {_num_receivers} receivers"
            );
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
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObservableVector").field("values", &self.values).finish_non_exhaustive()
    }
}

// Note: No DerefMut because all mutating must go through inherent methods that
// notify subscribers
impl<T> ops::Deref for ObservableVector<T> {
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
struct BroadcastMessage<T> {
    diff: VectorDiff<T>,
    state: Vector<T>,
}

/// A subscriber for updates of a [`Vector`].
///
/// Use its [`Stream`] implementation to interact with it (futures-util and
/// other futures-related crates have extension traits with convenience
/// methods).
#[derive(Debug)]
pub struct VectorSubscriber<T> {
    inner: ReusableBoxFuture<'static, SubscriberFutureReturn<BroadcastMessage<T>>>,
}

impl<T: Clone + Send + Sync + 'static> VectorSubscriber<T> {
    fn new(rx: Receiver<BroadcastMessage<T>>) -> Self {
        Self { inner: ReusableBoxFuture::new(make_future(rx)) }
    }
}

impl<T: Clone + Send + Sync + 'static> Stream for VectorSubscriber<T> {
    type Item = VectorDiff<T>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let (result, mut rx) = ready!(self.inner.poll(cx));

        let poll = match result {
            Ok(msg) => Poll::Ready(Some(msg.diff)),
            Err(RecvError::Closed) => Poll::Ready(None),
            Err(RecvError::Lagged(_)) => {
                let mut msg = None;
                loop {
                    match rx.try_recv() {
                        // There's a newer message in the receiver's buffer, use that for reset.
                        Ok(m) => {
                            msg = Some(m);
                        }
                        // Ideally we'd return a `VecDiff::Reset` with the last state before the
                        // channel was closed here, but we have no way of obtaining the last state.
                        Err(TryRecvError::Closed) => {
                            #[cfg(feature = "tracing")]
                            info!("Channel closed after lag, can't return last state");
                            break Poll::Ready(None);
                        }
                        // Lagged twice in a row, is this possible? If it is, it's fine to just
                        // loop again and look at the next try_recv result.
                        Err(TryRecvError::Lagged(_)) => {}
                        Err(TryRecvError::Empty) => match msg {
                            // We exhausted the internal buffer using try_recv, msg contains the
                            // last message from it, which we use for the reset.
                            Some(msg) => {
                                break Poll::Ready(Some(VectorDiff::Reset { values: msg.state }));
                            }
                            // We exhausted the internal buffer using try_recv but there was no
                            // message in it, even though we got TryRecvError::Lagged(_) before.
                            None => unreachable!("got no new message via try_recv after lag"),
                        },
                    }
                }
            }
        };

        self.inner.set(make_future(rx));
        poll
    }
}

type SubscriberFutureReturn<T> = (Result<T, RecvError>, Receiver<T>);

async fn make_future<T: Clone>(mut rx: Receiver<T>) -> SubscriberFutureReturn<T> {
    let result = rx.recv().await;
    (result, rx)
}

/// A change to an [`ObservableVector`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VectorDiff<T> {
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

impl<T: Clone> VectorDiff<T> {
    /// Transform `VectorDiff<T>` into `VectorDiff<U>` by applying the given
    /// function to any contained items.
    pub fn map<U: Clone>(self, mut f: impl FnMut(T) -> U) -> VectorDiff<U> {
        match self {
            VectorDiff::Append { values } => VectorDiff::Append { values: vector_map(values, f) },
            VectorDiff::Clear => VectorDiff::Clear,
            VectorDiff::PushFront { value } => VectorDiff::PushFront { value: f(value) },
            VectorDiff::PushBack { value } => VectorDiff::PushBack { value: f(value) },
            VectorDiff::PopFront => VectorDiff::PopFront,
            VectorDiff::PopBack => VectorDiff::PopBack,
            VectorDiff::Insert { index, value } => VectorDiff::Insert { index, value: f(value) },
            VectorDiff::Set { index, value } => VectorDiff::Set { index, value: f(value) },
            VectorDiff::Remove { index } => VectorDiff::Remove { index },
            VectorDiff::Reset { values } => VectorDiff::Reset { values: vector_map(values, f) },
        }
    }
}

fn vector_map<T: Clone, U: Clone>(v: Vector<T>, f: impl FnMut(T) -> U) -> Vector<U> {
    v.into_iter().map(f).collect()
}
