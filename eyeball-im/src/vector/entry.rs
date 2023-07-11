use std::{fmt, ops::Deref};

use super::ObservableVector;

/// A handle to a single value in an [`ObservableVector`].
pub struct ObservableVectorEntry<'a, T> {
    inner: &'a mut ObservableVector<T>,
    index: EntryIndex<'a>,
}

impl<'a, T> ObservableVectorEntry<'a, T>
where
    T: Clone + Send + Sync + 'static,
{
    pub(super) fn new(inner: &'a mut ObservableVector<T>, index: usize) -> Self {
        Self { inner, index: EntryIndex::Owned(index) }
    }

    pub(super) fn new_borrowed(inner: &'a mut ObservableVector<T>, index: &'a mut usize) -> Self {
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

impl<T> fmt::Debug for ObservableVectorEntry<'_, T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let index = self.index.value();
        f.debug_struct("ObservableVectorEntry")
            .field("item", &self.inner[index])
            .field("index", &index)
            .finish()
    }
}

impl<T> Deref for ObservableVectorEntry<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner[self.index.value()]
    }
}

impl<T> Drop for ObservableVectorEntry<'_, T> {
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

enum EntryIndex<'a> {
    Borrowed(&'a mut usize),
    Owned(usize),
}

impl<'a> EntryIndex<'a> {
    fn value(&self) -> usize {
        match self {
            EntryIndex::Borrowed(idx) => **idx,
            EntryIndex::Owned(idx) => *idx,
        }
    }

    /// Remove the association with the externally-stored index, if any.
    ///
    /// Returns the index value for convenience.
    fn make_owned(&mut self) -> usize {
        match self {
            EntryIndex::Borrowed(idx) => {
                let idx = **idx;
                *self = EntryIndex::Owned(idx);
                idx
            }
            EntryIndex::Owned(idx) => *idx,
        }
    }
}
