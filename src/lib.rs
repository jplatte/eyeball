#![doc = include_str!("../README.md")]
#![warn(missing_debug_implementations, missing_docs)]
#![allow(clippy::new_without_default)]

mod notifier;
mod observable;
mod vector;

pub use observable::{Observable, ObservableSubscriber};
pub use vector::{Vector, VectorDiff, VectorSubscriber};
