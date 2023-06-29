use std::ops::{Deref, DerefMut};

use crate::state::ObservableState;

pub trait Lock {
    type RwLock<T>;
    type RwLockReadGuard<'a, T>: Deref<Target = T>
    where
        T: 'a;
    type RwLockWriteGuard<'a, T>: DerefMut<Target = T>
    where
        T: 'a;
    type SharedReadGuard<'a, T>: Deref<Target = T>
    where
        T: 'a;
    type SubscriberState<S>;

    fn new_rwlock<T>(value: T) -> Self::RwLock<T>;
    fn read_noblock<T>(lock: &Self::RwLock<T>) -> Self::RwLockReadGuard<'_, T>;
}

/// Marker type for using a synchronous lock for the inner value.
#[allow(missing_debug_implementations)]
pub enum SyncLock {}

impl Lock for SyncLock {
    type RwLock<T> = std::sync::RwLock<T>;
    type RwLockReadGuard<'a, T> = std::sync::RwLockReadGuard<'a, T>
    where
        T: 'a;
    type RwLockWriteGuard<'a, T> = std::sync::RwLockWriteGuard<'a, T>
    where
        T: 'a;
    type SharedReadGuard<'a, T> = readlock::SharedReadGuard<'a, T>
    where
        T: 'a;
    type SubscriberState<T> = readlock::SharedReadLock<ObservableState<T>>;

    fn new_rwlock<T>(value: T) -> Self::RwLock<T> {
        Self::RwLock::new(value)
    }
    fn read_noblock<T>(lock: &Self::RwLock<T>) -> Self::RwLockReadGuard<'_, T> {
        lock.try_read().unwrap()
    }
}

/// Marker type for using an asynchronous lock for the inner value.
#[cfg(feature = "async-lock")]
#[allow(missing_debug_implementations)]
pub enum AsyncLock {}

#[cfg(feature = "async-lock")]
impl Lock for AsyncLock {
    type RwLock<T> = tokio::sync::RwLock<T>;
    type RwLockReadGuard<'a, T> = tokio::sync::RwLockReadGuard<'a, T>
    where
        T: 'a;
    type RwLockWriteGuard<'a, T> = tokio::sync::RwLockWriteGuard<'a, T>
    where
        T: 'a;
    type SharedReadGuard<'a, T> = readlock_tokio::SharedReadGuard<'a, T>
    where
        T: 'a;
    type SubscriberState<T> =
        crate::subscriber::async_lock::AsyncSubscriberState<ObservableState<T>>;

    fn new_rwlock<T>(value: T) -> Self::RwLock<T> {
        Self::RwLock::new(value)
    }
    fn read_noblock<T>(lock: &Self::RwLock<T>) -> Self::RwLockReadGuard<'_, T> {
        lock.try_read().unwrap()
    }
}
