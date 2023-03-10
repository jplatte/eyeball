//! Observable collections based on the `im` crate.
//!
//! Cargo features:
//!
//! - `tracing`: Emit [tracing] events when updates are sent out
#![warn(missing_debug_implementations, missing_docs, rust_2018_idioms)]
#![allow(clippy::new_without_default)]

mod vector;

pub use vector::{ObservableVector, VectorDiff, VectorSubscriber};
