//! Add observability to your Rust types!
//!
//! This crate implements a basic form of the [Observer pattern][] for Rust.
//! It provides [`Observable<T>`] as a type that uniquely owns an inner value
//! `T` and broadcasts changes to any associated [`Subscriber<T>`]s.
//! `Subscriber`s can currently only be polled for updates using `async` /
//! `.await`, but this may change in the future.
//!
//! Here is a quick walk-through:
//!
//! ```
//! use eyeball::Observable;
//!
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() {
//! let mut observable = Observable::new("A".to_owned());
//! // Observable has no methods of its own, as those could conflict
//! // with methods of the inner type, which it `Deref`erences to.
//! let mut subscriber1 = Observable::subscribe(&observable);
//! let mut subscriber2 = Observable::subscribe(&observable);
//!
//! // You can get the current value from a subscriber without waiting
//! // for updates.
//! assert_eq!(subscriber1.get(), "A");
//!
//! Observable::set(&mut observable, "B".to_owned());
//! // `.next().await` will wait for the next update, then return the
//! // new value.
//! assert_eq!(subscriber1.next().await, Some("B".to_owned()));
//!
//! // If multiple updates have happened without the subscriber being
//! // polled, the next poll will skip all but the latest.
//! Observable::set(&mut observable, "C".to_owned());
//! assert_eq!(subscriber1.next().await, Some("C".to_owned()));
//! assert_eq!(subscriber2.next().await, Some("C".to_owned()));
//!
//! // You can even obtain the value without cloning the value, by
//! // using `.read()` (no waiting) or `.next_ref().await` (waits for
//! // the next update).
//! // If you restrict yourself to these methods, you can even use
//! // `Observable` with inner types that don't implement the `Clone`
//! // trait.
//! // However, note that while a read guard returned by `.read()` or
//! // `.next_ref().await` is alive, updating the observable is
//! // blocked.
//! Observable::set(&mut observable, "D".to_owned());
//! {
//!     let guard = subscriber1.next_ref().await.unwrap();
//!     assert_eq!(*guard, "D");
//! }
//!
//! // The latest value is kept alive by subscribers when the
//! // `Observable` is dropped.
//! drop(observable);
//! assert_eq!(subscriber1.get(), "D");
//! assert_eq!(*subscriber2.read(), "D");
//! # }
//! ```
//!
//! If you want to modify an observable from multiple places, have a look at the
//! [`SharedObservable`] type, which makes working with an `Observable` behind
//! an `Arc<RwLock<_>>` a little more convenient.
//!
//! Cargo features:
//!
//! - `tracing`: Emit [tracing] events when updates are sent out
//!
//! [Observer pattern]: https://en.wikipedia.org/wiki/Observer_pattern
#![warn(missing_debug_implementations, missing_docs)]
#![allow(clippy::new_without_default)]

mod observable;
mod shared_observable;
mod subscriber;

pub use observable::Observable;
pub use shared_observable::{ObservableLock, SharedObservable, SharedObservableBase};
pub use subscriber::{Subscriber, SubscriberReadGuard};
