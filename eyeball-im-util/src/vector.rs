//! Utilities around [`ObservableVector`][eyeball_im::ObservableVector].

mod filter;
mod limit;
mod ops;
mod sort;
mod traits;

use eyeball_im::VectorDiff;
use futures_core::Stream;

use self::ops::{VectorDiffContainerFamilyMember, VectorDiffContainerOps};
pub use self::{
    filter::{Filter, FilterMap},
    limit::{EmptyLimitStream, Limit},
    sort::{Sort, SortBy, SortByKey},
    traits::{
        BatchedVectorSubscriber, VectorDiffContainer, VectorObserver, VectorObserverExt,
        VectorSubscriberExt,
    },
};

/// Type alias for extracting the element type from a stream of
/// [`VectorDiffContainer`]s.
pub type VectorDiffContainerStreamElement<S> =
    <<S as Stream>::Item as VectorDiffContainer>::Element;

/// Type alias for extracting the stream item type after the element type was
/// mapped to the given type `U`, from a stream of [`VectorDiffContainer`]s.
pub type VectorDiffContainerStreamMappedItem<S, U> =
    VectorDiffContainerFamilyMember<VectorDiffContainerStreamFamily<S>, U>;

/// Type alias for extracting the [`VectorDiffContainerFamily`] type from a
/// stream of [`VectorDiffContainer`]s.
///
/// [`VectorDiffContainerFamily`]: ops::VectorDiffContainerFamily
type VectorDiffContainerStreamFamily<S> =
    <<S as Stream>::Item as VectorDiffContainerOps<VectorDiffContainerStreamElement<S>>>::Family;

/// Type alias for a `VectorDiff` of `VectorDiffContainerStreamElement`s.
type VectorDiffContainerDiff<S> = VectorDiff<VectorDiffContainerStreamElement<S>>;

/// Type alias for extracting the buffer type from a stream of
/// [`VectorDiffContainer`]s' `ArrayBuf`.
type VectorDiffContainerStreamArrayBuf<S> =
    <<S as Stream>::Item as VectorDiffContainerOps<VectorDiffContainerStreamElement<S>>>::ArrayBuf;

/// Type alias for extracting the buffer type from a stream of
/// [`VectorDiffContainer`]s' `VecBuf`.
type VectorDiffContainerStreamVecBuf<S> =
    <<S as Stream>::Item as VectorDiffContainerOps<VectorDiffContainerStreamElement<S>>>::VecBuf;
