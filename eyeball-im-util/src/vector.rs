//! Utilities around [`ObservableVector`][eyeball_im::ObservableVector].

mod filter;
mod limit;
mod ops;

use eyeball_im::VectorDiff;
use futures_core::Stream;

use self::ops::{
    VectorDiffContainerFamily, VectorDiffContainerFamilyMember, VectorDiffContainerOps,
};
pub use self::{
    filter::{Filter, FilterMap},
    limit::{EmptyLimitStream, Limit},
};

/// Abstraction over stream items that the adapters in this module can deal
/// with.
pub trait VectorDiffContainer: VectorDiffContainerOps<Self::Element> {
    /// The element type of the [`Vector`][imbl::Vector] that diffs are being
    /// handled for.
    type Element;
}

impl<T> VectorDiffContainer for VectorDiff<T> {
    type Element = T;
}

impl<T> VectorDiffContainer for Vec<VectorDiff<T>> {
    type Element = T;
}

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
type VectorDiffContainerStreamFamily<S> =
    <<S as Stream>::Item as VectorDiffContainerOps<VectorDiffContainerStreamElement<S>>>::Family;

/// Type alias for a `VectorDiff` of `VectorDiffContainerStreamElement`s.
type VectorDiffContainerDiff<S> = VectorDiff<VectorDiffContainerStreamElement<S>>;
