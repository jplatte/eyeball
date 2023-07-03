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
    sync::{Arc, Weak},
};

use readlock::{SharedReadGuard, SharedReadLock};
#[cfg(feature = "async-lock")]
use readlock_tokio::{
    SharedReadGuard as SharedAsyncReadGuard, SharedReadLock as SharedAsyncReadLock,
};

#[cfg(feature = "async-lock")]
use crate::AsyncLock;
use crate::{lock::Lock, state::ObservableState, ObservableReadGuard, Subscriber, SyncLock};

/// A value whose changes will be broadcast to subscribers.
///
/// Unlike [`unique::Observable`](crate::unique::Observable), this `Observable`
/// can be `Clone`d but does't dereference to `T`. Because of the latter, it has
/// regular methods to access or modify the inner value.
///
/// # Async-aware locking
///
/// If you want to write-lock the inner value over a `.await` point, that
/// requires an async-aware lock. You can use [`new_async`][Self::new_async] to
/// create an `Observable<T, AsyncLock>`, where most methods are `async` but in
/// return locking the inner value over `.await` points becomes unproblematic.
pub struct Observable<T, L: Lock = SyncLock> {
    state: Arc<L::RwLock<ObservableState<T>>>,
    /// Ugly hack to track the amount of clones of this observable,
    /// *excluding subscribers*.
    _num_clones: Arc<()>,
}

impl<T> Observable<T> {
    /// Create a new `Observable` with the given initial value.
    #[must_use]
    pub fn new(value: T) -> Self {
        Self::from_inner(Arc::new(std::sync::RwLock::new(ObservableState::new(value))))
    }

    /// Obtain a new subscriber.
    ///
    /// Calling `.next().await` or `.next_ref().await` on the returned
    /// subscriber only resolves once the inner value has been updated again
    /// after the call to `subscribe`.
    ///
    /// See [`subscribe_reset`][Self::subscribe_reset] if you want to obtain a
    /// subscriber that immediately yields without any updates.
    pub fn subscribe(&self) -> Subscriber<T> {
        let version = self.state.read().unwrap().version();
        Subscriber::new(SharedReadLock::from_inner(Arc::clone(&self.state)), version)
    }

    /// Obtain a new subscriber that immediately yields.
    ///
    /// `.subscribe_reset()` is equivalent to `.subscribe()` with a subsequent
    /// call to [`.reset()`][Subscriber::reset] on the returned subscriber.
    ///
    /// In contrast to [`subscribe`][Self::subscribe], calling `.next().await`
    /// or `.next_ref().await` on the returned subscriber before updating the
    /// inner value yields the current value instead of waiting. Further calls
    /// to either of the two will wait for updates.
    pub fn subscribe_reset(&self) -> Subscriber<T> {
        Subscriber::new(SharedReadLock::from_inner(Arc::clone(&self.state)), 0)
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

    /// Set the inner value to the given `value`, notify subscribers and return
    /// the previous value.
    pub fn set(&self, value: T) -> T {
        self.state.write().unwrap().set(value)
    }

    /// Set the inner value to the given `value` if it doesn't compare equal to
    /// the existing value.
    ///
    /// If the inner value is set, subscribers are notified and
    /// `Some(previous_value)` is returned. Otherwise, `None` is returned.
    pub fn set_if_not_eq(&self, value: T) -> Option<T>
    where
        T: PartialEq,
    {
        self.state.write().unwrap().set_if_not_eq(value)
    }

    /// Set the inner value to the given `value` if it has a different hash than
    /// the existing value.
    ///
    /// If the inner value is set, subscribers are notified and
    /// `Some(previous_value)` is returned. Otherwise, `None` is returned.
    pub fn set_if_hash_not_eq(&self, value: T) -> Option<T>
    where
        T: Hash,
    {
        self.state.write().unwrap().set_if_hash_not_eq(value)
    }

    /// Set the inner value to a `Default` instance of its type, notify
    /// subscribers and return the previous value.
    ///
    /// Shorthand for `observable.set(T::default())`.
    pub fn take(&self) -> T
    where
        T: Default,
    {
        self.set(T::default())
    }

    /// Update the inner value and notify subscribers.
    ///
    /// Note that even if the inner value is not actually changed by the
    /// closure, subscribers will be notified as if it was. Use
    /// [`update_if`][Self::update_if] if you want to conditionally mutate the
    /// inner value.
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        self.state.write().unwrap().update(f);
    }

