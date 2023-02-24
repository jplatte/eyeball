//! Add observability to your Rust types!
//!
//! Cargo features:
//!
//! - `tracing`: Emit [tracing] events when updates are sent out
#![warn(missing_debug_implementations, missing_docs)]
#![allow(clippy::new_without_default)]

mod observable;
mod shared_observable;
mod subscriber;

pub use observable::Observable;
pub use shared_observable::{ObservableLock, SharedObservable, SharedObservableBase};
pub use subscriber::{Subscriber, SubscriberReadGuard};
