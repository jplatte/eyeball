use std::{
    fmt,
    future::poll_fn,
    ops,
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::Stream;
use readlock::{SharedReadGuard, SharedReadLock};
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};

#[cfg(doc)]
use crate::observable::Observable;

/// A subscriber for updates of an [`Observable`].
#[derive(Debug)]
pub struct Subscriber<T> {
    read_lock: SharedReadLock<T>,
    notification_stream: BroadcastStream<()>,
}

impl<T> Subscriber<T> {
    pub(crate) fn new(
        read_lock: SharedReadLock<T>,
        notification_stream: BroadcastStream<()>,
    ) -> Self {
        Self { read_lock, notification_stream }
    }

    /// Wait for an update and get a clone of the updated value.
    ///
    /// This method is a convenience so you don't have to import a `Stream`
    /// extension trait such as `futures::StreamExt` or
    /// `tokio_stream::StreamExt`.
    pub async fn next(&mut self) -> Option<T>
    where
        T: Clone,
    {
        poll_fn(|cx| self.poll_notification_stream(cx)).await?;
        Some(self.get())
    }

    /// Wait for an update and get a read lock for the updated value.
    ///
    /// You can use this method to get updates of an [`Observable`] where the
    /// inner type does not implement `Clone`. However, the `Observable`
    /// will be locked (not updateable) while any read locks are alive.
    pub async fn next_ref(&mut self) -> Option<SubscriberReadGuard<'_, T>> {
        poll_fn(|cx| self.poll_notification_stream(cx)).await?;
        Some(self.read())
    }

    /// Get a clone of the inner value without waiting for an update.
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.read().clone()
    }

    /// Lock the inner value for reading without waiting for an update.
    ///
    /// Note that as long as the returned [`SubscriberReadGuard`] is kept alive,
    /// the associated [`Observable`] is locked and can not be updated.
    pub fn read(&self) -> SubscriberReadGuard<'_, T> {
        SubscriberReadGuard::new(self.read_lock.lock())
    }

    /// Poll the inner notification stream.
    ///
    /// Returns `Ready(Some(()))` if there was a notification.
    /// Returns `Ready(None)` if the notification stream was closed.
    fn poll_notification_stream(&mut self, cx: &mut Context<'_>) -> Poll<Option<()>> {
        loop {
            let poll = match Pin::new(&mut self.notification_stream).poll_next(cx) {
                Poll::Ready(Some(Ok(_))) => Poll::Ready(Some(())),
                Poll::Ready(None) => Poll::Ready(None),
                Poll::Ready(Some(Err(BroadcastStreamRecvError::Lagged(_)))) => continue,
                Poll::Pending => Poll::Pending,
            };

            return poll;
        }
    }
}

impl<T: Clone> Stream for Subscriber<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.poll_notification_stream(cx).map(|ready| ready.map(|_| self.get()))
    }
}

/// A read guard that allows you to read the inner value of an observable
/// without cloning.
///
/// Note that as long as a SubscriberReadGuard is kept alive, the associated
/// [`Observable`] is locked and can not be updated.
pub struct SubscriberReadGuard<'a, T> {
    inner: SharedReadGuard<'a, T>,
}

impl<'a, T> SubscriberReadGuard<'a, T> {
    fn new(inner: SharedReadGuard<'a, T>) -> Self {
        Self { inner }
    }
}

impl<T: fmt::Debug> fmt::Debug for SubscriberReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T> ops::Deref for SubscriberReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
