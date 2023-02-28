//! Details of observable [`Subscriber`]s.
//!
//! Usually, you don't need to interact with this module at all, since its most
//! important types are re-exported at the crate root.

use std::{
    fmt,
    future::{poll_fn, Future},
    ops,
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::Stream;
use readlock::{SharedReadGuard, SharedReadLock};

#[cfg(doc)]
use crate::observable::Observable;
use crate::state::ObservableState;

/// A subscriber for updates of an [`Observable`].
#[derive(Debug)]
pub struct Subscriber<T> {
    state: SharedReadLock<ObservableState<T>>,
    observed_version: u64,
}

impl<T> Subscriber<T> {
    pub(crate) fn new(read_lock: SharedReadLock<ObservableState<T>>, version: u64) -> Self {
        Self { state: read_lock, observed_version: version }
    }

    /// Wait for an update and get a clone of the updated value.
    ///
    /// This method is a convenience so you don't have to import a `Stream`
    /// extension trait such as `futures::StreamExt` or
    /// `tokio_stream::StreamExt`.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Next<T>
    where
        T: Clone,
    {
        Next::new(self)
    }

    /// Get a clone of the inner value without waiting for an update.
    ///
    /// If the returned value has not been observed by this subscriber before,
    /// it is marked as observed such that a subsequent call of
    /// [`next`][Self::next] or [`next_ref`][Self::next_ref] won't return the
    /// same value again. See [`get`][Self::get] for a function that doesn't
    /// mark the value as observed.
    pub fn next_now(&mut self) -> T
    where
        T: Clone,
    {
        let lock = self.state.lock();
        self.observed_version = lock.version();
        lock.get().clone()
    }

    /// Get a clone of the inner value without waiting for an update.
    ///
    /// If the returned value has not been observed by this subscriber before,
    /// it is **not** marked as observed such that a subsequent call of
    /// [`next`][Self::next] or [`next_ref`][Self::next_ref] will return the
    /// same value again.
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.read().clone()
    }

    /// Wait for an update and get a read lock for the updated value.
    ///
    /// You can use this method to get updates of an [`Observable`] where the
    /// inner type does not implement `Clone`. However, the `Observable`
    /// will be locked (not updateable) while any read locks are alive.
    pub async fn next_ref(&mut self) -> Option<SubscriberReadGuard<'_, T>> {
        // Unclear how to implement this as a named future.
        poll_fn(|cx| self.poll_next_ref(cx).map(|opt| opt.map(|_| {}))).await?;
        Some(self.next_ref_now())
    }

    /// Lock the inner value for reading without waiting for an update.
    ///
    /// Note that as long as the returned [`SubscriberReadGuard`] is kept alive,
    /// the associated [`Observable`] is locked and can not be updated.
    ///
    /// If the returned value has not been observed by this subscriber before,
    /// it is marked as observed such that a subsequent call of
    /// [`next`][Self::next] or [`next_ref`][Self::next_ref] won't return the
    /// same value again. See [`get`][Self::get] for a function that doesn't
    /// mark the value as observed.
    pub fn next_ref_now(&mut self) -> SubscriberReadGuard<'_, T> {
        let lock = self.state.lock();
        self.observed_version = lock.version();
        SubscriberReadGuard::new(lock)
    }

    /// Lock the inner value for reading without waiting for an update.
    ///
    /// Note that as long as the returned [`SubscriberReadGuard`] is kept alive,
    /// the associated [`Observable`] is locked and can not be updated.
    ///
    /// If the returned value has not been observed by this subscriber before,
    /// it is **not** marked as observed such that a subsequent call of
    /// [`next`][Self::next] or [`next_ref`][Self::next_ref] will return the
    /// same value again.
    pub fn read(&self) -> SubscriberReadGuard<'_, T> {
        SubscriberReadGuard::new(self.state.lock())
    }

    fn poll_next_ref(&mut self, cx: &mut Context<'_>) -> Poll<Option<SubscriberReadGuard<'_, T>>> {
        let state = self.state.lock();
        let version = state.version();
        if version == 0 {
            Poll::Ready(None)
        } else if self.observed_version < version {
            self.observed_version = version;
            Poll::Ready(Some(SubscriberReadGuard::new(state)))
        } else {
            state.add_waker(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl<T: Clone> Stream for Subscriber<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.poll_next_ref(cx).map(opt_guard_to_owned)
    }
}

/// A read guard that allows you to read the inner value of an observable
/// without cloning.
///
/// Note that as long as a SubscriberReadGuard is kept alive, the associated
/// [`Observable`] is locked and can not be updated.
#[clippy::has_significant_drop]
pub struct SubscriberReadGuard<'a, T> {
    inner: SharedReadGuard<'a, ObservableState<T>>,
}

impl<'a, T> SubscriberReadGuard<'a, T> {
    fn new(inner: SharedReadGuard<'a, ObservableState<T>>) -> Self {
        Self { inner }
    }
}

impl<T: fmt::Debug> fmt::Debug for SubscriberReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T> ops::Deref for SubscriberReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.get()
    }
}

/// Future returned by [`Subscriber::next`].
#[derive(Debug)]
pub struct Next<'a, T> {
    subscriber: &'a mut Subscriber<T>,
}

impl<'a, T> Next<'a, T> {
    fn new(subscriber: &'a mut Subscriber<T>) -> Self {
        Self { subscriber }
    }
}

impl<'a, T: Clone> Future for Next<'a, T> {
    type Output = Option<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.subscriber.poll_next_ref(cx).map(opt_guard_to_owned)
    }
}

fn opt_guard_to_owned<T: Clone>(value: Option<SubscriberReadGuard<'_, T>>) -> Option<T> {
    value.map(|guard| guard.to_owned())
}
