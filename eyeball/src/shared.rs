//! This module defines a shared [`Observable`] type that is clonable, requires
//! only `&` access to update its inner value but doesn't dereference to the
//! inner value.
//!
//! Use this in situations where multiple locations in the code should be able
//! to update the inner value.

use std::{
    fmt,
    future::{poll_fn, Future},
    hash::Hash,
    ops,
    pin::Pin,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
    task::{Context, Poll},
};

use futures_core::Stream;

use crate::state::ObservableState;

/// A value whose changes will be broadcast to subscribers.
///
/// Unlike [`unique::Observable`](crate::unique::Observable), this `Observable`
/// can be `Clone`d but does't dereference to `T`. Because of the latter, it has
/// regular methods to access or modify the inner value.
#[derive(Debug)]
pub struct Observable<T> {
    state: Arc<RwLock<ObservableState<T>>>,
    /// Ugly hack to track the amount of clones of this observable,
    /// *excluding subscribers*.
    _num_clones: Arc<()>,
}

impl<T> Observable<T> {
    /// Create a new `Observable` with the given initial value.
    pub fn new(value: T) -> Self {
        Self {
            state: Arc::new(RwLock::new(ObservableState::new(value))),
            _num_clones: Arc::new(()),
        }
    }

    /// Obtain a new subscriber.
    pub fn subscribe(&self) -> Subscriber<T> {
        let version = self.state.read().unwrap().version();
        Subscriber::new(Arc::clone(&self.state), version)
    }

    /// Read the inner value.
    ///
    /// While the returned read guard is alive, nobody can update the inner
    /// value. If you want to update the value based on the previous value, do
    /// **not** use this method because it can cause races with other clones of
    /// the same `Observable`. Instead, call of of the `update_` methods, or
    /// if that doesn't fit your use case, call [`write`][Self::write] and
    /// update the value through the write guard it returns.
    pub fn read(&self) -> ObservableReadGuard<'_, T> {
        ObservableReadGuard::new(self.state.read().unwrap())
    }

    /// Get a write guard to the inner value.
    ///
    /// This can be used to set a new value based on the existing value. The
    /// returned write guard dereferences (immutably) to the inner type, and has
    /// associated functions to update it.
    pub fn write(&self) -> ObservableWriteGuard<'_, T> {
        ObservableWriteGuard::new(self.state.write().unwrap())
    }

    /// Set the inner value to the given `value` and notify subscribers.
    pub fn set(&self, value: T) {
        self.state.write().unwrap().set(value);
    }

    /// Set the inner value to the given `value` and notify subscribers if the
    /// updated value does not equal the previous value.
    pub fn set_eq(&self, value: T)
    where
        T: Clone + PartialEq,
    {
        Self::update_eq(self, |inner| {
            *inner = value;
        });
    }

    /// Set the inner value to the given `value` and notify subscribers if the
    /// hash of the updated value does not equal the hash of the previous
    /// value.
    pub fn set_hash(&self, value: T)
    where
        T: Hash,
    {
        Self::update_hash(self, |inner| {
            *inner = value;
        });
    }

    /// Set the inner value to the given `value`, notify subscribers and return
    /// the previous value.
    pub fn replace(&self, value: T) -> T {
        self.state.write().unwrap().replace(value)
    }

    /// Set the inner value to a `Default` instance of its type, notify
    /// subscribers and return the previous value.
    ///
    /// Shorthand for `observable.replace(T::default())`.
    pub fn take(&self) -> T
    where
        T: Default,
    {
        self.replace(T::default())
    }

    /// Update the inner value and notify subscribers.
    ///
    /// Note that even if the inner value is not actually changed by the
    /// closure, subscribers will be notified as if it was. Use one of the
    /// other update methods below if you want to conditionally mutate the
    /// inner value.
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        self.state.write().unwrap().update(f);
    }

    /// Update the inner value and notify subscribers if the updated value does
    /// not equal the previous value.
    pub fn update_eq(&self, f: impl FnOnce(&mut T))
    where
        T: Clone + PartialEq,
    {
        self.state.write().unwrap().update_eq(f);
    }

    /// Update the inner value and notify subscribers if the hash of the updated
    /// value does not equal the hash of the previous value.
    pub fn update_hash(&self, f: impl FnOnce(&mut T))
    where
        T: Hash,
    {
        self.state.write().unwrap().update_hash(f);
    }
}

impl<T> Clone for Observable<T> {
    fn clone(&self) -> Self {
        Self { state: self.state.clone(), _num_clones: self._num_clones.clone() }
    }
}

impl<T: Default> Default for Observable<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> Drop for Observable<T> {
    fn drop(&mut self) {
        // Only close the state if there are no other clones of this
        // `Observable`.
        if Arc::strong_count(&self._num_clones) == 1 {
            self.state.write().unwrap().close();
        }
    }
}

/// A read guard for the inner value of an observable.
///
/// Note that as long as a `ObservableReadGuard` is kept alive, the associated
/// [`Observable`] is locked and can not be updated.
#[clippy::has_significant_drop]
pub struct ObservableReadGuard<'a, T> {
    inner: RwLockReadGuard<'a, ObservableState<T>>,
}

impl<'a, T> ObservableReadGuard<'a, T> {
    fn new(inner: RwLockReadGuard<'a, ObservableState<T>>) -> Self {
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

/// A write guard for the inner value of an observable.
///
/// Note that as long as a `ObservableReadGuard` is kept alive, the associated
/// [`Observable`] is locked and can not be updated.
#[clippy::has_significant_drop]
pub struct ObservableWriteGuard<'a, T> {
    inner: RwLockWriteGuard<'a, ObservableState<T>>,
}

impl<'a, T> ObservableWriteGuard<'a, T> {
    fn new(inner: RwLockWriteGuard<'a, ObservableState<T>>) -> Self {
        Self { inner }
    }

    /// Set the inner value to the given `value` and notify subscribers.
    pub fn set(this: &mut Self, value: T) {
        this.inner.set(value);
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
        this.inner.replace(value)
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
        this.inner.update(f);
    }

    /// Update the inner value and notify subscribers if the updated value does
    /// not equal the previous value.
    pub fn update_eq(this: &mut Self, f: impl FnOnce(&mut T))
    where
        T: Clone + PartialEq,
    {
        this.inner.update_eq(f);
    }

    /// Update the inner value and notify subscribers if the hash of the updated
    /// value does not equal the hash of the previous value.
    pub fn update_hash(this: &mut Self, f: impl FnOnce(&mut T))
    where
        T: Hash,
    {
        this.inner.update_hash(f);
    }
}

impl<T: fmt::Debug> fmt::Debug for ObservableWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T> ops::Deref for ObservableWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.get()
    }
}

/// A subscriber for updates of an [`Observable`].
#[derive(Debug)]
pub struct Subscriber<T> {
    state: Arc<RwLock<ObservableState<T>>>,
    observed_version: u64,
}

impl<T> Subscriber<T> {
    pub(crate) fn new(read_lock: Arc<RwLock<ObservableState<T>>>, version: u64) -> Self {
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
        let lock = self.state.read().unwrap();
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
        let lock = self.state.read().unwrap();
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
        ObservableReadGuard::new(self.state.read().unwrap())
    }

    fn poll_next_ref(&mut self, cx: &mut Context<'_>) -> Poll<Option<ObservableReadGuard<'_, T>>> {
        let state = self.state.read().unwrap();
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
