//! Add observability to your Rust types!
//!
//! This crate implements a basic form of the [Observer pattern][] for Rust.
//! It provides [`unique::Observable<T>`] as a type that semi-transparently
//! wraps an inner value `T` and broadcasts changes to any associated
//! [`Subscriber<T>`]s. `Subscriber`s can currently only be polled for updates
//! using `async` / `.await`, but this may change in the future.
//!
//! There is also [`shared::Observable<T>`] as another variation which
//! implements [`Clone`] but not [`Deref`][std::ops::Deref]. It is more
//! ergonomic and efficient than putting a `unique::Observable` inside of
//! `Arc<RwLock<_>>` for updating the value from multiple places in the code.
//!
//! Here is a quick walk-through:
//!
//! ```
//! use eyeball::unique::Observable;
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
//! Cargo features:
//!
//! - `tracing`: Emit [tracing] events when updates are sent out
//!
//! [Observer pattern]: https://en.wikipedia.org/wiki/Observer_pattern
#![warn(missing_debug_implementations, missing_docs, rust_2018_idioms, unreachable_pub)]
// https://github.com/rust-lang/rust-clippy/issues/10486
#![allow(clippy::double_must_use)]

mod read_guard;
pub mod shared;
mod state;
pub mod subscriber;
pub mod unique;

#[doc(inline)]
pub use read_guard::ObservableReadGuard;
#[doc(inline)]
pub use subscriber::Subscriber;
