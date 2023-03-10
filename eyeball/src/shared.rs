//! This module defines a shared [`Observable`] type that is clonable, requires
//! only `&` access to update its inner value but doesn't dereference to the
//! inner value.
//!
//! Use this in situations where multiple locations in the code should be able
//! to update the inner value.

use std::{
    fmt,
    hash::Hash,
    ops,
    sync::{Arc, RwLock, RwLockWriteGuard},
};

use readlock::{SharedReadGuard, SharedReadLock};

use crate::{state::ObservableState, ObservableReadGuard, Subscriber};

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
        Subscriber::new(SharedReadLock::from_inner(Arc::clone(&self.state)), version)
    }

    /// Get a clone of the inner value.
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.state.read().unwrap().get().clone()
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
        ObservableReadGuard::new(SharedReadGuard::from_inner(self.state.read().unwrap()))
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

/// A write guard for the inner value of an observable.
///
/// Note that as long as an `ObservableWriteGuard` is kept alive, the associated
/// [`Observable`] is locked and can not be updated except through that guard.
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
