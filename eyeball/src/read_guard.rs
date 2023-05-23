use std::{fmt, mem::MaybeUninit, ops};

use readlock::SharedReadGuard;

use crate::state::ObservableState;

/// A read guard for the inner value of an observable.
///
/// Note that as long as an `ObservableReadGuard` is kept alive, the associated
/// `Observable` is locked and can not be updated.
#[must_use]
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

/// A read guard for the inner value of a lazy observable.
///
/// Note that as long as an `ObservableReadGuard` is kept alive, the associated
/// `LazyObservable` is locked and can not be updated.
#[must_use]
#[clippy::has_significant_drop]
pub struct LazyObservableReadGuard<'a, T> {
    inner: SharedReadGuard<'a, ObservableState<MaybeUninit<T>>>,
}

impl<'a, T> LazyObservableReadGuard<'a, T> {
    pub(crate) fn new(inner: SharedReadGuard<'a, ObservableState<MaybeUninit<T>>>) -> Option<Self> {
        inner.is_initialized().then_some(Self { inner })
    }
}

impl<T: fmt::Debug> fmt::Debug for LazyObservableReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T> ops::Deref for LazyObservableReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: The constructor made sure the value is initialized
        unsafe { self.inner.get().assume_init_ref() }
    }
}
