//! Utilities around [`ObservableVector`][eyeball_im::ObservableVector].

mod filter;
mod head;
mod ops;
mod sort;
mod tail;
mod traits;

use eyeball_im::VectorDiff;
use futures_core::Stream;

use self::ops::{VectorDiffContainerFamilyMember, VectorDiffContainerOps};
pub use self::{
    filter::{Filter, FilterMap},
    head::{EmptyLimitStream, Head},
    sort::{Sort, SortBy, SortByKey},
    tail::Tail,
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
/// [`VectorDiffContainer`]s' `HeadBuf`.
type VectorDiffContainerStreamHeadBuf<S> =
    <<S as Stream>::Item as VectorDiffContainerOps<VectorDiffContainerStreamElement<S>>>::HeadBuf;

/// Type alias for extracting the buffer type from a stream of
/// [`VectorDiffContainer`]s' `TailBuf`.
type VectorDiffContainerStreamTailBuf<S> =
    <<S as Stream>::Item as VectorDiffContainerOps<VectorDiffContainerStreamElement<S>>>::TailBuf;

/// Type alias for extracting the buffer type from a stream of
/// [`VectorDiffContainer`]s' `SortBuf`.
type VectorDiffContainerStreamSortBuf<S> =
    <<S as Stream>::Item as VectorDiffContainerOps<VectorDiffContainerStreamElement<S>>>::SortBuf;
