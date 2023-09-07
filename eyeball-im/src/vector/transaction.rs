use std::{fmt, mem, ops};

use imbl::Vector;

use crate::vector::OneOrManyDiffs;

use super::{entry::EntryIndex, BroadcastMessage, ObservableVector, VectorDiff};

/// A transaction that allows making multiple updates to an `ObservableVector`
/// as an atomic unit.
///
/// For updates from the transaction to have affect, it has to be finalized with
/// [`.commit()`](Self::commit). If the transaction is dropped without that
/// method being called, the updates will be discarded.
pub struct ObservableVectorTransaction<'o, T: Clone> {
    // The observable vector being modified, only modified on commit.
    inner: &'o mut ObservableVector<T>,
    // A clone of the observable's values, what the methods operate on until commit.
    values: Vector<T>,
    // The batched updates, to be sent to subscribers on commit.
    batch: Vec<VectorDiff<T>>,
}

impl<'o, T: Clone + Send + Sync + 'static> ObservableVectorTransaction<'o, T> {
    pub(super) fn new(inner: &'o mut ObservableVector<T>) -> Self {
        let values = inner.values.clone();
        Self { inner, values, batch: Vec::new() }
    }

    /// Commit this transaction, persisting the changes and notifying
    /// subscribers.
    pub fn commit(mut self) {
        #[cfg(feature = "tracing")]
        tracing::debug!("commit");

        self.inner.values = mem::take(&mut self.values);

        if self.batch.is_empty() {
            #[cfg(feature = "tracing")]
            tracing::trace!(
                target: "eyeball_im::vector::broadcast",
                "Skipping broadcast of empty list of diffs"
            );
        } else {
            let diffs = OneOrManyDiffs::Many(mem::take(&mut self.batch));
            let msg = BroadcastMessage { diffs, state: self.inner.values.clone() };
            let _num_receivers = self.inner.sender.send(msg).unwrap_or(0);
            #[cfg(feature = "tracing")]
            tracing::debug!(
                target: "eyeball_im::vector::broadcast",
                "New observable value broadcast to {_num_receivers} receivers"
            );
        }
    }

    /// Roll back all changes made using this transaction so far.
    ///
    /// Same as dropping the transaction and starting a new one, semantically.
    pub fn rollback(&mut self) {
        #[cfg(feature = "tracing")]
        tracing::debug!("rollback (explicit)");

        self.values = self.inner.values.clone();
        self.batch.clear();
    }

    /// Append the given elements at the end of the `Vector` and notify
    /// subscribers.
    pub fn append(&mut self, values: Vector<T>) {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            target: "eyeball_im::vector::transaction::update",
            "append(len = {})", values.len()
        );

        self.values.append(values.clone());
        self.add_to_batch(VectorDiff::Append { values });
    }

    /// Clear out all of the elements in this `Vector` and notify subscribers.
    pub fn clear(&mut self) {
        #[cfg(feature = "tracing")]
        tracing::debug!(target: "eyeball_im::vector::transaction::update", "clear");

        self.values.clear();
        self.batch.clear(); // All previous batched updates are irrelevant now
        self.add_to_batch(VectorDiff::Clear);
    }

    /// Add an element at the front of the list and notify subscribers.
    pub fn push_front(&mut self, value: T) {
        #[cfg(feature = "tracing")]
        tracing::debug!(target: "eyeball_im::vector::transaction::update", "push_front");

        self.values.push_front(value.clone());
        self.add_to_batch(VectorDiff::PushFront { value });
    }

    /// Add an element at the back of the list and notify subscribers.
    pub fn push_back(&mut self, value: T) {
        #[cfg(feature = "tracing")]
        tracing::debug!(target: "eyeball_im::vector::transaction::update", "push_back");

        self.values.push_back(value.clone());
        self.add_to_batch(VectorDiff::PushBack { value });
    }

    /// Remove the first element, notify subscribers and return the element.
    ///
    /// If there are no elements, subscribers will not be notified and this
    /// method will return `None`.
    pub fn pop_front(&mut self) -> Option<T> {
        let value = self.values.pop_front();
        if value.is_some() {
            #[cfg(feature = "tracing")]
            tracing::debug!(target: "eyeball_im::vector::transaction::update", "pop_front");

            self.add_to_batch(VectorDiff::PopFront);
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
            tracing::debug!(target: "eyeball_im::vector::transaction::update", "pop_back");

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
        let len = self.values.len();
        if index <= len {
            #[cfg(feature = "tracing")]
            tracing::debug!(
                target: "eyeball_im::vector::transaction::update",
                "insert(index = {index})"
            );

            self.values.insert(index, value.clone());
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
        let len = self.values.len();
        if index < len {
            #[cfg(feature = "tracing")]
            tracing::debug!(
                target: "eyeball_im::vector::transaction::update",
                "set(index = {index})"
            );

            let old_value = self.values.set(index, value.clone());
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
        let len = self.values.len();
        if index < len {
            #[cfg(feature = "tracing")]
            tracing::debug!(
                target: "eyeball_im::vector::transaction::update",
                "remove(index = {index})"
            );

            let value = self.values.remove(index);
            self.add_to_batch(VectorDiff::Remove { index });
            value
        } else {
            panic!("index out of bounds: the length is {len} but the index is {index}");
        }
    }

    /// Gets an entry for the given index through which only the element at that
    /// index alone can be updated or removed.
    ///
    /// # Panics
    ///
    /// Panics if `index >= len`.
    #[track_caller]
    pub fn entry(&mut self, index: usize) -> ObservableVectorTransactionEntry<'_, 'o, T> {
        let len = self.values.len();
        if index < len {
            ObservableVectorTransactionEntry::new(self, index)
        } else {
            panic!("index out of bounds: the length is {len} but the index is {index}");
        }
    }

    /// Call the given closure for every element in this `ObservableVector`,
    /// with an entry struct that allows updating or removing that element.
    ///
    /// Iteration happens in order, i.e. starting at index `0`.
    pub fn for_each(&mut self, mut f: impl FnMut(ObservableVectorTransactionEntry<'_, 'o, T>)) {
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
    pub fn entries(&mut self) -> ObservableVectorTransactionEntries<'_, 'o, T> {
        ObservableVectorTransactionEntries::new(self)
    }

    fn add_to_batch(&mut self, diff: VectorDiff<T>) {
        if self.inner.sender.receiver_count() != 0 {
            self.batch.push(diff);
        }
    }
}

impl<T> fmt::Debug for ObservableVectorTransaction<'_, T>
where
    T: Clone + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObservableVectorWriteGuard")
            .field("values", &self.values)
            .finish_non_exhaustive()
    }
}

// Note: No DerefMut because all mutating must go through inherent methods that
// notify subscribers
impl<T: Clone> ops::Deref for ObservableVectorTransaction<'_, T> {
    type Target = Vector<T>;

    fn deref(&self) -> &Self::Target {
        &self.values
    }
}

