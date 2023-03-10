use std::{fmt, ops};

use readlock::SharedReadGuard;

use crate::state::ObservableState;

/// A read guard for the inner value of an observable.
///
/// Note that as long as an `ObservableReadGuard` is kept alive, the associated
/// `Observable` is locked and can not be updated.
#[clippy::has_significant_drop]
pub struct ObservableReadGuard<'a, T> {
    inner: SharedReadGuard<'a, ObservableState<T>>,
}

impl<'a, T> ObservableReadGuard<'a, T> {
    pub(crate) fn new(inner: SharedReadGuard<'a, ObservableState<T>>) -> Self {
        Self { inner }
    }
}

impl<T: fmt::Debug> fmt::Debug for ObservableReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T> ops::Deref for ObservableReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.get()
    }
}
