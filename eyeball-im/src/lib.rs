//! Observable collections based on the `im` crate.
//!
//! Cargo features:
//!
//! - `tracing`: Emit [tracing] events when updates are sent out

mod vector;

pub use vector::{
    ObservableVector, ObservableVectorEntries, ObservableVectorEntry, ObservableVectorTransaction,
    ObservableVectorTransactionEntries, ObservableVectorTransactionEntry, VectorDiff,
    VectorSubscriber, VectorSubscriberBatchedStream, VectorSubscriberStream,
};

#[doc(no_inline)]
pub use imbl::Vector;
