use std::{fmt, ops::Deref};

use super::ObservableVector;

/// A handle to a single value in an [`ObservableVector`].
pub struct ObservableVectorEntry<'a, T> {
    inner: &'a mut ObservableVector<T>,
    index: usize,
    removed: Option<&'a mut bool>,
}

impl<'a, T> ObservableVectorEntry<'a, T>
where
    T: Clone + Send + Sync + 'static,
{
    pub(super) fn new(
        inner: &'a mut ObservableVector<T>,
        index: usize,
        removed: Option<&'a mut bool>,
    ) -> Self {
        Self { inner, index, removed }
    }

    /// Get the index of the element this `ObservableVectorEntry` refers to.
    pub fn index(this: &Self) -> usize {
        this.index
    }

    /// Replace the given element, notify subscribers and return the previous
    /// element.
    pub fn set(this: &mut Self, value: T) -> T {
        this.inner.set(this.index, value)
    }

    /// Remove the given element, notify subscribers and return the element.
    pub fn remove(this: Self) -> T {
        if let Some(removed) = this.removed {
            *removed = true;
        }
        this.inner.remove(this.index)
    }
}

impl<T> fmt::Debug for ObservableVectorEntry<'_, T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(&mut true) = &self.removed {
            panic!("removed must only be set when consuming self");
        }

        f.debug_struct("ObservableVectorEntry")
            .field("item", &self.inner[self.index])
            .field("index", &self.index)
            .finish()
    }
}

impl<T> Deref for ObservableVectorEntry<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner[self.index]
    }
}
