use std::{
    fmt,
    future::{poll_fn, Future},
    pin::Pin,
    task::{ready, Context, Poll},
};

use futures_core::Stream;
use tokio_util::sync::ReusableBoxFuture;

use super::{Next, Subscriber};
use crate::{state::ObservableState, AsyncLock, ObservableReadGuard};

pub struct AsyncSubscriberState<S> {
    inner: readlock_tokio::SharedReadLock<S>,
    get_lock: ReusableBoxFuture<'static, readlock_tokio::OwnedSharedReadGuard<S>>,
}

impl<S: Send + Sync + 'static> Clone for AsyncSubscriberState<S> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            get_lock: ReusableBoxFuture::new(self.inner.clone().lock_owned()),
        }
    }
}

impl<S: fmt::Debug> fmt::Debug for AsyncSubscriberState<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T: Send + Sync + 'static> Subscriber<T, AsyncLock> {
    pub(crate) fn new_async(
        inner: readlock_tokio::SharedReadLock<ObservableState<T>>,
        version: u64,
    ) -> Self {
        let get_lock = ReusableBoxFuture::new(inner.clone().lock_owned());
        Self { state: AsyncSubscriberState { inner, get_lock }, observed_version: version }
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
    pub async fn next(&mut self) -> Option<T>
    where
        T: Clone,
    {
        self.next_ref().await.map(|read_guard| read_guard.clone())
    }

    /// Get a clone of the inner value without waiting for an update.
    ///
    /// If the returned value has not been observed by this subscriber before,
    /// it is marked as observed such that a subsequent call of
    /// [`next`][Self::next] or [`next_ref`][Self::next_ref] won't return the
    /// same value again. See [`get`][Self::get] for a function that doesn't
    /// mark the value as observed.
    #[must_use]
    pub async fn next_now(&mut self) -> T
    where
        T: Clone,
    {
        let lock = self.state.inner.lock().await;
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
    pub async fn get(&self) -> T
    where
        T: Clone,
    {
        self.read().await.clone()
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
    pub async fn next_ref(&mut self) -> Option<ObservableReadGuard<'_, T, AsyncLock>> {
        // Unclear how to implement this as a named future.
        poll_fn(|cx| self.poll_update(cx)).await?;
        Some(self.next_ref_now().await)
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
    pub async fn next_ref_now(&mut self) -> ObservableReadGuard<'_, T, AsyncLock> {
        let lock = self.state.inner.lock().await;
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
    pub async fn read(&self) -> ObservableReadGuard<'_, T, AsyncLock> {
        ObservableReadGuard::new(self.state.inner.lock().await)
    }

    fn poll_update(&mut self, cx: &mut Context<'_>) -> Poll<Option<()>> {
        let state = ready!(self.state.get_lock.poll(cx));
        self.state.get_lock.set(self.state.inner.clone().lock_owned());

        let version = state.version();
        if version == 0 {
            Poll::Ready(None)
        } else if self.observed_version < version {
            self.observed_version = version;
            Poll::Ready(Some(()))
        } else {
            state.add_waker(cx.waker().clone());
            Poll::Pending
        }
    }

    fn poll_next_nopin(&mut self, cx: &mut Context<'_>) -> Poll<Option<T>>
    where
        T: Clone,
    {
        let state = ready!(self.state.get_lock.poll(cx));
        self.state.get_lock.set(self.state.inner.clone().lock_owned());

        let version = state.version();
        if version == 0 {
            Poll::Ready(None)
        } else if self.observed_version < version {
            self.observed_version = version;
            Poll::Ready(Some(state.get().clone()))
        } else {
            state.add_waker(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl<T: Clone + Send + Sync + 'static> Stream for Subscriber<T, AsyncLock> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.poll_next_nopin(cx)
    }
}

impl<T: Clone + Send + Sync + 'static> Future for Next<'_, T, AsyncLock> {
    type Output = Option<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.subscriber.poll_next_nopin(cx)
    }
}
