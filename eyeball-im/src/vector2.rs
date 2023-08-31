use std::{
    fmt, mem, ops,
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

use crate::VectorDiff;

pub use self::entry::{ObservableVector2Entries, ObservableVector2Entry};

/// An ordered list of elements that broadcasts any changes made to it.
pub struct ObservableVector2<T> {
    values: Vector<T>,
    sender: Sender<BroadcastMessage<T>>,
}

impl<T: Clone + Send + Sync + 'static> ObservableVector2<T> {
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
    pub fn subscribe(&self) -> VectorSubscriber2<T> {
        let rx = self.sender.subscribe();
        VectorSubscriber2::new(rx)
    }

    pub fn write(&mut self) -> ObservableVector2WriteGuard<'_, T> {
        ObservableVector2WriteGuard::new(self)
    }
}

impl<T: Clone + Send + Sync + 'static> Default for ObservableVector2<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> fmt::Debug for ObservableVector2<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObservableVector").field("values", &self.values).finish_non_exhaustive()
    }
}

impl<T> ops::Deref for ObservableVector2<T> {
    type Target = Vector<T>;

    fn deref(&self) -> &Self::Target {
        &self.values
    }
}

impl<T: Clone + Send + Sync + 'static> From<Vector<T>> for ObservableVector2<T> {
    fn from(values: Vector<T>) -> Self {
        let mut this = Self::new();
        this.write().append(values);
        this
    }
}

pub struct ObservableVector2WriteGuard<'o, T: Clone> {
    inner: &'o mut ObservableVector2<T>,
    batch: Vec<VectorDiff<T>>,
}

