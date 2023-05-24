use std::{
    fmt,
    hash::Hash,
    mem::MaybeUninit,
    ops,
    sync::{Arc, RwLock, RwLockWriteGuard, Weak},
};

use readlock::{SharedReadGuard, SharedReadLock};

use crate::{state::ObservableState, LazyObservableReadGuard, LazySubscriber};

/// A value whose changes will be broadcast to subscribers.
///
/// Unlike [`unique::Observable`](crate::unique::Observable), this `Observable`
/// can be `Clone`d but does't dereference to `T`. Because of the latter, it has
/// regular methods to access or modify the inner value.
#[derive(Debug)]
pub struct LazyObservable<T> {
    state: Arc<RwLock<ObservableState<MaybeUninit<T>>>>,
    /// Ugly hack to track the amount of clones of this observable,
    /// *excluding subscribers*.
    _num_clones: Arc<()>,
}

impl<T> LazyObservable<T> {
    /// Create a new `LazyObservable`.
    #[must_use]
    pub fn new() -> Self {
        Self::from_inner(Arc::new(RwLock::new(ObservableState::new(MaybeUninit::uninit()))))
    }

    pub(crate) fn from_inner(
        state: Arc<RwLock<ObservableState<MaybeUninit<T>>>>,
    ) -> LazyObservable<T> {
        Self { state, _num_clones: Arc::new(()) }
    }

    /// Obtain a new subscriber.
    ///
    /// Calling `.next().await` or `.next_ref().await` on the returned
    /// subscriber only resolves once the inner value has been updated again
    /// after the call to `subscribe`.
    ///
    /// See [`subscribe_reset`][Self::subscribe_reset] if you want to obtain a
    /// subscriber that immediately yields without any updates.
    pub fn subscribe(&self) -> LazySubscriber<T> {
        let version = self.state.read().unwrap().version();
        LazySubscriber::new(SharedReadLock::from_inner(Arc::clone(&self.state)), version)
    }

    /// Obtain a new subscriber that immediately yields.
    ///
    /// `.subscribe_reset()` is equivalent to `.subscribe()` with a subsequent
    /// call to [`.reset()`][LazySubscriber::reset] on the returned subscriber.
    ///
    /// In contrast to [`subscribe`][Self::subscribe], calling `.next().await`
    /// or `.next_ref().await` on the returned subscriber before updating the
    /// inner value yields the current value instead of waiting. Further calls
    /// to either of the two will wait for updates.
    pub fn subscribe_reset(&self) -> LazySubscriber<T> {
        LazySubscriber::new(SharedReadLock::from_inner(Arc::clone(&self.state)), 0)
    }

    /// Get a clone of the inner value.
    pub fn get(&self) -> Option<T>
    where
        T: Clone,
    {
        self.read().map(|lock| lock.clone())
    }

    /// Read the inner value.
    ///
    /// While the returned read guard is alive, nobody can update the inner
    /// value. If you want to update the value based on the previous value, do
    /// **not** use this method because it can cause races with other clones of
    /// the same `Observable`. Instead, call of of the `update_` methods, or
    /// if that doesn't fit your use case, call [`write`][Self::write] and
    /// update the value through the write guard it returns.
    pub fn read(&self) -> Option<LazyObservableReadGuard<'_, T>> {
        LazyObservableReadGuard::new(SharedReadGuard::from_inner(self.state.read().unwrap()))
    }

    /// Get a write guard to the inner value.
    ///
    /// This can be used to set a new value based on the existing value. The
    /// returned write guard dereferences (immutably) to the inner type, and has
    /// associated functions to update it.
    pub fn write(&self) -> LazyObservableWriteGuard<'_, T> {
        LazyObservableWriteGuard::new(self.state.write().unwrap())
    }

    /// Set the inner value to the given `value`, notify subscribers and return
    /// the previous value.
    pub fn set(&self, value: T) -> Option<T> {
        self.state.write().unwrap().init_or_set(value)
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
        self.state.write().unwrap().init_or_set_if_not_eq(value)
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
        self.state.write().unwrap().init_or_set_if_hash_not_eq(value)
    }

    /// Set the inner value to a `Default` instance of its type, notify
    /// subscribers and return the previous value.
    ///
    /// Shorthand for `observable.set(T::default())`.
    pub fn take(&self) -> Option<T>
    where
        T: Default,
    {
        self.set(T::default())
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
    pub fn downgrade(&self) -> WeakLazyObservable<T> {
        WeakLazyObservable {
            state: Arc::downgrade(&self.state),
            _num_clones: Arc::downgrade(&self._num_clones),
        }
    }
}

impl<T> Clone for LazyObservable<T> {
    fn clone(&self) -> Self {
        Self { state: self.state.clone(), _num_clones: self._num_clones.clone() }
    }
}

