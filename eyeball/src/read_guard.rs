use std::{fmt, ops};

use crate::{lock::Lock, state::ObservableState, SyncLock};

/// A read guard for the inner value of an observable.
///
/// Note that as long as an `ObservableReadGuard` is kept alive, the associated
/// `Observable` is locked and can not be updated.
#[must_use]
#[clippy::has_significant_drop]
pub struct ObservableReadGuard<'a, T: 'a, L: Lock = SyncLock> {
    inner: L::SharedReadGuard<'a, ObservableState<T>>,
}

impl<'a, T: 'a, L: Lock> ObservableReadGuard<'a, T, L> {
    pub(crate) fn new(inner: L::SharedReadGuard<'a, ObservableState<T>>) -> Self {
        Self { inner }
    }
}

impl<T: fmt::Debug, L: Lock> fmt::Debug for ObservableReadGuard<'_, T, L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T, L: Lock> ops::Deref for ObservableReadGuard<'_, T, L> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.get()
    }
}
