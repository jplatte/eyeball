use arrayvec::ArrayVec;
use eyeball_im::VectorDiff;
use smallvec::SmallVec;

/// Default capacity for the `VectorDiffContainerOps::ArrayBuf` buffer or the
/// `VectorDiffContainerOps::VecBuf` buffer.
///
/// Capacity is hardcoded to 2 because that's all we need for now.
pub(super) const BUF_CAP: usize = 2;

pub trait VectorDiffContainerOps<T>: Sized {
    type Family: VectorDiffContainerFamily;
    type ArrayBuf: Default;
    type VecBuf: Default;

    fn from_item(vector_diff: VectorDiff<T>) -> Self;

    fn filter_map<U>(
        self,
        f: impl FnMut(VectorDiff<T>) -> Option<VectorDiff<U>>,
    ) -> Option<VectorDiffContainerFamilyMember<Self::Family, U>>;

    fn push_into_array_buf(
        self,
        buffer: &mut Self::ArrayBuf,
        make_diffs: impl FnMut(VectorDiff<T>) -> ArrayVec<VectorDiff<T>, BUF_CAP>,
    ) -> Option<Self>;

    fn pop_from_array_buf(buffer: &mut Self::ArrayBuf) -> Option<Self>;

    fn push_into_vec_buf(
        self,
        buffer: &mut Self::VecBuf,
        make_diffs: impl FnMut(VectorDiff<T>) -> SmallVec<[VectorDiff<T>; BUF_CAP]>,
    ) -> Option<Self>;

    fn pop_from_vec_buf(buffer: &mut Self::VecBuf) -> Option<Self>;
}

#[allow(unreachable_pub)]
pub type VectorDiffContainerFamilyMember<F, U> = <F as VectorDiffContainerFamily>::Member<U>;

impl<T> VectorDiffContainerOps<T> for VectorDiff<T> {
    type Family = VectorDiffFamily;
    type ArrayBuf = Option<VectorDiff<T>>;
    type VecBuf = SmallVec<[VectorDiff<T>; BUF_CAP]>;

    fn from_item(vector_diff: VectorDiff<T>) -> Self {
        vector_diff
    }

    fn filter_map<U>(
        self,
        mut f: impl FnMut(VectorDiff<T>) -> Option<VectorDiff<U>>,
    ) -> Option<VectorDiffContainerFamilyMember<Self::Family, U>> {
        f(self)
    }

    fn push_into_array_buf(
        self,
        buffer: &mut Self::ArrayBuf,
        mut make_diffs: impl FnMut(VectorDiff<T>) -> ArrayVec<VectorDiff<T>, BUF_CAP>,
    ) -> Option<Self> {
        assert!(buffer.is_none(), "buffer must be None when calling push_into_array_buf");

        let mut diffs = make_diffs(self);

        let last = diffs.pop();
        if let Some(first) = diffs.pop() {
            *buffer = last;
            Some(first)
        } else {
            last
        }
    }

    fn pop_from_array_buf(buffer: &mut Self::ArrayBuf) -> Option<Self> {
        buffer.take()
    }

    fn push_into_vec_buf(
        self,
        buffer: &mut Self::VecBuf,
        mut make_diffs: impl FnMut(VectorDiff<T>) -> SmallVec<[VectorDiff<T>; BUF_CAP]>,
    ) -> Option<Self> {
        assert!(buffer.is_empty(), "buffer must be empty when calling `push_into_vec_buf`");

        let mut diffs = make_diffs(self);

        match diffs.len() {
            0 => None,
            1 => diffs.pop(),
            _ => {
                // We want the first element. We can't “pop front” on a `SmallVec`.
                // The idea is to reverse the `diffs` and to pop from it.
                diffs.reverse();
                *buffer = diffs;

                buffer.pop()
            }
        }
    }

    fn pop_from_vec_buf(buffer: &mut Self::VecBuf) -> Option<Self> {
        buffer.pop()
    }
}

impl<T> VectorDiffContainerOps<T> for Vec<VectorDiff<T>> {
    type Family = VecVectorDiffFamily;
    type ArrayBuf = ();
    type VecBuf = ();

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

    fn push_into_array_buf(
        self,
        _buffer: &mut Self::ArrayBuf,
        make_diffs: impl FnMut(VectorDiff<T>) -> ArrayVec<VectorDiff<T>, BUF_CAP>,
    ) -> Option<Self> {
        let res: Vec<_> = self.into_iter().flat_map(make_diffs).collect();

        if res.is_empty() {
            None
        } else {
            Some(res)
        }
    }

    fn pop_from_array_buf(_: &mut Self::ArrayBuf) -> Option<Self> {
        None
    }

    fn push_into_vec_buf(
        self,
        _buffer: &mut (),
        make_diffs: impl FnMut(VectorDiff<T>) -> SmallVec<[VectorDiff<T>; BUF_CAP]>,
    ) -> Option<Self> {
        let res: Vec<_> = self.into_iter().flat_map(make_diffs).collect();

        if res.is_empty() {
            None
        } else {
            Some(res)
        }
    }

    fn pop_from_vec_buf(_: &mut Self::ArrayBuf) -> Option<Self> {
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