    /// Maybe update the inner value and notify subscribers if it changed.
    ///
    /// The closure given to this function must return `true` if subscribers
    /// should be notified of a change to the inner value.
    pub fn update_if(&self, f: impl FnOnce(&mut T) -> bool) {
        self.state.write().unwrap().update_if(f);
    }
}

#[cfg(feature = "async-lock")]
impl<T: Send + Sync + 'static> Observable<T, AsyncLock> {
    /// Create a new async `Observable` with the given initial value.
    #[must_use]
    pub fn new_async(value: T) -> Self {
        Self::from_inner(Arc::new(tokio::sync::RwLock::new(ObservableState::new(value))))
    }

    /// Obtain a new subscriber.
    ///
    /// Calling `.next().await` or `.next_ref().await` on the returned
    /// subscriber only resolves once the inner value has been updated again
    /// after the call to `subscribe`.
    ///
    /// See [`subscribe_reset`][Self::subscribe_reset] if you want to obtain a
    /// subscriber that immediately yields without any updates.
    pub async fn subscribe(&self) -> Subscriber<T, AsyncLock> {
        let version = self.state.read().await.version();
        Subscriber::new_async(SharedAsyncReadLock::from_inner(Arc::clone(&self.state)), version)
    }

    /// Obtain a new subscriber that immediately yields.
    ///
    /// `.subscribe_reset()` is equivalent to `.subscribe()` with a subsequent
    /// call to [`.reset()`][Subscriber::reset] on the returned subscriber.
    ///
    /// In contrast to [`subscribe`][Self::subscribe], calling `.next().await`
    /// or `.next_ref().await` on the returned subscriber before updating the
    /// inner value yields the current value instead of waiting. Further calls
    /// to either of the two will wait for updates.
    pub fn subscribe_reset(&self) -> Subscriber<T, AsyncLock> {
        Subscriber::new_async(SharedAsyncReadLock::from_inner(Arc::clone(&self.state)), 0)
    }

    /// Get a clone of the inner value.
    pub async fn get(&self) -> T
    where
        T: Clone,
    {
        self.state.read().await.get().clone()
    }

    /// Read the inner value.
    ///
    /// While the returned read guard is alive, nobody can update the inner
    /// value. If you want to update the value based on the previous value, do
    /// **not** use this method because it can cause races with other clones of
    /// the same `Observable`. Instead, call of of the `update_` methods, or
    /// if that doesn't fit your use case, call [`write`][Self::write] and
    /// update the value through the write guard it returns.
    pub async fn read(&self) -> ObservableReadGuard<'_, T, AsyncLock> {
        ObservableReadGuard::new(SharedAsyncReadGuard::from_inner(self.state.read().await))
    }

    /// Get a write guard to the inner value.
    ///
    /// This can be used to set a new value based on the existing value. The
    /// returned write guard dereferences (immutably) to the inner type, and has
    /// associated functions to update it.
    pub async fn write(&self) -> ObservableWriteGuard<'_, T, AsyncLock> {
        ObservableWriteGuard::new(self.state.write().await)
    }

    /// Set the inner value to the given `value`, notify subscribers and return
    /// the previous value.
    pub async fn set(&self, value: T) -> T {
        self.state.write().await.set(value)
    }

    /// Set the inner value to the given `value` if it doesn't compare equal to
    /// the existing value.
    ///
    /// If the inner value is set, subscribers are notified and
    /// `Some(previous_value)` is returned. Otherwise, `None` is returned.
    pub async fn set_if_not_eq(&self, value: T) -> Option<T>
    where
        T: PartialEq,
    {
        self.state.write().await.set_if_not_eq(value)
    }

    /// Set the inner value to the given `value` if it has a different hash than
    /// the existing value.
    ///
    /// If the inner value is set, subscribers are notified and
    /// `Some(previous_value)` is returned. Otherwise, `None` is returned.
    pub async fn set_if_hash_not_eq(&self, value: T) -> Option<T>
    where
        T: Hash,
    {
        self.state.write().await.set_if_hash_not_eq(value)
    }

    /// Set the inner value to a `Default` instance of its type, notify
    /// subscribers and return the previous value.
    ///
    /// Shorthand for `observable.set(T::default())`.
    pub async fn take(&self) -> T
    where
        T: Default,
    {
        self.set(T::default()).await
    }

    /// Update the inner value and notify subscribers.
    ///
    /// Note that even if the inner value is not actually changed by the
    /// closure, subscribers will be notified as if it was. Use
    /// [`update_if`][Self::update_if] if you want to conditionally mutate the
    /// inner value.
    pub async fn update(&self, f: impl FnOnce(&mut T)) {
        self.state.write().await.update(f);
    }

    /// Maybe update the inner value and notify subscribers if it changed.
    ///
    /// The closure given to this function must return `true` if subscribers
    /// should be notified of a change to the inner value.
    pub async fn update_if(&self, f: impl FnOnce(&mut T) -> bool) {
        self.state.write().await.update_if(f);
    }
}