impl<'o, T: Clone + Send + Sync + 'static> ObservableVector2WriteGuard<'o, T> {
    fn new(inner: &'o mut ObservableVector2<T>) -> Self {
        Self { inner, batch: Vec::new() }
    }

    /// Append the given elements at the end of the `Vector` and notify
    /// subscribers.
    pub fn append(&mut self, values: Vector<T>) {
        #[cfg(feature = "tracing")]
        tracing::debug!(target: "eyeball_im::vector::update", "append(len = {})", values.len());

        self.inner.values.append(values.clone());
        self.add_to_batch(VectorDiff::Append { values });
    }

    /// Clear out all of the elements in this `Vector` and notify subscribers.
    pub fn clear(&mut self) {
        #[cfg(feature = "tracing")]
        tracing::debug!(target: "eyeball_im::vector::update", "clear");

        self.inner.values.clear();
        self.add_to_batch(VectorDiff::Clear);
    }

    /// Add an element at the front of the list and notify subscribers.
    pub fn push_front(&mut self, value: T) {
        #[cfg(feature = "tracing")]
        tracing::debug!(target: "eyeball_im::vector::update", "push_front");

        self.inner.values.push_front(value.clone());
        self.add_to_batch(VectorDiff::PushFront { value });
    }

    /// Add an element at the back of the list and notify subscribers.
    pub fn push_back(&mut self, value: T) {
        #[cfg(feature = "tracing")]
        tracing::debug!(target: "eyeball_im::vector::update", "push_back");

        self.inner.values.push_back(value.clone());
        self.add_to_batch(VectorDiff::PushBack { value });
    }

    /// Remove the first element, notify subscribers and return the element.
    ///
    /// If there are no elements, subscribers will not be notified and this
    /// method will return `None`.
    pub fn pop_front(&mut self) -> Option<T> {
        let value = self.inner.values.pop_front();
        if value.is_some() {
            #[cfg(feature = "tracing")]
            tracing::debug!(target: "eyeball_im::vector::update", "pop_front");

            self.add_to_batch(VectorDiff::PopFront);
        }
        value
    }

    /// Remove the last element, notify subscribers and return the element.
    ///
    /// If there are no elements, subscribers will not be notified and this
    /// method will return `None`.
    pub fn pop_back(&mut self) -> Option<T> {
        let value = self.inner.values.pop_back();
        if value.is_some() {
            #[cfg(feature = "tracing")]
            tracing::debug!(target: "eyeball_im::vector::update", "pop_back");

            self.add_to_batch(VectorDiff::PopBack);
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
        let len = self.inner.values.len();
        if index <= len {
            #[cfg(feature = "tracing")]
            tracing::debug!(target: "eyeball_im::vector::update", "insert(index = {index})");

            self.inner.values.insert(index, value.clone());
            self.add_to_batch(VectorDiff::Insert { index, value });
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
        let len = self.inner.values.len();
        if index < len {
            #[cfg(feature = "tracing")]
            tracing::debug!(target: "eyeball_im::vector::update", "set(index = {index})");

            let old_value = self.inner.values.set(index, value.clone());
            self.add_to_batch(VectorDiff::Set { index, value });
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
        let len = self.inner.values.len();
        if index < len {
            #[cfg(feature = "tracing")]
            tracing::debug!(target: "eyeball_im::vector::update", "remove(index = {index})");

            let value = self.inner.values.remove(index);
            self.add_to_batch(VectorDiff::Remove { index });
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
    pub fn entry(&mut self, index: usize) -> ObservableVector2Entry<'_, 'o, T> {
        let len = self.inner.values.len();
        if index < len {
            ObservableVector2Entry::new(self, index)
        } else {
            panic!("index out of bounds: the length is {len} but the index is {index}");
        }
    }

    /// Call the given closure for every element in this `ObservableVector`,
    /// with an entry struct that allows updating or removing that element.
    ///
    /// Iteration happens in order, i.e. starting at index `0`.
    pub fn for_each(&mut self, mut f: impl FnMut(ObservableVector2Entry<'_, 'o, T>)) {
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
    pub fn entries(&mut self) -> ObservableVector2Entries<'_, 'o, T> {
        ObservableVector2Entries::new(self)
    }

    fn add_to_batch(&mut self, diff: VectorDiff<T>) {
        if self.inner.sender.receiver_count() != 0 {
            self.batch.push(diff);
        }
    }
}

impl<T: Clone> Drop for ObservableVector2WriteGuard<'_, T> {
    fn drop(&mut self) {
        if self.batch.is_empty() {
            #[cfg(feature = "tracing")]
            tracing::trace!(
                target: "eyeball_im::vector::broadcast",
                "Skipping broadcast of empty list of diffs"
            );
        } else {
            let diffs = mem::take(&mut self.batch);
            let msg = BroadcastMessage { diffs, state: self.inner.values.clone() };
            let _num_receivers = self.inner.sender.send(msg).unwrap_or(0);
            #[cfg(feature = "tracing")]
            tracing::debug!(
                target: "eyeball_im::vector::broadcast",
                "New observable value broadcast to {_num_receivers} receivers"
            );
        }
    }
}

impl<T> fmt::Debug for ObservableVector2WriteGuard<'_, T>
where
    T: Clone + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObservableVectorWriteGuard")
            .field("values", &self.inner.values)
            .finish_non_exhaustive()
    }
}

// Note: No DerefMut because all mutating must go through inherent methods that
// notify subscribers
impl<T: Clone> ops::Deref for ObservableVector2WriteGuard<'_, T> {
    type Target = Vector<T>;

    fn deref(&self) -> &Self::Target {
        &self.inner.values
    }
}

#[derive(Clone)]
struct BroadcastMessage<T> {
    diffs: Vec<VectorDiff<T>>,
    state: Vector<T>,
}

/// A subscriber for updates of a [`Vector`].
///
/// Use its [`Stream`] implementation to interact with it (futures-util and
/// other futures-related crates have extension traits with convenience
/// methods).
#[derive(Debug)]
pub struct VectorSubscriber2<T> {
    inner: ReusableBoxFuture<'static, SubscriberFutureReturn<BroadcastMessage<T>>>,
}

impl<T: Clone + Send + Sync + 'static> VectorSubscriber2<T> {
    fn new(rx: Receiver<BroadcastMessage<T>>) -> Self {
        Self { inner: ReusableBoxFuture::new(make_future(rx)) }
    }
}

impl<T: Clone + Send + Sync + 'static> Stream for VectorSubscriber2<T> {
    type Item = Vec<VectorDiff<T>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let (result, mut rx) = ready!(self.inner.poll(cx));

        let poll = match result {
            Ok(msg) => Poll::Ready(Some(msg.diffs)),
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
                                break Poll::Ready(Some(vec![VectorDiff::Reset {
                                    values: msg.state,
                                }]));
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
