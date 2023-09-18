use arrayvec::ArrayVec;
use eyeball_im::VectorDiff;

pub trait VectorDiffContainerOps<T>: Sized {
    type Family: VectorDiffContainerFamily;
    type Buffer: Default;

    fn from_item(vector_diff: VectorDiff<T>) -> Self;

    fn filter_map<U>(
        self,
        f: impl FnMut(VectorDiff<T>) -> Option<VectorDiff<U>>,
    ) -> Option<VectorDiffContainerFamilyMember<Self::Family, U>>;

    fn push_into_buffer(
        self,
        buffer: &mut Self::Buffer,
        make_diffs: impl FnMut(VectorDiff<T>) -> ArrayVec<VectorDiff<T>, 2>,
    ) -> Option<Self>;

    fn pop_from_buffer(buffer: &mut Self::Buffer) -> Option<Self>;
}

#[allow(unreachable_pub)]
pub type VectorDiffContainerFamilyMember<F, U> = <F as VectorDiffContainerFamily>::Member<U>;

impl<T> VectorDiffContainerOps<T> for VectorDiff<T> {
    type Family = VectorDiffFamily;
    type Buffer = Option<VectorDiff<T>>;

    fn from_item(vector_diff: VectorDiff<T>) -> Self {
        vector_diff
    }

    fn filter_map<U>(
        self,
        mut f: impl FnMut(VectorDiff<T>) -> Option<VectorDiff<U>>,
    ) -> Option<VectorDiffContainerFamilyMember<Self::Family, U>> {
        f(self)
    }

    fn push_into_buffer(
        self,
        buffer: &mut Self::Buffer,
        mut make_diffs: impl FnMut(VectorDiff<T>) -> ArrayVec<VectorDiff<T>, 2>,
    ) -> Option<Self> {
        assert!(buffer.is_none(), "buffer must be None when calling push_into_buffer");

        let mut diffs = make_diffs(self);

        let last = diffs.pop();
        if let Some(first) = diffs.pop() {
            *buffer = last;
            Some(first)
        } else {
            last
        }
    }

    fn pop_from_buffer(buffer: &mut Self::Buffer) -> Option<Self> {
        buffer.take()
    }
}

impl<T> VectorDiffContainerOps<T> for Vec<VectorDiff<T>> {
    type Family = VecVectorDiffFamily;
    type Buffer = ();

    fn from_item(vector_diff: VectorDiff<T>) -> Self {
        vec![vector_diff]
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

    fn push_into_buffer(
        self,
        _buffer: &mut (),
        make_diffs: impl FnMut(VectorDiff<T>) -> ArrayVec<VectorDiff<T>, 2>,
    ) -> Option<Self> {
        let res: Vec<_> = self.into_iter().flat_map(make_diffs).collect();
        if res.is_empty() {
            None
        } else {
            Some(res)
        }
    }

    fn pop_from_buffer(_: &mut Self::Buffer) -> Option<Self> {
        None
    }
}

#[allow(unreachable_pub)]
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
