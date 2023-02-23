use std::{
    hash::{Hash, Hasher},
    mem, ops,
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::Stream;
use readlock::{Shared, SharedReadLock};
use tokio::sync::broadcast::{self, Sender};
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};

/// A value whose changes will be broadcast to subscribers.
///
/// `Observable<T>` dereferences to `T`, and does not have methods of its own to
/// not clash with methods of the inner type. Instead, to interact with the
/// `Observable` itself rather than the inner value, use its associated
/// functions (e.g. `Observable::subscribe(observable)`).
#[derive(Debug)]
pub struct Observable<T> {
    value: Shared<T>,
    sender: Sender<()>,
}

impl<T> Observable<T> {
    /// Create a new `Observable` with the given initial value.
    pub fn new(value: T) -> Self {
        let (sender, _) = broadcast::channel(1);
        Self { value: Shared::new(value), sender }
    }

    /// Obtain a new subscriber.
    pub fn subscribe(this: &Self) -> Subscriber<T> {
        let rx = this.sender.subscribe();
        Subscriber::new(Shared::get_read_lock(&this.value), BroadcastStream::new(rx))
    }

    /// Get a reference to the inner value.
    ///
    /// Usually, you don't need to call this function since `Observable<T>`
    /// implements `Deref`. Use this if you want to pass the inner value to a
    /// generic function where the compiler can't infer that you want to have
    /// the `Observable` dereferenced otherwise.
    pub fn get(this: &Self) -> &T {
        &this.value
    }

    /// Set the inner value to the given `value` and notify subscribers.
    pub fn set(this: &mut Self, value: T) {
        *Shared::lock(&mut this.value) = value;
        Self::broadcast_update(this);
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
        let result = mem::replace(&mut *Shared::lock(&mut this.value), value);
        Self::broadcast_update(this);
        result
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
        f(&mut *Shared::lock(&mut this.value));
        Self::broadcast_update(this);
    }

    /// Update the inner value and notify subscribers if the updated value does
    /// not equal the previous value.
    pub fn update_eq(this: &mut Self, f: impl FnOnce(&mut T))
    where
        T: Clone + PartialEq,
    {
        let prev = this.value.clone();
        f(&mut *Shared::lock(&mut this.value));
        if *this.value != prev {
            Self::broadcast_update(this);
        }
    }

    /// Update the inner value and notify subscribers if the hash of the updated
    /// value does not equal the hash of the previous value.
    pub fn update_hash(this: &mut Self, f: impl FnOnce(&mut T))
    where
        T: Hash,
    {
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        this.value.hash(&mut hasher);
        let prev_hash = hasher.finish();

        f(&mut *Shared::lock(&mut this.value));

        let mut hasher = DefaultHasher::new();
        this.value.hash(&mut hasher);
        let new_hash = hasher.finish();

        if prev_hash != new_hash {
            Self::broadcast_update(this);
        }
    }

    fn broadcast_update(this: &Self) {
        let _num_receivers = this.sender.send(()).unwrap_or(0);
        #[cfg(feature = "tracing")]
        if _num_receivers > 0 {
            tracing::debug!("New observable value broadcast to {_num_receivers} receivers");
        }
    }
}

impl<T: Default> Default for Observable<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

// Note: No DerefMut because all mutating must go through inherent methods that
// notify subscribers
impl<T> ops::Deref for Observable<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

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
    fn new(read_lock: SharedReadLock<T>, notification_stream: BroadcastStream<()>) -> Self {
        Self { read_lock, notification_stream }
    }
}

impl<T: Clone> Stream for Subscriber<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            let poll = match Pin::new(&mut self.notification_stream).poll_next(cx) {
                Poll::Ready(Some(Ok(_))) => Poll::Ready(Some(self.read_lock.lock().clone())),
                Poll::Ready(None) => Poll::Ready(None),
                Poll::Ready(Some(Err(BroadcastStreamRecvError::Lagged(_)))) => continue,
                Poll::Pending => Poll::Pending,
            };

            return poll;
        }
    }
}
