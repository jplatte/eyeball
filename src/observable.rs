use std::{
    mem, ops,
    pin::Pin,
    task::{Context, Poll},
};

use futures_channel::mpsc;
use futures_core::Stream;
use pin_project_lite::pin_project;

use crate::notifier::Notifier;

/// A value whose changes will be broadcast to subscribers.
#[derive(Debug)]
pub struct Observable<T> {
    value: T,
    notifier: Notifier<T>,
}

impl<T> Observable<T> {
    /// Create a new `Observable` with the given initial value.
    pub const fn new(value: T) -> Self {
        Self { value, notifier: Notifier::new() }
    }
}

impl<T: Clone> Observable<T> {
    /// Set the inner value to the given `value` and notify subscribers.
    pub fn set(&mut self, value: T) {
        self.replace(value);
    }

    /// Set the inner value to the given `value`, notify subscribers and return the previous value.
    pub fn replace(&mut self, value: T) -> T {
        self.notifier.notify(|| value.clone());
        mem::replace(&mut self.value, value)
    }

    /// Obtain a new subscriber.
    ///
    /// It will immediately receive an update with the current value.
    pub fn subscribe(&mut self) -> ObservableSubscriber<T> {
        let (tx, rx) = mpsc::unbounded();
        tx.unbounded_send(self.value.clone()).unwrap();
        self.notifier.add_sender(tx);

        ObservableSubscriber::new(rx)
    }
}

// Note: No DerefMut because all mutating must go through inherent methods that notify subscribers
impl<T: Clone> ops::Deref for Observable<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

pin_project! {
    /// A subscriber for updates of an [`Observable`].
    ///
    /// Use its [`Stream`] implementation to interact with it (futures-util and other
    /// futures-related crates have extension traits with convenience methods).
    pub struct ObservableSubscriber<T> {
        #[pin]
        inner: mpsc::UnboundedReceiver<T>,
    }
}

impl<T> ObservableSubscriber<T> {
    const fn new(inner: mpsc::UnboundedReceiver<T>) -> Self {
        Self { inner }
    }
}

impl<T> Stream for ObservableSubscriber<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().inner.poll_next(cx)
    }
}
