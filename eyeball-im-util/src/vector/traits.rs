//! Public traits.

use eyeball_im::VectorDiff;

use super::ops::VectorDiffContainerOps;

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
