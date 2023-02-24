use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::Stream;
use readlock::SharedReadLock;
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};

/// A subscriber for updates of an [`Observable`].
///
/// Use its [`Stream`] implementation to interact with it (futures-util and
/// other futures-related crates have extension traits with convenience
/// methods).
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
        self.poll_notification_stream(cx).map(|ready| ready.map(|_| self.read_lock.lock().clone()))
    }
}