impl<T> Default for LazyObservable<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for LazyObservable<T> {
    fn drop(&mut self) {
        // Only close the state if there are no other clones of this
        // `Observable`.
        if Arc::strong_count(&self._num_clones) == 1 {
            self.state.write().unwrap().close();
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
#[derive(Debug)]
pub struct WeakLazyObservable<T> {
    state: Weak<RwLock<ObservableState<MaybeUninit<T>>>>,
    _num_clones: Weak<()>,
}

impl<T> WeakLazyObservable<T> {
    /// Attempt to upgrade the `WeakObservable` into an `Observable`.
    ///
    /// Returns `None` if the inner value has already been dropped.
    pub fn upgrade(&self) -> Option<LazyObservable<T>> {
        let state = Weak::upgrade(&self.state)?;
        let _num_clones = Weak::upgrade(&self._num_clones)?;
        Some(LazyObservable { state, _num_clones })
    }
}

impl<T> Clone for WeakLazyObservable<T> {
    fn clone(&self) -> Self {
        Self { state: self.state.clone(), _num_clones: self._num_clones.clone() }
    }
}

/// A write guard for the inner value of an observable.
///
/// Note that as long as an `ObservableWriteGuard` is kept alive, the associated
/// [`Observable`] is locked and can not be updated except through that guard.
#[must_use]
#[clippy::has_significant_drop]
pub enum LazyObservableWriteGuard<'a, T> {
    /// The observable hasn't been initialized yet.
    Empty(EmptyLazyObservableWriteGuard<'a, T>),
    /// The observable is initialized, i.e. holds a value.
    Initialized(InitializedLazyObservableWriteGuard<'a, T>),
}

impl<'a, T> LazyObservableWriteGuard<'a, T> {
    fn new(inner: RwLockWriteGuard<'a, ObservableState<MaybeUninit<T>>>) -> Self {
        if inner.is_initialized() {
            Self::Empty(EmptyLazyObservableWriteGuard { inner })
        } else {
            Self::Initialized(InitializedLazyObservableWriteGuard { inner })
        }
    }

    fn inner_mut(&mut self) -> &mut RwLockWriteGuard<'a, ObservableState<MaybeUninit<T>>> {
        match self {
            Self::Empty(guard) => &mut guard.inner,
            Self::Initialized(guard) => &mut guard.inner,
        }
    }

    /// Set the inner value to the given `value`, notify subscribers and return
    /// the previous value, if any.
    pub fn set(&mut self, value: T) -> Option<T> {
        self.inner_mut().init_or_set(value)
    }

    /// Set the inner value to the given `value` if it doesn't compare equal to
    /// the existing value.
    ///
    /// If the inner value is set, subscribers are notified and
    /// `Some(previous_value)` is returned. Otherwise, `None` is returned.
    pub fn set_if_not_eq(&mut self, value: T) -> Option<T>
    where
        T: PartialEq,
    {
        self.inner_mut().init_or_set_if_not_eq(value)
    }

    /// Set the inner value to the given `value` if it has a different hash than
    /// the existing value.
    ///
    /// If the inner value is set, subscribers are notified and
    /// `Some(previous_value)` is returned. Otherwise, `None` is returned.
    pub fn set_if_hash_not_eq(&mut self, value: T) -> Option<T>
    where
        T: Hash,
    {
        self.inner_mut().init_or_set_if_hash_not_eq(value)
    }
}

impl<T: fmt::Debug> fmt::Debug for LazyObservableWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LazyObservableWriteGuard::Empty(guard) => guard.fmt(f),
            LazyObservableWriteGuard::Initialized(guard) => guard.fmt(f),
        }
    }
}

/// A write guard for a lazy observable that hasn't been initialized yet.
///
/// Note that as long as an `ObservableWriteGuard` is kept alive, the associated
/// [`Observable`] is locked and can not be updated except through that guard.
#[must_use]
#[clippy::has_significant_drop]
pub struct EmptyLazyObservableWriteGuard<'a, T> {
    inner: RwLockWriteGuard<'a, ObservableState<MaybeUninit<T>>>,
}

impl<'a, T> EmptyLazyObservableWriteGuard<'a, T> {
    /// Set the inner value to the given `value` and notify subscribers.
    pub fn set(mut self, value: T) -> InitializedLazyObservableWriteGuard<'a, T> {
        self.inner.init_or_set(value);
        InitializedLazyObservableWriteGuard { inner: self.inner }
    }
}

impl<T: fmt::Debug> fmt::Debug for EmptyLazyObservableWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

/// A write guard for a lazy observable that has been initialized.
///
/// Note that as long as an `ObservableWriteGuard` is kept alive, the associated
/// [`Observable`] is locked and can not be updated except through that guard.
#[must_use]
#[clippy::has_significant_drop]
pub struct InitializedLazyObservableWriteGuard<'a, T> {
    inner: RwLockWriteGuard<'a, ObservableState<MaybeUninit<T>>>,
}

impl<'a, T> InitializedLazyObservableWriteGuard<'a, T> {
    /// Set the inner value to the given `value`, notify subscribers and return
    /// the previous value.
    pub fn set(this: &mut Self, value: T) -> T {
        this.inner.init_or_set(value).unwrap()
    }

    /// Set the inner value to the given `value` if it doesn't compare equal to
    /// the existing value.
    ///
    /// If the inner value is set, subscribers are notified and
    /// `Some(previous_value)` is returned. Otherwise, `None` is returned.
    pub fn set_if_not_eq(this: &mut Self, value: T) -> T
    where
        T: PartialEq,
    {
        this.inner.init_or_set_if_not_eq(value).unwrap()
    }

    /// Set the inner value to the given `value` if it has a different hash than
    /// the existing value.
    ///
    /// If the inner value is set, subscribers are notified and
    /// `Some(previous_value)` is returned. Otherwise, `None` is returned.
    pub fn set_if_hash_not_eq(this: &mut Self, value: T) -> T
    where
        T: Hash,
    {
        this.inner.init_or_set_if_hash_not_eq(value).unwrap()
    }
}

impl<T: fmt::Debug> fmt::Debug for InitializedLazyObservableWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T> ops::Deref for InitializedLazyObservableWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: This type is only ever created with initialized inner state
        unsafe { self.inner.get().assume_init_ref() }
    }
}