impl<T, L: Lock> Observable<T, L> {
    pub(crate) fn from_inner(state: Arc<L::RwLock<ObservableState<T>>>) -> Self {
        Self { state, _num_clones: Arc::new(()) }
    }

    /// Get the number of `Observable` clones.
    ///
    /// This always returns at least `1` since `self` is included in the count.
    ///
    /// Be careful when using this. The result is only reliable if it is exactly
    /// `1`, as otherwise it could be incremented right after your call to this
    /// function, before you look at its result or do anything based on that.
    #[must_use]
    pub fn observable_count(&self) -> usize {
        Arc::strong_count(&self._num_clones)
    }

    /// Get the number of subscribers.
    ///
    /// Be careful when using this. The result can change right after your call
    /// to this function, before you look at its result or do anything based
    /// on that.
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.strong_count() - self.observable_count()
    }

    /// Get the number of strong references to the inner value.
    ///
    /// Every clone of the `Observable` and every associated `Subscriber` holds
    /// a reference, so this is the sum of all clones and subscribers.
    /// This always returns at least `1` since `self` is included in the count.
    ///
    /// Equivalent to `ob.observable_count() + ob.subscriber_count()`.
    ///
    /// Be careful when using this. The result is only reliable if it is exactly
    /// `1`, as otherwise it could be incremented right after your call to this
    /// function, before you look at its result or do anything based on that.
    #[must_use]
    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.state)
    }

    /// Get the number of weak references to the inner value.
    ///
    /// Weak references are created using [`downgrade`][Self::downgrade] or by
    /// cloning an existing weak reference.
    #[must_use]
    pub fn weak_count(&self) -> usize {
        Arc::weak_count(&self.state)
    }

    /// Create a new [`WeakObservable`] reference to the same inner value.
    pub fn downgrade(&self) -> WeakObservable<T, L> {
        WeakObservable {
            state: Arc::downgrade(&self.state),
            _num_clones: Arc::downgrade(&self._num_clones),
        }
    }
}

impl<T, L: Lock> Clone for Observable<T, L> {
    fn clone(&self) -> Self {
        Self { state: self.state.clone(), _num_clones: self._num_clones.clone() }
    }
}

impl<T, L: Lock> fmt::Debug for Observable<T, L>
where
    L::RwLock<ObservableState<T>>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Observable")
            .field("state", &self.state)
            .field("_num_clones", &self._num_clones)
            .finish()
    }
}

impl<T, L> Default for Observable<T, L>
where
    T: Default,
    L: Lock,
{
    fn default() -> Self {
        let rwlock = L::new_rwlock(ObservableState::new(T::default()));
        Self::from_inner(Arc::new(rwlock))
    }
}

impl<T, L: Lock> Drop for Observable<T, L> {
    fn drop(&mut self) {
        // Only close the state if there are no other clones of this
        // `Observable`.
        if Arc::strong_count(&self._num_clones) == 1 {
            // If there are no other clones, obtaining a read lock can't fail.
            L::read_noblock(&self.state).close();
        }
    }
}