impl<T: Clone> Drop for ObservableVectorTransaction<'_, T> {
    fn drop(&mut self) {
        #[cfg(feature = "tracing")]
        if !self.batch.is_empty() {
            tracing::debug!("rollback (drop)");
        }
    }
}

/// A handle to a single value in an [`ObservableVector`], obtained from a
/// transaction.
pub struct ObservableVectorTransactionEntry<'a, 'o, T: Clone> {
    inner: &'a mut ObservableVectorTransaction<'o, T>,
    index: EntryIndex<'a>,
}

impl<'a, 'o, T> ObservableVectorTransactionEntry<'a, 'o, T>
where
    T: Clone + Send + Sync + 'static,
{
    pub(super) fn new(inner: &'a mut ObservableVectorTransaction<'o, T>, index: usize) -> Self {
        Self { inner, index: EntryIndex::Owned(index) }
    }

    fn new_borrowed(
        inner: &'a mut ObservableVectorTransaction<'o, T>,
        index: &'a mut usize,
    ) -> Self {
        Self { inner, index: EntryIndex::Borrowed(index) }
    }

    /// Get the index of the element this `ObservableVectorEntry` refers to.
    pub fn index(this: &Self) -> usize {
        this.index.value()
    }

    /// Replace the given element, notify subscribers and return the previous
    /// element.
    pub fn set(this: &mut Self, value: T) -> T {
        this.inner.set(this.index.value(), value)
    }

    /// Remove the given element, notify subscribers and return the element.
    pub fn remove(mut this: Self) -> T {
        this.inner.remove(this.index.make_owned())
    }
}

impl<T> fmt::Debug for ObservableVectorTransactionEntry<'_, '_, T>
where
    T: Clone + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let index = self.index.value();
        f.debug_struct("ObservableVectorEntry")
            .field("item", &self.inner[index])
            .field("index", &index)
            .finish()
    }
}

impl<T: Clone> ops::Deref for ObservableVectorTransactionEntry<'_, '_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner[self.index.value()]
    }
}

impl<T: Clone> Drop for ObservableVectorTransactionEntry<'_, '_, T> {
    fn drop(&mut self) {
        // If there is an association with an externally-stored index, that
        // index must be incremented on drop. This allows an external iterator
        // that produces ObservableVectorEntry items to advance conditionally.
        //
        // There are two cases this branch is not hit:
        //
        // - make_owned was previously called (used for removing the item and resuming
        //   iteration with the same index)
        // - the ObservableVectorEntry was created with ObservableVector::entry, i.e.
        //   it's not used for iteration at all
        if let EntryIndex::Borrowed(idx) = &mut self.index {
            **idx += 1;
        }
    }
}

/// An "iterator"¹ that yields entries into an [`ObservableVector`], obtained
/// from a transaction.
///
/// ¹ conceptually, though it does not implement `std::iterator::Iterator`
#[derive(Debug)]
pub struct ObservableVectorTransactionEntries<'a, 'o, T: Clone> {
    inner: &'a mut ObservableVectorTransaction<'o, T>,
    index: usize,
}

impl<'a, 'o, T> ObservableVectorTransactionEntries<'a, 'o, T>
where
    T: Clone + Send + Sync + 'static,
{
    pub(super) fn new(inner: &'a mut ObservableVectorTransaction<'o, T>) -> Self {
        Self { inner, index: 0 }
    }

    /// Advance this iterator, yielding an `ObservableVectorEntry` for the next
    /// item in the vector, or `None` if all items have been visited.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<ObservableVectorTransactionEntry<'_, 'o, T>> {
        if self.index < self.inner.len() {
            Some(ObservableVectorTransactionEntry::new_borrowed(self.inner, &mut self.index))
        } else {
            None
        }
    }
}
