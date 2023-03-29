//! This module defines a unique [`Observable`] type that requires `&mut` access
//! to update its inner value but can be dereferenced (immutably).
//!
//! Use this in situations where only a single location in the code should be
//! able to update the inner value.

use std::{hash::Hash, ops};

use readlock::Shared;

use crate::{state::ObservableState, Subscriber};

/// A value whose changes will be broadcast to subscribers.
///
/// `Observable<T>` dereferences to `T`, and does not have methods of its own to
/// not clash with methods of the inner type. Instead, to interact with the
/// `Observable` itself rather than the inner value, use its associated
/// functions (e.g. `Observable::subscribe(observable)`).
#[derive(Debug)]
pub struct Observable<T> {
    state: Shared<ObservableState<T>>,
}

impl<T> Observable<T> {
    /// Create a new `Observable` with the given initial value.
    pub fn new(value: T) -> Self {
        Self { state: Shared::new(ObservableState::new(value)) }
    }

    /// Obtain a new subscriber.
    ///
    /// Calling `.next().await` or `.next_ref().await` on the returned
    /// subscriber only resolves once the inner value has been updated again
    /// after the call to `subscribe`.
    ///
    /// See [`subscribe_reset`][Self::subscribe_reset] if you want to obtain a
    /// subscriber that immediately yields without any updates.
    pub fn subscribe(this: &Self) -> Subscriber<T> {
        Subscriber::new(Shared::get_read_lock(&this.state), this.state.version())
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
    pub fn subscribe_reset(this: &Self) -> Subscriber<T> {
        Subscriber::new(Shared::get_read_lock(&this.state), 0)
    }

    /// Get a reference to the inner value.
    ///
    /// Usually, you don't need to call this function since `Observable<T>`
    /// implements `Deref`. Use this if you want to pass the inner value to a
    /// generic function where the compiler can't infer that you want to have
    /// the `Observable` dereferenced otherwise.
    pub fn get(this: &Self) -> &T {
        this.state.get()
    }

    /// Set the inner value to the given `value`, notify subscribers and return
    /// the previous value.
    pub fn set(this: &mut Self, value: T) -> T {
        Shared::lock(&mut this.state).set(value)
    }

    /// Set the inner value to the given `value` and notify subscribers if the
    /// updated value does not equal the previous value.
    pub fn set_eq(this: &mut Self, value: T)
    where
        T: PartialEq,
    {
        Shared::lock(&mut this.state).set_eq(value);
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

    /// Set the inner value to a `Default` instance of its type, notify
    /// subscribers and return the previous value.
    ///
    /// Shorthand for `Observable::replace(this, T::default())`.
    pub fn take(this: &mut Self) -> T
    where
        T: Default,
    {
        Self::set(this, T::default())
    }

    /// Update the inner value and notify subscribers.
    ///
    /// Note that even if the inner value is not actually changed by the
    /// closure, subscribers will be notified as if it was. Use one of the
    /// other update methods below if you want to conditionally mutate the
    /// inner value.
    pub fn update(this: &mut Self, f: impl FnOnce(&mut T)) {
        Shared::lock(&mut this.state).update(f);
    }

    /// Update the inner value and notify subscribers if the updated value does
    /// not equal the previous value.
    pub fn update_eq(this: &mut Self, f: impl FnOnce(&mut T))
    where
        T: Clone + PartialEq,
    {
        Shared::lock(&mut this.state).update_eq(f);
    }

    /// Update the inner value and notify subscribers if the hash of the updated
    /// value does not equal the hash of the previous value.
    pub fn update_hash(this: &mut Self, f: impl FnOnce(&mut T))
    where
        T: Hash,
    {
        Shared::lock(&mut this.state).update_hash(f);
    }

    /// Get the number of subscribers.
    ///
    /// Be careful when using this. The result is only reliable if it is exactly
    /// `0`, as otherwise it could be incremented right after your call to this
    /// function, before you look at its result or do anything based on that.
    pub fn subscriber_count(&self) -> usize {
        Shared::read_count(&self.state)
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
        self.state.get()
    }
}

impl<T> Drop for Observable<T> {
    fn drop(&mut self) {
        Shared::lock(&mut self.state).close();
    }
}
