//! Details of observable [`Subscriber`]s.
//!
//! Usually, you don't need to interact with this module at all, since its most
//! important type `Subscriber` is re-exported at the crate root.

use std::{
    future::{poll_fn, Future},
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::Stream;
use readlock::SharedReadLock;

use crate::{state::ObservableState, ObservableReadGuard};

/// A subscriber for updates of an `Observable`.
#[must_use]
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
    /// Awaiting returns `Some(_)` after an update happened, or `None` after the
    /// `Observable` (and all clones for `shared::Observable`) is dropped.
    ///
    /// This method is a convenience so you don't have to import a `Stream`
    /// extension trait such as `futures::StreamExt` or
    /// `tokio_stream::StreamExt`.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Next<'_, T>
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
    #[must_use]
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
    #[must_use]
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.read().clone()
    }

    /// Wait for an update and get a read lock for the updated value.
    ///
    /// Awaiting returns `Some(_)` after an update happened, or `None` after the
    /// `Observable` (and all clones for `shared::Observable`) is dropped.
    ///
    /// You can use this method to get updates of an `Observable` where the
    /// inner type does not implement `Clone`. However, the `Observable`
    /// will be locked (not updateable) while any read locks are alive.
    #[must_use]
    pub async fn next_ref(&mut self) -> Option<ObservableReadGuard<'_, T>> {
        // Unclear how to implement this as a named future.
        poll_fn(|cx| self.poll_next_ref(cx).map(|opt| opt.map(|_| {}))).await?;
        Some(self.next_ref_now())
    }

    /// Lock the inner value for reading without waiting for an update.
    ///
    /// Note that as long as the returned [`ObservableReadGuard`] is kept alive,
    /// the associated `Observable` is locked and can not be updated.
    ///
    /// If the returned value has not been observed by this subscriber before,
    /// it is marked as observed such that a subsequent call of
    /// [`next`][Self::next] or [`next_ref`][Self::next_ref] won't return the
    /// same value again. See [`get`][Self::get] for a function that doesn't
    /// mark the value as observed.
    pub fn next_ref_now(&mut self) -> ObservableReadGuard<'_, T> {
        let lock = self.state.lock();
        self.observed_version = lock.version();
        ObservableReadGuard::new(lock)
    }

    /// Lock the inner value for reading without waiting for an update.
    ///
    /// Note that as long as the returned [`ObservableReadGuard`] is kept alive,
    /// the associated `Observable` is locked and can not be updated.
    ///
    /// If the returned value has not been observed by this subscriber before,
    /// it is **not** marked as observed such that a subsequent call of
    /// [`next`][Self::next] or [`next_ref`][Self::next_ref] will return the
    /// same value again.
    pub fn read(&self) -> ObservableReadGuard<'_, T> {
        ObservableReadGuard::new(self.state.lock())
    }

    /// Reset the observed version of the inner value.
    ///
    /// After calling this, it is guaranteed that the next call to
    /// `.next().await` or `.next_ref().await` will resolve immediately.
    ///
    /// This is only useful if you do this before passing the subscriber to some
    /// other generic function or returning it, if you would be calling
    /// `.next().await` right afterwards, you can call
    /// [`.next_now()`][Self::next_now] instead (same for `.reset()` plus
    /// `.next_ref().await`, which can be expressed by
    /// [`.next_ref_now()`](Self::next_ref_now)).
    pub fn reset(&mut self) {
        self.observed_version = 0;
    }

    /// Clone this `Subscriber` and reset the observed version of the inner
    /// value.
    ///
    /// This is equivalent to using the regular [`clone`][Self::clone] method
    /// and calling [`reset`][Self::reset] on the clone afterwards.
    pub fn clone_reset(&self) -> Self {
        Self { state: self.state.clone(), observed_version: 0 }
    }

    fn poll_next_ref(&mut self, cx: &mut Context<'_>) -> Poll<Option<ObservableReadGuard<'_, T>>> {
        let state = self.state.lock();
        let version = state.version();
        if version == 0 {
            Poll::Ready(None)
        } else if self.observed_version < version {
            self.observed_version = version;
            Poll::Ready(Some(ObservableReadGuard::new(state)))
        } else {
            state.add_waker(cx.waker().clone());
            Poll::Pending
        }
    }
}

/// Clone this `Subscriber` exactly, including the observed version of the inner
/// value.
///
/// That means that if the original `Subscriber` was up-to-date with the latest
/// value of the observable, the new one will be as well, and vice-versa.
///
/// See [`clone_reset`][Self::clone_reset] for a convenient way of making a new
/// `Subscriber` from an existing one without inheriting the observed version of
/// the inner value.
impl<T> Clone for Subscriber<T> {
    fn clone(&self) -> Self {
        Self { state: self.state.clone(), observed_version: self.observed_version }
    }
}

impl<T: Clone> Stream for Subscriber<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.poll_next_ref(cx).map(opt_guard_to_owned)
    }
}

/// Future returned by [`Subscriber::next`].
#[must_use]
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

fn opt_guard_to_owned<T: Clone>(value: Option<ObservableReadGuard<'_, T>>) -> Option<T> {
    value.map(|guard| guard.to_owned())
}
