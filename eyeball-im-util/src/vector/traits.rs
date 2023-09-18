//! Public traits.

use eyeball_im::VectorDiff;

use super::ops::{
    VecVectorDiffFamily, VectorDiffContainerFamily, VectorDiffContainerOps, VectorDiffFamily,
};

/// Abstraction over stream items that the adapters in this module can deal
/// with.
pub trait VectorDiffContainer:
    VectorDiffContainerOps<Self::Element, Family = <Self as VectorDiffContainer>::Family>
{
    /// The element type of the [`Vector`][imbl::Vector] that diffs are being
    /// handled for.
    type Element;

    #[doc(hidden)]
    type Family: VectorDiffContainerFamily<Member<Self::Element> = Self>;
}

impl<T> VectorDiffContainer for VectorDiff<T> {
    type Element = T;
    type Family = VectorDiffFamily;
}

impl<T> VectorDiffContainer for Vec<VectorDiff<T>> {
    type Element = T;
    type Family = VecVectorDiffFamily;
}
