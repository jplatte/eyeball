use std::{
    cell::{Ref, RefCell, RefMut},
    hash::Hash,
    ops::{Deref, DerefMut},
    rc::Rc,
    sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use crate::{Observable, Subscriber};

/// A common type of shared observable, where shared ownership is achieved via
/// `Arc` in addition to shared mutation via `RwLock`.
pub type SharedObservable<T> = SharedObservableBase<Arc<RwLock<Observable<T>>>>;

/// A wrapper around a lock that contains an [`Observable`].
///
/// You can use this type to remove some of the boilerplate of obtaining locks
/// for various operations on the inner [`Observable`]. It does not provide any
/// special capabilities compared to using `I` directly though.
///
/// Here are some examples for possible types to use as `I` (where `_` is always
/// `Observable<T>` for an arbitrary `T`):
///
/// - `Arc<RwLock<_>>` ([`SharedObservable`])
/// - `Arc<Mutex<_>>`
/// - just `RwLock<_>` or `Mutex<_>`
/// - `Rc<RefCell<_>>` or just `RefCell<_>` if you only want to write from a
///   single thread but still need shared mutability and possibly shared
///   ownership
///
/// It is recommended to create a type alias to the kind of shared observable
/// you want, if it is something other than `Arc<RwLock<_>>`.
#[derive(Clone, Debug)]
pub struct SharedObservableBase<I>(pub I);

impl<T, L> SharedObservableBase<L>
where
    L: ObservableLock<Item = T>,
{
    /// Create a new `Observable` with the given initial value.
    pub fn new(value: T) -> Self {
        Self(L::from_observable(Observable::new(value)))
    }

    /// Obtain a new subscriber.
    pub fn subscribe(&self) -> Subscriber<T> {
        Observable::subscribe(&self.read())
    }

    /// Lock the inner [`Observable`] for reading.
    pub fn read(&self) -> L::ReadGuard<'_> {
        self.0.read()
    }

    /// Lock the inner [`Observable`] for writing.
    pub fn write(&self) -> L::WriteGuard<'_> {
        self.0.write()
    }

    /// Get a clone of the inner value.
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.read().clone()
    }

    /// Set the inner value to the given `value` and notify subscribers.
    pub fn set(&self, value: T) {
        Observable::set(&mut self.write(), value);
    }

    /// Set the inner value to the given `value` and notify subscribers if the
    /// updated value does not equal the previous value.
    pub fn set_eq(&self, value: T)
    where
        T: Clone + PartialEq,
    {
        Observable::set_eq(&mut self.write(), value);
    }

    /// Set the inner value to the given `value` and notify subscribers if the
    /// hash of the updated value does not equal the hash of the previous
    /// value.
    pub fn set_hash(&self, value: T)
    where
        T: Hash,
    {
        Observable::set_hash(&mut self.write(), value);
    }

    /// Set the inner value to the given `value`, notify subscribers and return
    /// the previous value.
    pub fn replace(&self, value: T) -> T {
        Observable::replace(&mut self.write(), value)
    }

    /// Set the inner value to a `Default` instance of its type, notify
    /// subscribers and return the previous value.
    ///
    /// Shorthand for `Observable::replace(this, T::default())`.
    pub fn take(&self) -> T
    where
        T: Default,
    {
        Observable::take(&mut self.write())
    }

    /// Update the inner value and notify subscribers.
    ///
    /// Note that even if the inner value is not actually changed by the
    /// closure, subscribers will be notified as if it was. Use one of the
    /// other update methods below if you want to conditionally mutate the
    /// inner value.
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        Observable::update(&mut self.write(), f);
    }

    /// Update the inner value and notify subscribers if the updated value does
    /// not equal the previous value.
    pub fn update_eq(&self, f: impl FnOnce(&mut T))
    where
        T: Clone + PartialEq,
    {
        Observable::update_eq(&mut self.write(), f);
    }

    /// Update the inner value and notify subscribers if the hash of the updated
    /// value does not equal the hash of the previous value.
    pub fn update_hash(&self, f: impl FnOnce(&mut T))
    where
        T: Hash,
    {
        Observable::update_hash(&mut self.write(), f);
    }
}

