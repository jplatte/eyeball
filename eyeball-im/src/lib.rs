//! Observable collections based on the `im` crate.
//!
//! Cargo features:
//!
//! - `tracing`: Emit [tracing] events when updates are sent out

mod vector;
mod vector2;

pub use vector::{
    ObservableVector, ObservableVectorEntries, ObservableVectorEntry, VectorDiff, VectorSubscriber,
};
pub use vector2::{
    ObservableVector2, ObservableVector2Entries, ObservableVector2Entry,
    ObservableVector2WriteGuard, VectorSubscriber2,
};

#[doc(no_inline)]
pub use imbl::Vector;
