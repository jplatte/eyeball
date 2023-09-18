use std::collections::VecDeque;

use eyeball_im::VectorDiff;

pub trait VectorDiffContainerOps<T> {
    type Family: VectorDiffContainerFamily;

    fn from_item(vector_diff: VectorDiff<T>) -> Self;

    fn pick_item(vector_diffs: &mut VecDeque<Self>) -> Option<Self>
    where
        Self: Sized;

    fn for_each(self, f: impl FnMut(VectorDiff<T>));

    fn filter_map<U>(
        self,
        f: impl FnMut(VectorDiff<T>) -> Option<VectorDiff<U>>,
    ) -> Option<VectorDiffContainerFamilyMember<Self::Family, U>>;
}

#[allow(unreachable_pub)]
pub type VectorDiffContainerFamilyMember<F, U> = <F as VectorDiffContainerFamily>::Member<U>;

impl<T> VectorDiffContainerOps<T> for VectorDiff<T> {
    type Family = VectorDiffFamily;

    fn from_item(vector_diff: VectorDiff<T>) -> Self {
        vector_diff
    }

    fn pick_item(vector_diffs: &mut VecDeque<Self>) -> Option<Self>
    where
        Self: Sized,
    {
        vector_diffs.pop_front()
    }

    fn for_each(self, mut f: impl FnMut(VectorDiff<T>)) {
        f(self);
    }

    fn filter_map<U>(
        self,
        mut f: impl FnMut(VectorDiff<T>) -> Option<VectorDiff<U>>,
    ) -> Option<VectorDiffContainerFamilyMember<Self::Family, U>> {
        f(self)
    }
}

impl<T> VectorDiffContainerOps<T> for Vec<VectorDiff<T>> {
    type Family = VecVectorDiffFamily;

    fn from_item(vector_diff: VectorDiff<T>) -> Self {
        vec![vector_diff]
    }

    fn pick_item(vector_diffs: &mut VecDeque<Self>) -> Option<Self>
    where
        Self: Sized,
    {
        Some(vector_diffs.drain(..).flatten().collect())
    }

    fn for_each(self, f: impl FnMut(VectorDiff<T>)) {
        self.into_iter().for_each(f);
    }

    fn filter_map<U>(
        self,
        f: impl FnMut(VectorDiff<T>) -> Option<VectorDiff<U>>,
    ) -> Option<VectorDiffContainerFamilyMember<Self::Family, U>> {
        let res: Vec<_> = self.into_iter().filter_map(f).collect();
        if res.is_empty() {
            None
        } else {
            Some(res)
        }
    }
}

pub trait VectorDiffContainerFamily {
    type Member<T>: VectorDiffContainerOps<T, Family = Self>;
}

#[derive(Debug)]
pub enum VectorDiffFamily {}

impl VectorDiffContainerFamily for VectorDiffFamily {
    type Member<T> = VectorDiff<T>;
}

#[derive(Debug)]
pub enum VecVectorDiffFamily {}

impl VectorDiffContainerFamily for VecVectorDiffFamily {
    type Member<T> = Vec<VectorDiff<T>>;
}