impl<T, I> Default for SharedObservableBase<I>
where
    T: Default,
    I: ObservableLock<Item = T>,
{
    fn default() -> Self {
        Self::new(T::default())
    }
}

/// A lock that holds an [`Observable`].
pub trait ObservableLock {
    /// The type inside the [`Observable`].
    type Item;

    /// The lock's read guard type. May be the same as the write guard type.
    type ReadGuard<'a>: Deref<Target = Observable<Self::Item>>
    where
        Self: 'a;

    /// The lock's write guard type. May be the same as the write guard type.
    type WriteGuard<'a>: DerefMut<Target = Observable<Self::Item>>
    where
        Self: 'a;

    /// Create a new lock from the given [`Observable`].
    fn from_observable(ob: Observable<Self::Item>) -> Self;

    /// Lock `self` for reading.
    fn read(&self) -> Self::ReadGuard<'_>;

    /// Lock `self` for writing.
    fn write(&self) -> Self::WriteGuard<'_>;
}

impl<T> ObservableLock for RefCell<Observable<T>> {
    type Item = T;
    type ReadGuard<'a> = Ref<'a, Observable<T>>
        where Self: 'a;
    type WriteGuard<'a> = RefMut<'a, Observable<T>>
    where
        Self: 'a;

    fn from_observable(ob: Observable<Self::Item>) -> Self {
        Self::new(ob)
    }

    fn read(&self) -> Self::ReadGuard<'_> {
        self.borrow()
    }

    fn write(&self) -> Self::WriteGuard<'_> {
        self.borrow_mut()
    }
}

impl<T> ObservableLock for Mutex<Observable<T>> {
    type Item = T;
    type ReadGuard<'a> = MutexGuard<'a, Observable<T>>
    where
        Self: 'a;
    type WriteGuard<'a> = MutexGuard<'a, Observable<T>>
    where
        Self: 'a;

    fn from_observable(ob: Observable<Self::Item>) -> Self {
        Self::new(ob)
    }

    fn read(&self) -> Self::ReadGuard<'_> {
        self.lock().unwrap()
    }

    fn write(&self) -> Self::WriteGuard<'_> {
        self.lock().unwrap()
    }
}

impl<T> ObservableLock for RwLock<Observable<T>> {
    type Item = T;
    type ReadGuard<'a> = RwLockReadGuard<'a, Observable<T>>
    where
        Self: 'a;
    type WriteGuard<'a> = RwLockWriteGuard<'a, Observable<T>>
    where
        Self: 'a;

    fn from_observable(ob: Observable<Self::Item>) -> Self {
        Self::new(ob)
    }

    fn read(&self) -> Self::ReadGuard<'_> {
        self.read().unwrap()
    }

    fn write(&self) -> Self::WriteGuard<'_> {
        self.write().unwrap()
    }
}

impl<L: ObservableLock> ObservableLock for Rc<L> {
    type Item = L::Item;
    type ReadGuard<'a> = L::ReadGuard<'a>
    where
        Self: 'a;
    type WriteGuard<'a> = L::WriteGuard<'a>
    where
        Self: 'a;

    fn from_observable(ob: Observable<Self::Item>) -> Self {
        Self::new(L::from_observable(ob))
    }

    fn read(&self) -> Self::ReadGuard<'_> {
        (**self).read()
    }

    fn write(&self) -> Self::WriteGuard<'_> {
        (**self).write()
    }
}

impl<L: ObservableLock> ObservableLock for Arc<L> {
    type Item = L::Item;
    type ReadGuard<'a> = L::ReadGuard<'a>
    where
        Self: 'a;
    type WriteGuard<'a> = L::WriteGuard<'a>
    where
        Self: 'a;

    fn from_observable(ob: Observable<Self::Item>) -> Self {
        Self::new(L::from_observable(ob))
    }

    fn read(&self) -> Self::ReadGuard<'_> {
        (**self).read()
    }

    fn write(&self) -> Self::WriteGuard<'_> {
        (**self).write()
    }
}
