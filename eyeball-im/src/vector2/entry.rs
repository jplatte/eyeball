use std::{fmt, ops::Deref};

use super::ObservableVector2WriteGuard;

/// A handle to a single value in an
/// [`ObservableVector2`][super::ObservableVector2].
pub struct ObservableVector2Entry<'a, 'o, T: Clone> {
    inner: &'a mut ObservableVector2WriteGuard<'o, T>,
    index: EntryIndex<'a>,
}

impl<'a, 'o, T> ObservableVector2Entry<'a, 'o, T>
where
    T: Clone + Send + Sync + 'static,
{
    pub(super) fn new(inner: &'a mut ObservableVector2WriteGuard<'o, T>, index: usize) -> Self {
        Self { inner, index: EntryIndex::Owned(index) }
    }

    fn new_borrowed(
        inner: &'a mut ObservableVector2WriteGuard<'o, T>,
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

impl<T> fmt::Debug for ObservableVector2Entry<'_, '_, T>
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

impl<T: Clone> Deref for ObservableVector2Entry<'_, '_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner[self.index.value()]
    }
}

impl<T: Clone> Drop for ObservableVector2Entry<'_, '_, T> {
    fn drop(&mut self) {
        // If there is an association with an externally-stored index, that
        // index must be incremented on drop. This allows an external iterator
        // that produces ObservableVectorEntry items to advance conditionally.
        //
        // There are two cases this branch is not hit:
        //
        // - make_owned was previously called (used for removing the item and resuming
        //   iteration with the same index)
        // - the ObservableVectorEntry was created with ObservableVector2::entry, i.e.
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

/// An "iterator"¹ that yields entries into an [`ObservableVector2`].
///
/// ¹ conceptually, though it does not implement `std::iterator::Iterator`
#[derive(Debug)]
pub struct ObservableVector2Entries<'a, 'o, T: Clone> {
    inner: &'a mut ObservableVector2WriteGuard<'o, T>,
    index: usize,
}

impl<'a, 'o, T> ObservableVector2Entries<'a, 'o, T>
where
    T: Clone + Send + Sync + 'static,
{
    pub(super) fn new(inner: &'a mut ObservableVector2WriteGuard<'o, T>) -> Self {
        Self { inner, index: 0 }
    }

    /// Advance this iterator, yielding an `ObservableVectorEntry` for the next
    /// item in the vector, or `None` if all items have been visited.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<ObservableVector2Entry<'_, 'o, T>> {
        if self.index < self.inner.len() {
            Some(ObservableVector2Entry::new_borrowed(self.inner, &mut self.index))
        } else {
            None
        }
    }
}