/// A weak reference to a shared [`Observable`].
///
/// This type is only useful in niche cases, since one generally shouldn't nest
/// interior-mutable types in observables, which includes observables
/// themselves.
///
/// See [`std::sync::Weak`] for a general explanation of weak references.
pub struct WeakObservable<T, L: Lock = SyncLock> {
    state: Weak<L::RwLock<ObservableState<T>>>,
    _num_clones: Weak<()>,
}

impl<T, L: Lock> WeakObservable<T, L> {
    /// Attempt to upgrade the `WeakObservable` into an `Observable`.
    ///
    /// Returns `None` if the inner value has already been dropped.
    pub fn upgrade(&self) -> Option<Observable<T, L>> {
        let state = Weak::upgrade(&self.state)?;
        let _num_clones = Weak::upgrade(&self._num_clones)?;
        Some(Observable { state, _num_clones })
    }
}

impl<T, L: Lock> Clone for WeakObservable<T, L> {
    fn clone(&self) -> Self {
        Self { state: self.state.clone(), _num_clones: self._num_clones.clone() }
    }
}

impl<T, L: Lock> fmt::Debug for WeakObservable<T, L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WeakObservable").finish_non_exhaustive()
    }
}

/// A write guard for the inner value of an observable.
///
/// Note that as long as an `ObservableWriteGuard` is kept alive, the associated
/// [`Observable`] is locked and can not be updated except through that guard.
#[must_use]
#[clippy::has_significant_drop]
pub struct ObservableWriteGuard<'a, T: 'a, L: Lock = SyncLock> {
    inner: L::RwLockWriteGuard<'a, ObservableState<T>>,
}

impl<'a, T: 'a, L: Lock> ObservableWriteGuard<'a, T, L> {
    fn new(inner: L::RwLockWriteGuard<'a, ObservableState<T>>) -> Self {
        Self { inner }
    }

    /// Set the inner value to the given `value`, notify subscribers and return
    /// the previous value.
    pub fn set(this: &mut Self, value: T) -> T {
        this.inner.set(value)
    }

    /// Set the inner value to the given `value` if it doesn't compare equal to
    /// the existing value.
    ///
    /// If the inner value is set, subscribers are notified and
    /// `Some(previous_value)` is returned. Otherwise, `None` is returned.
    pub fn set_if_not_eq(this: &mut Self, value: T) -> Option<T>
    where
        T: PartialEq,
    {
        this.inner.set_if_not_eq(value)
    }

    /// Set the inner value to the given `value` if it has a different hash than
    /// the existing value.
    ///
    /// If the inner value is set, subscribers are notified and
    /// `Some(previous_value)` is returned. Otherwise, `None` is returned.
    pub fn set_if_hash_not_eq(this: &mut Self, value: T) -> Option<T>
    where
        T: Hash,
    {
        this.inner.set_if_hash_not_eq(value)
    }

    /// Set the inner value to a `Default` instance of its type, notify
    /// subscribers and return the previous value.
    ///
    /// Shorthand for `Observable::set(this, T::default())`.
    pub fn take(this: &mut Self) -> T
    where
        T: Default,
    {
        Self::set(this, T::default())
    }

    /// Update the inner value and notify subscribers.
    ///
    /// Note that even if the inner value is not actually changed by the
    /// closure, subscribers will be notified as if it was. Use
    /// [`update_if`][Self::update_if] if you want to conditionally mutate the
    /// inner value.
    pub fn update(this: &mut Self, f: impl FnOnce(&mut T)) {
        this.inner.update(f);
    }

    /// Maybe update the inner value and notify subscribers if it changed.
    ///
    /// The closure given to this function must return `true` if subscribers
    /// should be notified of a change to the inner value.
    pub fn update_if(this: &mut Self, f: impl FnOnce(&mut T) -> bool) {
        this.inner.update_if(f);
    }
}

impl<T: fmt::Debug> fmt::Debug for ObservableWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T, L: Lock> ops::Deref for ObservableWriteGuard<'_, T, L> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.get()
    }
}
