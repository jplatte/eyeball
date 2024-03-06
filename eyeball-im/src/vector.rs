use std::{fmt, ops};

use imbl::Vector;
use tokio::sync::broadcast::{self, Sender};

mod entry;
mod subscriber;
mod transaction;

pub use self::{
    entry::{ObservableVectorEntries, ObservableVectorEntry},
    subscriber::{VectorSubscriber, VectorSubscriberBatchedStream, VectorSubscriberStream},
    transaction::{
        ObservableVectorTransaction, ObservableVectorTransactionEntries,
        ObservableVectorTransactionEntry,
    },
};

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
        VectorSubscriber::new(self.values.clone(), rx)
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
        let already_empty = self.values.is_empty();

        #[cfg(feature = "tracing")]
        tracing::debug!(
            target: "eyeball_im::vector::update",
            nop = already_empty.then_some(true),
            "clear"
        );

        if !already_empty {
            self.values.clear();
            self.broadcast_diff(VectorDiff::Clear);
        }
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

    /// Truncate the vector to `len` elements and notify subscribers.
    ///
    /// Does nothing if `len` is greater or equal to the vector's current
    /// length.
    pub fn truncate(&mut self, len: usize) {
        if len < self.len() {
            #[cfg(feature = "tracing")]
            tracing::debug!(target: "eyeball_im::vector::update", "truncate(len = {len})");

            self.values.truncate(len);
            self.broadcast_diff(VectorDiff::Truncate { length: len });
        }
    }

    /// Gets an entry for the given index, through which only the element at
    /// that index alone can be updated or removed.
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

    /// Start a new transaction to make multiple updates as one unit.
    ///
    /// See [`ObservableVectorTransaction`]s documentation for more details.
    pub fn transaction(&mut self) -> ObservableVectorTransaction<'_, T> {
        ObservableVectorTransaction::new(self)
    }

    fn broadcast_diff(&self, diff: VectorDiff<T>) {
        if self.sender.receiver_count() != 0 {
            let msg =
                BroadcastMessage { diffs: OneOrManyDiffs::One(diff), state: self.values.clone() };
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
    diffs: OneOrManyDiffs<T>,
    state: Vector<T>,
}

#[derive(Clone)]
enum OneOrManyDiffs<T> {
    One(VectorDiff<T>),
    Many(Vec<VectorDiff<T>>),
}

impl<T> OneOrManyDiffs<T> {
    fn into_vec(self) -> Vec<VectorDiff<T>> {
        match self {
            OneOrManyDiffs::One(diff) => vec![diff],
            OneOrManyDiffs::Many(diffs) => diffs,
        }
    }
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
    /// Truncation of the vector.
    Truncate {
        /// The number of elements that remain.
        length: usize,
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
            VectorDiff::Truncate { length } => VectorDiff::Truncate { length },
            VectorDiff::Reset { values } => VectorDiff::Reset { values: vector_map(values, f) },
        }
    }

    /// Applies this [`VectorDiff`] to a vector.
    ///
    /// This is useful to keep two vectors in sync, with potentially one
    /// containing data [`map`](Self::map)ped from the other.
    ///
    /// # Panics
    ///
    /// When inserting/setting/removing elements past the end.
    pub fn apply(self, vec: &mut Vector<T>) {
        match self {
            VectorDiff::Append { values } => {
                vec.extend(values);
            }
            VectorDiff::Clear => {
                vec.clear();
            }
            VectorDiff::PushFront { value } => {
                vec.push_front(value);
            }
            VectorDiff::PushBack { value } => {
                vec.push_back(value);
            }
            VectorDiff::PopFront => {
                vec.pop_front();
            }
            VectorDiff::PopBack => {
                vec.pop_back();
            }
            VectorDiff::Insert { index, value } => {
                vec.insert(index, value);
            }
            VectorDiff::Set { index, value } => {
                vec.set(index, value);
            }
            VectorDiff::Remove { index } => {
                vec.remove(index);
            }
            VectorDiff::Truncate { length } => {
                vec.truncate(length);
            }
            VectorDiff::Reset { values } => {
                *vec = values;
            }
        }
    }
}

#[cfg(feature = "serde")]
impl<T> serde::Serialize for VectorDiff<T>
where
    T: serde::Serialize + Clone,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStructVariant;

        const SELF_NAME: &str = "VectorDiff";

        match self {
            Self::Append { values } => {
                let mut state = serializer.serialize_struct_variant(SELF_NAME, 0, "Append", 1)?;
                state.serialize_field("values", values)?;
                state.end()
            }
            VectorDiff::Clear => {
                serializer.serialize_struct_variant(SELF_NAME, 1, "Clear", 0)?.end()
            }
            VectorDiff::PushFront { value } => {
                let mut state =
                    serializer.serialize_struct_variant(SELF_NAME, 2, "PushFront", 1)?;
                state.serialize_field("value", value)?;
                state.end()
            }
            VectorDiff::PushBack { value } => {
                let mut state = serializer.serialize_struct_variant(SELF_NAME, 3, "PushBack", 1)?;
                state.serialize_field("value", value)?;
                state.end()
            }
            VectorDiff::PopFront => {
                serializer.serialize_struct_variant(SELF_NAME, 4, "PopFront", 0)?.end()
            }
            VectorDiff::PopBack => {
                serializer.serialize_struct_variant(SELF_NAME, 5, "PopBack", 0)?.end()
            }
            VectorDiff::Insert { index, value } => {
                let mut state = serializer.serialize_struct_variant(SELF_NAME, 6, "Insert", 2)?;
                state.serialize_field("index", index)?;
                state.serialize_field("value", value)?;
                state.end()
            }
            VectorDiff::Set { index, value } => {
                let mut state = serializer.serialize_struct_variant(SELF_NAME, 7, "Set", 2)?;
                state.serialize_field("index", index)?;
                state.serialize_field("value", value)?;
                state.end()
            }
            VectorDiff::Remove { index } => {
                let mut state = serializer.serialize_struct_variant(SELF_NAME, 8, "Remove", 1)?;
                state.serialize_field("index", index)?;
                state.end()
            }
            VectorDiff::Truncate { length } => {
                let mut state = serializer.serialize_struct_variant(SELF_NAME, 9, "Truncate", 1)?;
                state.serialize_field("length", length)?;
                state.end()
            }
            VectorDiff::Reset { values } => {
                let mut state = serializer.serialize_struct_variant(SELF_NAME, 10, "Reset", 1)?;
                state.serialize_field("values", values)?;
                state.end()
            }
        }
    }
}

fn vector_map<T: Clone, U: Clone>(v: Vector<T>, f: impl FnMut(T) -> U) -> Vector<U> {
    v.into_iter().map(f).collect()
}
