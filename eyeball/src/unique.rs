//! This module defines a unique [`Observable`] type that requires `&mut` access
//! to update its inner value but can be dereferenced (immutably).
//!
//! Use this in situations where only a single location in the code should be
//! able to update the inner value.

use std::{
    fmt,
    future::{poll_fn, Future},
    hash::Hash,
    ops,
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::Stream;
use readlock::{Shared, SharedReadGuard, SharedReadLock};

use crate::state::ObservableState;

/// A value whose changes will be broadcast to subscribers.
///
/// `Observable<T>` dereferences to `T`, and does not have methods of its own to
/// not clash with methods of the inner type. Instead, to interact with the
/// `Observable` itself rather than the inner value, use its associated
/// functions (e.g. `Observable::subscribe(observable)`).
#[derive(Debug)]
pub struct Observable<T> {
    state: Shared<ObservableState<T>>,
}

impl<T> Observable<T> {
    /// Create a new `Observable` with the given initial value.
    pub fn new(value: T) -> Self {
        Self { state: Shared::new(ObservableState::new(value)) }
    }

    /// Obtain a new subscriber.
    pub fn subscribe(this: &Self) -> Subscriber<T> {
        Subscriber::new(Shared::get_read_lock(&this.state), this.state.version())
    }

    /// Get a reference to the inner value.
    ///
    /// Usually, you don't need to call this function since `Observable<T>`
    /// implements `Deref`. Use this if you want to pass the inner value to a
    /// generic function where the compiler can't infer that you want to have
    /// the `Observable` dereferenced otherwise.
    pub fn get(this: &Self) -> &T {
        this.state.get()
    }

    /// Set the inner value to the given `value` and notify subscribers.
    pub fn set(this: &mut Self, value: T) {
        Shared::lock(&mut this.state).set(value);
    }

    /// Set the inner value to the given `value` and notify subscribers if the
    /// updated value does not equal the previous value.
    pub fn set_eq(this: &mut Self, value: T)
    where
        T: Clone + PartialEq,
    {
        Self::update_eq(this, |inner| {
            *inner = value;
        });
    }

    /// Set the inner value to the given `value` and notify subscribers if the
    /// hash of the updated value does not equal the hash of the previous
    /// value.
    pub fn set_hash(this: &mut Self, value: T)
    where
        T: Hash,
    {
        Self::update_hash(this, |inner| {
            *inner = value;
        });
    }

    /// Set the inner value to the given `value`, notify subscribers and return
    /// the previous value.
    pub fn replace(this: &mut Self, value: T) -> T {
        Shared::lock(&mut this.state).replace(value)
    }

    /// Set the inner value to a `Default` instance of its type, notify
    /// subscribers and return the previous value.
    ///
    /// Shorthand for `Observable::replace(this, T::default())`.
    pub fn take(this: &mut Self) -> T
    where
        T: Default,
    {
        Self::replace(this, T::default())
    }

    /// Update the inner value and notify subscribers.
    ///
    /// Note that even if the inner value is not actually changed by the
    /// closure, subscribers will be notified as if it was. Use one of the
    /// other update methods below if you want to conditionally mutate the
    /// inner value.
    pub fn update(this: &mut Self, f: impl FnOnce(&mut T)) {
        Shared::lock(&mut this.state).update(f);
    }

    /// Update the inner value and notify subscribers if the updated value does
    /// not equal the previous value.
    pub fn update_eq(this: &mut Self, f: impl FnOnce(&mut T))
    where
        T: Clone + PartialEq,
    {
        Shared::lock(&mut this.state).update_eq(f);
    }

    /// Update the inner value and notify subscribers if the hash of the updated
    /// value does not equal the hash of the previous value.
    pub fn update_hash(this: &mut Self, f: impl FnOnce(&mut T))
    where
        T: Hash,
    {
        Shared::lock(&mut this.state).update_hash(f);
    }
}

impl<T: Default> Default for Observable<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

// Note: No DerefMut because all mutating must go through inherent methods that
// notify subscribers
impl<T> ops::Deref for Observable<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.state.get()
    }
}

impl<T> Drop for Observable<T> {
    fn drop(&mut self) {
        Shared::lock(&mut self.state).close();
    }
}

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
    pub async fn next_ref(&mut self) -> Option<ObservableReadGuard<'_, T>> {
        // Unclear how to implement this as a named future.
        poll_fn(|cx| self.poll_next_ref(cx).map(|opt| opt.map(|_| {}))).await?;
        Some(self.next_ref_now())
    }

    /// Lock the inner value for reading without waiting for an update.
    ///
    /// Note that as long as the returned [`ObservableReadGuard`] is kept alive,
    /// the associated [`Observable`] is locked and can not be updated.
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
    /// the associated [`Observable`] is locked and can not be updated.
    ///
    /// If the returned value has not been observed by this subscriber before,
    /// it is **not** marked as observed such that a subsequent call of
    /// [`next`][Self::next] or [`next_ref`][Self::next_ref] will return the
    /// same value again.
    pub fn read(&self) -> ObservableReadGuard<'_, T> {
        ObservableReadGuard::new(self.state.lock())
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

impl<T: Clone> Stream for Subscriber<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.poll_next_ref(cx).map(opt_guard_to_owned)
    }
}

/// A read guard that allows you to read the inner value of an observable
/// without cloning.
///
/// Note that as long as a `ObservableReadGuard` is kept alive, the associated
/// [`Observable`] is locked and can not be updated.
#[clippy::has_significant_drop]
pub struct ObservableReadGuard<'a, T> {
    inner: SharedReadGuard<'a, ObservableState<T>>,
}

impl<'a, T> ObservableReadGuard<'a, T> {
    fn new(inner: SharedReadGuard<'a, ObservableState<T>>) -> Self {
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

fn opt_guard_to_owned<T: Clone>(value: Option<ObservableReadGuard<'_, T>>) -> Option<T> {
    value.map(|guard| guard.to_owned())
}
