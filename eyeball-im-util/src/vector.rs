//! Utilities around [`ObservableVector`][eyeball_im::ObservableVector].

mod filter;
mod limit;

use eyeball_im::VectorDiff;
use futures_core::Stream;

pub use self::{
    filter::{Filter, FilterMap},
    limit::DynamicLimit,
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

/// Type alias for extracting the [`VectorDiffContainerFamily`] type from a
/// stream of [`VectorDiffContainer`]s.
type VectorDiffContainerStreamFamily<S> =
    <<S as Stream>::Item as VectorDiffContainerOps<VectorDiffContainerStreamElement<S>>>::Family;

/// Type alias for extracting the stream item type after the element type was
/// mapped to the given type `U`, from a stream of [`VectorDiffContainer`]s.
pub type VectorDiffContainerStreamMappedItem<S, U> =
    <VectorDiffContainerStreamFamily<S> as VectorDiffContainerFamily>::Member<U>;

#[doc(hidden)]
pub trait VectorDiffContainerOps<T> {
    type Family: VectorDiffContainerFamily;

    fn from_item(vector_diff: VectorDiff<T>) -> Self;

    fn for_each(self, f: impl FnMut(VectorDiff<T>));

    fn filter_map<U>(
        self,
        f: impl FnMut(VectorDiff<T>) -> Option<VectorDiff<U>>,
    ) -> Option<<Self::Family as VectorDiffContainerFamily>::Member<U>>;
}

impl<T> VectorDiffContainerOps<T> for VectorDiff<T> {
    type Family = VectorDiffFamily;

    fn from_item(vector_diff: VectorDiff<T>) -> Self {
        vector_diff
    }

    fn for_each(self, mut f: impl FnMut(VectorDiff<T>)) {
        f(self);
    }

    fn filter_map<U>(
        self,
        mut f: impl FnMut(VectorDiff<T>) -> Option<VectorDiff<U>>,
    ) -> Option<<Self::Family as VectorDiffContainerFamily>::Member<U>> {
        f(self)
    }
}

impl<T> VectorDiffContainerOps<T> for Vec<VectorDiff<T>> {
    type Family = VecVectorDiffFamily;

    fn from_item(vector_diff: VectorDiff<T>) -> Self {
        vec![vector_diff]
    }

    fn for_each(self, f: impl FnMut(VectorDiff<T>)) {
        self.into_iter().for_each(f);
    }

    fn filter_map<U>(
        self,
        f: impl FnMut(VectorDiff<T>) -> Option<VectorDiff<U>>,
    ) -> Option<<Self::Family as VectorDiffContainerFamily>::Member<U>> {
        let res: Vec<_> = self.into_iter().filter_map(f).collect();
        if res.is_empty() {
            None
        } else {
            Some(res)
        }
    }
}

#[doc(hidden)]
pub trait VectorDiffContainerFamily {
    type Member<T>: VectorDiffContainerOps<T, Family = Self>;
}

#[doc(hidden)]
#[derive(Debug)]
pub enum VectorDiffFamily {}

impl VectorDiffContainerFamily for VectorDiffFamily {
    type Member<T> = VectorDiff<T>;
}

#[doc(hidden)]
#[derive(Debug)]
pub enum VecVectorDiffFamily {}

impl VectorDiffContainerFamily for VecVectorDiffFamily {
    type Member<T> = Vec<VectorDiff<T>>;
}
