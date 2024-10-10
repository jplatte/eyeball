use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

use crate::state::ObservableState;

pub trait Lock {
    type RwLock<T>;
    type RwLockReadGuard<'a, T>: Deref<Target = T>
    where
        T: 'a;
    type RwLockWriteGuard<'a, T>: DerefMut<Target = T>
    where
        T: 'a;
    type Shared<T>: Deref<Target = T>;
    type SharedReadGuard<'a, T>: Deref<Target = T>
    where
        T: 'a;
    type SubscriberState<S>;

    fn new_rwlock<T>(value: T) -> Self::RwLock<T>;
    fn read_noblock<T>(lock: &Self::RwLock<T>) -> Self::RwLockReadGuard<'_, T>;

    fn new_shared<T>(value: T) -> Self::Shared<T>;
    fn shared_read_count<T>(shared: &Self::Shared<T>) -> usize;
    fn shared_into_inner<T>(shared: Self::Shared<T>) -> Arc<Self::RwLock<T>>;

    fn drop_waker<S>(state: &Self::SubscriberState<S>, observed_version: u64, waker_key: usize);
}

/// Marker type for using a synchronous lock for the inner value.
#[allow(missing_debug_implementations)]
pub enum SyncLock {}

impl Lock for SyncLock {
    type RwLock<T> = std::sync::RwLock<T>;
    type RwLockReadGuard<'a, T>
        = std::sync::RwLockReadGuard<'a, T>
    where
        T: 'a;
    type RwLockWriteGuard<'a, T>
        = std::sync::RwLockWriteGuard<'a, T>
    where
        T: 'a;
    type Shared<T> = readlock::Shared<T>;
    type SharedReadGuard<'a, T>
        = readlock::SharedReadGuard<'a, T>
    where
        T: 'a;
    type SubscriberState<S> = readlock::SharedReadLock<ObservableState<S>>;

    fn new_rwlock<T>(value: T) -> Self::RwLock<T> {
        Self::RwLock::new(value)
    }
    fn read_noblock<T>(lock: &Self::RwLock<T>) -> Self::RwLockReadGuard<'_, T> {
        lock.try_read().unwrap()
    }

    fn new_shared<T>(value: T) -> Self::Shared<T> {
        Self::Shared::new(value)
    }
    fn shared_read_count<T>(shared: &Self::Shared<T>) -> usize {
        Self::Shared::read_count(shared)
    }
    fn shared_into_inner<T>(shared: Self::Shared<T>) -> Arc<Self::RwLock<T>> {
        Self::Shared::into_inner(shared)
    }

    fn drop_waker<S>(state: &Self::SubscriberState<S>, observed_version: u64, waker_key: usize) {
        if let Ok(guard) = state.try_lock() {
            guard.drop_waker(observed_version, waker_key);
        }
    }
}

/// Marker type for using an asynchronous lock for the inner value.
#[cfg(feature = "async-lock")]
#[allow(missing_debug_implementations)]
pub enum AsyncLock {}

#[cfg(feature = "async-lock")]
impl Lock for AsyncLock {
    type RwLock<T> = tokio::sync::RwLock<T>;
    type RwLockReadGuard<'a, T>
        = tokio::sync::RwLockReadGuard<'a, T>
    where
        T: 'a;
    type RwLockWriteGuard<'a, T>
        = tokio::sync::RwLockWriteGuard<'a, T>
    where
        T: 'a;
    type Shared<T> = readlock_tokio::Shared<T>;
    type SharedReadGuard<'a, T>
        = readlock_tokio::SharedReadGuard<'a, T>
    where
        T: 'a;
    type SubscriberState<S> = crate::subscriber::async_lock::AsyncSubscriberState<S>;

    fn new_rwlock<T>(value: T) -> Self::RwLock<T> {
        Self::RwLock::new(value)
    }
    fn read_noblock<T>(lock: &Self::RwLock<T>) -> Self::RwLockReadGuard<'_, T> {
        lock.try_read().unwrap()
    }

    fn new_shared<T>(value: T) -> Self::Shared<T> {
        Self::Shared::new(value)
    }
    fn shared_read_count<T>(shared: &Self::Shared<T>) -> usize {
        Self::Shared::read_count(shared)
    }
    fn shared_into_inner<T>(shared: Self::Shared<T>) -> Arc<Self::RwLock<T>> {
        Self::Shared::into_inner(shared)
    }

    fn drop_waker<S>(state: &Self::SubscriberState<S>, observed_version: u64, waker_key: usize) {
        state.drop_waker(observed_version, waker_key);
    }
}
