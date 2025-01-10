use std::{
    cmp::Ordering,
    ops::Not,
    pin::Pin,
    task::{self, ready, Poll},
};

use eyeball_im::{Vector, VectorDiff};
use futures_core::Stream;
use pin_project_lite::pin_project;
use smallvec::SmallVec;

use super::{
    ops::BUF_CAP, VectorDiffContainer, VectorDiffContainerOps, VectorDiffContainerStreamElement,
    VectorDiffContainerStreamVecBuf,
};

type UnsortedIndex = usize;

pin_project! {
    /// A [`VectorDiff`] stream adapter that presents a sorted view of the
    /// underlying [`ObservableVector`] items.
    ///
    /// ```rust
    /// use eyeball_im::{ObservableVector, VectorDiff};
    /// use eyeball_im_util::vector::VectorObserverExt;
    /// use imbl::vector;
    /// use stream_assert::{assert_closed, assert_next_eq, assert_pending};
    ///
    /// // Our vector.
    /// let mut ob = ObservableVector::<char>::new();
    /// let (values, mut sub) = ob.subscribe().sort();
    ///
    /// assert!(values.is_empty());
    /// assert_pending!(sub);
    ///
    /// // Append multiple unsorted values.
    /// ob.append(vector!['d', 'b', 'e']);
    /// // We get a `VectorDiff::Append` with sorted values!
    /// assert_next_eq!(sub, VectorDiff::Append { values: vector!['b', 'd', 'e'] });
    ///
    /// // Let's recap what we have. `ob` is our `ObservableVector`,
    /// // `sub` is the “sorted view” / “sorted stream” of `ob`:
    /// // | `ob`  | d b e |
    /// // | `sub` | b d e |
    ///
    /// // Append multiple other values.
    /// ob.append(vector!['f', 'g', 'a', 'c']);
    /// // We get three `VectorDiff`s!
    /// assert_next_eq!(sub, VectorDiff::PushFront { value: 'a' });
    /// assert_next_eq!(sub, VectorDiff::Insert { index: 2, value: 'c' });
    /// assert_next_eq!(sub, VectorDiff::Append { values: vector!['f', 'g'] });
    ///
    /// // Let's recap what we have:
    /// // | `ob`  | d b e f g a c |
    /// // | `sub` | a b c d e f g |
    /// //           ^   ^     ^^^
    /// //           |   |     |
    /// //           |   |     with `VectorDiff::Append { .. }`
    /// //           |   with `VectorDiff::Insert { index: 2, .. }`
    /// //           with `VectorDiff::PushFront { .. }`
    ///
    /// // Technically, `Sort` emits `VectorDiff`s that mimic a sorted `Vector`.
    ///
    /// drop(ob);
    /// assert_closed!(sub);
    /// ```
    ///
    /// [`ObservableVector`]: eyeball_im::ObservableVector
    pub struct Sort<S>
    where
        S: Stream,
        S::Item: VectorDiffContainer,
    {
        #[pin]
        inner: SortImpl<S>,
    }
}

impl<S> Sort<S>
where
    S: Stream,
    S::Item: VectorDiffContainer,
    VectorDiffContainerStreamElement<S>: Ord,
{
    /// Create a new `Sort` with the given (unsorted) initial values and stream
    /// of `VectorDiff` updates for those values.
    pub fn new(
        initial_values: Vector<VectorDiffContainerStreamElement<S>>,
        inner_stream: S,
    ) -> (Vector<VectorDiffContainerStreamElement<S>>, Self) {
        let (initial_sorted, inner) = SortImpl::new(initial_values, inner_stream, Ord::cmp);
        (initial_sorted, Self { inner })
    }
}

impl<S> Stream for Sort<S>
where
    S: Stream,
    S::Item: VectorDiffContainer,
    VectorDiffContainerStreamElement<S>: Ord,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().inner.poll_next(cx, Ord::cmp)
    }
}

pin_project! {
    /// A [`VectorDiff`] stream adapter that presents a sorted view of the
    /// underlying [`ObservableVector`] items.
    ///
    /// Sorting is done using a custom comparison function. Otherwise this
    /// adapter works exactly like [`Sort`], see that type's documentation for
    /// details on how this adapter operates.
    ///
    /// [`ObservableVector`]: eyeball_im::ObservableVector
    pub struct SortBy<S, F>
    where
        S: Stream,
        S::Item: VectorDiffContainer,
    {
        #[pin]
        inner: SortImpl<S>,

        // The comparison function to sort items.
        compare: F,
    }
}

impl<S, F> SortBy<S, F>
where
    S: Stream,
    S::Item: VectorDiffContainer,
    F: Fn(&VectorDiffContainerStreamElement<S>, &VectorDiffContainerStreamElement<S>) -> Ordering,
{
    /// Create a new `SortBy` with the given (unsorted) initial values, stream
    /// of `VectorDiff` updates for those values, and the comparison function.
    pub fn new(
        initial_values: Vector<VectorDiffContainerStreamElement<S>>,
        inner_stream: S,
        compare: F,
    ) -> (Vector<VectorDiffContainerStreamElement<S>>, Self) {
        let (initial_sorted, inner) = SortImpl::new(initial_values, inner_stream, &compare);
        (initial_sorted, Self { inner, compare })
    }
}

impl<S, F> Stream for SortBy<S, F>
where
    S: Stream,
    S::Item: VectorDiffContainer,
    F: Fn(&VectorDiffContainerStreamElement<S>, &VectorDiffContainerStreamElement<S>) -> Ordering,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        this.inner.poll_next(cx, &*this.compare)
    }
}

pin_project! {
    /// A [`VectorDiff`] stream adapter that presents a sorted view of the
    /// underlying [`ObservableVector`] items.
    ///
    /// Sorting is done by transforming items to a key with a custom function
    /// and comparing those. Otherwise this adapter works exactly like [`Sort`],
    /// see that type's documentation for details on how this adapter operates.
    ///
    /// [`ObservableVector`]: eyeball_im::ObservableVector
    pub struct SortByKey<S, F>
    where
        S: Stream,
        S::Item: VectorDiffContainer,
    {
        #[pin]
        inner: SortImpl<S>,

        // The function to convert an item to a key used for comparison.
        key_fn: F,
    }
}

impl<S, F, K> SortByKey<S, F>
where
    S: Stream,
    S::Item: VectorDiffContainer,
    F: Fn(&VectorDiffContainerStreamElement<S>) -> K,
    K: Ord,
{
    /// Create a new `SortByKey` with the given (unsorted) initial values,
    /// stream of `VectorDiff` updates for those values, and the key function.
    pub fn new(
        initial_values: Vector<VectorDiffContainerStreamElement<S>>,
        inner_stream: S,
        key_fn: F,
    ) -> (Vector<VectorDiffContainerStreamElement<S>>, Self) {
        let (initial_sorted, inner) =
            SortImpl::new(initial_values, inner_stream, |a, b| key_fn(a).cmp(&key_fn(b)));
        (initial_sorted, Self { inner, key_fn })
    }
}

impl<S, F, K> Stream for SortByKey<S, F>
where
    S: Stream,
    S::Item: VectorDiffContainer,
    F: Fn(&VectorDiffContainerStreamElement<S>) -> K,
    K: Ord,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let key_fn = &*this.key_fn;
        this.inner.poll_next(cx, |a, b| key_fn(a).cmp(&key_fn(b)))
    }
}

pin_project! {
    pub struct SortImpl<S>
    where
        S: Stream,
        S::Item: VectorDiffContainer,
    {
        // The main stream to poll items from.
        #[pin]
        inner_stream: S,

        // This is the **sorted** buffered vector.
        buffered_vector: Vector<(UnsortedIndex, VectorDiffContainerStreamElement<S>)>,

        // This adapter can produce many items per item of the underlying stream.
        //
        // Thus, if the item type is just `VectorDiff<_>` (non-bached, can't
        // just add diffs to a `poll_next` result), we need a buffer to store the
        // possible extra items in.
        ready_values: VectorDiffContainerStreamVecBuf<S>,
    }
}

impl<S> SortImpl<S>
where
    S: Stream,
    S::Item: VectorDiffContainer,
{
    fn new<F>(
        initial_values: Vector<VectorDiffContainerStreamElement<S>>,
        inner_stream: S,
        compare: F,
    ) -> (Vector<VectorDiffContainerStreamElement<S>>, Self)
    where
        F: Fn(
            &VectorDiffContainerStreamElement<S>,
            &VectorDiffContainerStreamElement<S>,
        ) -> Ordering,
    {
        let mut initial_values = initial_values.into_iter().enumerate().collect::<Vector<_>>();
        initial_values.sort_by(|(_, left), (_, right)| compare(left, right));

        (
            initial_values.iter().map(|(_, value)| value.clone()).collect(),
            Self {
                inner_stream,
                buffered_vector: initial_values,
                ready_values: Default::default(),
            },
        )
    }

    fn poll_next<F>(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        compare: F,
    ) -> Poll<Option<S::Item>>
    where
        F: Fn(
                &VectorDiffContainerStreamElement<S>,
                &VectorDiffContainerStreamElement<S>,
            ) -> Ordering
            + Copy,
    {
        let mut this = self.project();

        loop {
            // First off, if any values are ready, return them.
            if let Some(value) = S::Item::pop_from_vec_buf(this.ready_values) {
                return Poll::Ready(Some(value));
            }

            // Poll `VectorDiff`s from the `inner_stream`.
            let Some(diffs) = ready!(this.inner_stream.as_mut().poll_next(cx)) else {
                return Poll::Ready(None);
            };

            // Consume and apply the diffs if possible.
            let ready = diffs.push_into_vec_buf(this.ready_values, |diff| {
                handle_diff_and_update_buffered_vector(diff, compare, this.buffered_vector)
            });

            if let Some(diff) = ready {
                return Poll::Ready(Some(diff));
            }

            // Else loop and poll the streams again.
        }
    }
}

/// Map a `VectorDiff` to potentially `VectorDiff`s. Keep in mind that
/// `buffered_vector` contains the sorted values.
///
/// When looking for the _position_ of a value (e.g. where to insert a new
/// value?), `Vector::binary_search_by` is used — it is possible because the
/// `Vector` is sorted. When looking for the _unsorted index_ of a value,
/// `Iterator::position` is used.
fn handle_diff_and_update_buffered_vector<T, F>(
    diff: VectorDiff<T>,
    compare: F,
    buffered_vector: &mut Vector<(usize, T)>,
) -> SmallVec<[VectorDiff<T>; BUF_CAP]>
where
    T: Clone,
    F: Fn(&T, &T) -> Ordering,
{
    let mut result = SmallVec::new();

    match diff {
        VectorDiff::Append { values: new_values } => {
            // Sort `new_values`.
            let mut new_values = {
                // Calculate the `new_values` with their `unsorted_index`.
                // The `unsorted_index` is the index of the new value in `new_values` + an
                // offset, where the offset is given by `offset`, i.e the actual size of the
                // `buffered_vector`.
                let offset = buffered_vector.len();
                let mut new_values = new_values
                    .into_iter()
                    .enumerate()
                    .map(|(unsorted_index, value)| (unsorted_index + offset, value))
                    .collect::<Vector<_>>();

                // Now, we can sort `new_values`.
                new_values.sort_by(|(_, left), (_, right)| compare(left, right));

                new_values
            };

            // If `buffered_vector` is empty, all `new_values` are appended.
            if buffered_vector.is_empty() {
                buffered_vector.append(new_values.clone());
                result.push(VectorDiff::Append {
                    values: new_values.into_iter().map(|(_, value)| value).collect(),
                });
            } else {
                // Read the first item of `new_values`. We get a reference to it.
                //
                // Why using `Vector::get`? We _could_ use `new_values.pop_front()` to get
                // ownership of `new_value`. But in the slow path, in the `_` branch, we
                // would need to generate a `VectorDiff::PushBack`, followed by the
                // `VectorDiff::Append` outside this loop, which is 2 diffs. Or, alternatively,
                // we would need to `push_front` the `new_value` again, which has a cost too.
                // By using a reference, and `pop_front`ing when necessary, we reduce the number
                // of diffs.
                while let Some((_, new_value)) = new_values.get(0) {
                    // Fast path.
                    //
                    // If `new_value`, i.e. the first item from `new_values`, is greater than or
                    // equal to the last item from `buffered_vector`, it means
                    // that all items in `new_values` can be appended. That's because `new_values`
                    // is already sorted.
                    if compare(
                        new_value,
                        buffered_vector
                            .last()
                            .map(|(_, value)| value)
                            .expect("`buffered_vector` cannot be empty"),
                    )
                    .is_ge()
                    {
                        // `new_value` isn't consumed. Let's break the loop and emit a
                        // `VectorDiff::Append` just hereinafter.
                        break;
                    }
                    // Slow path.
                    //
                    // Look for the position where to insert the `new_value`.
                    else {
                        // Find the position where to insert `new_value`.
                        match buffered_vector
                            .binary_search_by(|(_, value)| compare(value, new_value))
                        {
                            // Somewhere?
                            Ok(index) | Err(index) if index != buffered_vector.len() => {
                                // Insert the new value. We get it by using `pop_front` on
                                // `new_values`. This time the new value is consumed.
                                let (unsorted_index, new_value) =
                                    new_values.pop_front().expect("`new_values` cannot be empty");

                                buffered_vector.insert(index, (unsorted_index, new_value.clone()));
                                result.push(
                                    // At the beginning? Let's emit a `VectorDiff::PushFront`.
                                    if index == 0 {
                                        VectorDiff::PushFront { value: new_value }
                                    }
                                    // Somewhere in the middle? Let's emit a `VectorDiff::Insert`.
                                    else {
                                        VectorDiff::Insert { index, value: new_value }
                                    },
                                );
                            }
                            // At the end?
                            _ => {
                                // `new_value` isn't consumed. Let's break the loop and emit a
                                // `VectorDiff::Append` just after.
                                break;
                            }
                        }
                    }
                }

                // Some values have not been inserted. Based on our algorithm, it means they
                // must be appended.
                if new_values.is_empty().not() {
                    buffered_vector.append(new_values.clone());
                    result.push(VectorDiff::Append {
                        values: new_values.into_iter().map(|(_, value)| value).collect(),
                    });
                }
            }
        }
        VectorDiff::Clear => {
            // Nothing to do but clear.
            buffered_vector.clear();
            result.push(VectorDiff::Clear);
        }
        VectorDiff::PushFront { value: new_value } => {
            // The unsorted index is inevitably 0, because we push a new item at the front
            // of the vector.
            let unsorted_index = 0;

            // Shift all unsorted indices to the right.
            buffered_vector.iter_mut().for_each(|(unsorted_index, _)| *unsorted_index += 1);

            // Find where to insert the `new_value`.
            match buffered_vector.binary_search_by(|(_, value)| compare(value, &new_value)) {
                // At the beginning? Let's emit a `VectorDiff::PushFront`.
                Ok(0) | Err(0) => {
                    buffered_vector.push_front((unsorted_index, new_value.clone()));
                    result.push(VectorDiff::PushFront { value: new_value });
                }
                // Somewhere in the middle? Let's emit a `VectorDiff::Insert`.
                Ok(index) | Err(index) if index != buffered_vector.len() => {
                    buffered_vector.insert(index, (unsorted_index, new_value.clone()));
                    result.push(VectorDiff::Insert { index, value: new_value });
                }
                // At the end? Let's emit a `VectorDiff::PushBack`.
                _ => {
                    buffered_vector.push_back((unsorted_index, new_value.clone()));
                    result.push(VectorDiff::PushBack { value: new_value });
                }
            }
        }
        VectorDiff::PushBack { value: new_value } => {
            let buffered_vector_length = buffered_vector.len();

            // The unsorted index is inevitably the size of `buffered_vector`, because
            // we push a new item at the back of the vector.
            let unsorted_index = buffered_vector_length;

            // Find where to insert the `new_value`.
            match buffered_vector.binary_search_by(|(_, value)| compare(value, &new_value)) {
                // At the beginning? Let's emit a `VectorDiff::PushFront`.
                Ok(0) | Err(0) => {
                    buffered_vector.push_front((unsorted_index, new_value.clone()));
                    result.push(VectorDiff::PushFront { value: new_value });
                }
                // Somewhere in the middle? Let's emit a `VectorDiff::Insert`.
                Ok(index) | Err(index) if index != buffered_vector_length => {
                    buffered_vector.insert(index, (unsorted_index, new_value.clone()));
                    result.push(VectorDiff::Insert { index, value: new_value });
                }
                // At the end? Let's emit a `VectorDiff::PushBack`.
                _ => {
                    buffered_vector.push_back((unsorted_index, new_value.clone()));
                    result.push(VectorDiff::PushBack { value: new_value });
                }
            }
        }
        VectorDiff::Insert { index: new_unsorted_index, value: new_value } => {
            // Shift all unsorted indices after `new_unsorted_index` to the right.
            buffered_vector.iter_mut().for_each(|(unsorted_index, _)| {
                if *unsorted_index >= new_unsorted_index {
                    *unsorted_index += 1;
                }
            });

            // Find where to insert the `new_value`.
            match buffered_vector.binary_search_by(|(_, value)| compare(value, &new_value)) {
                // At the beginning? Let's emit a `VectorDiff::PushFront`.
                Ok(0) | Err(0) => {
                    buffered_vector.push_front((new_unsorted_index, new_value.clone()));
                    result.push(VectorDiff::PushFront { value: new_value });
                }
                // Somewhere in the middle? Let's emit a `VectorDiff::Insert`.
                Ok(index) | Err(index) if index != buffered_vector.len() => {
                    buffered_vector.insert(index, (new_unsorted_index, new_value.clone()));
                    result.push(VectorDiff::Insert { index, value: new_value });
                }
                // At the end? Let's emit a `VectorDiff::PushBack`.
                _ => {
                    buffered_vector.push_back((new_unsorted_index, new_value.clone()));
                    result.push(VectorDiff::PushBack { value: new_value });
                }
            }
        }
        VectorDiff::PopFront => {
            let last_index = buffered_vector.len() - 1;

            // Find the position and shift all unsorted indices to the left safely.
            // Also, find the value to remove.
            let position = buffered_vector
                .iter_mut()
                .enumerate()
                .fold(None, |mut position, (index, (unsorted_index, _))| {
                    // Position has been found.
                    if position.is_none() && *unsorted_index == 0 {
                        position = Some(index);
                    }
                    // Otherwise, let's shift all other unsorted indices to the left.
                    // Value with an `unsorted_index` of 0 will be removed hereinafter.
                    else {
                        *unsorted_index -= 1;
                    }

                    position
                })
                .expect("`buffered_vector` must have an item with an unsorted index of 0");

            match position {
                // At the beginning? Let's emit a `VectorDiff::PopFront`.
                0 => {
                    buffered_vector.pop_front();
                    result.push(VectorDiff::PopFront);
                }
                // At the end? Let's emit a `VectorDiff::PopBack`.
                index if index == last_index => {
                    buffered_vector.pop_back();
                    result.push(VectorDiff::PopBack);
                }
                // Somewhere in the middle? Let's emit a `VectorDiff::Remove`.
                index => {
                    buffered_vector.remove(index);
                    result.push(VectorDiff::Remove { index });
                }
            }
        }
        VectorDiff::PopBack => {
            let last_index = buffered_vector.len() - 1;

            // Find the value to remove.
            match buffered_vector
                .iter()
                .position(|(unsorted_index, _)| *unsorted_index == last_index)
                .expect(
                    "`buffered_vector` must have an item with an unsorted index of `last_index`",
                ) {
                // At the beginning? Let's emit a `VectorDiff::PopFront`.
                0 => {
                    buffered_vector.pop_front();
                    result.push(VectorDiff::PopFront);
                }
                // At the end? Let's emit a `VectorDiff::PopBack`.
                index if index == last_index => {
                    buffered_vector.pop_back();
                    result.push(VectorDiff::PopBack);
                }
                // Somewhere in the middle? Let's emit a `VectorDiff::Remove`.
                index => {
                    buffered_vector.remove(index);
                    result.push(VectorDiff::Remove { index });
                }
            }
        }
        VectorDiff::Remove { index: new_unsorted_index } => {
            let last_index = buffered_vector.len() - 1;

            // Shift all items with an `unsorted_index` greater than `new_unsorted_index` to
            // the left.
            // Also, find the value to remove.
            let position = buffered_vector
                .iter_mut()
                .enumerate()
                .fold(None, |mut position, (index, (unsorted_index, _))| {
                    if position.is_none() && *unsorted_index == new_unsorted_index {
                        position = Some(index);
                    }

                    if *unsorted_index > new_unsorted_index {
                        *unsorted_index -= 1;
                    }

                    position
                })
                .expect("`buffered_vector` must contain an item with an unsorted index of `new_unsorted_index`");

            match position {
                // At the beginning? Let's emit a `VectorDiff::PopFront`.
                0 => {
                    buffered_vector.pop_front();
                    result.push(VectorDiff::PopFront);
                }
                // At the end? Let's emit a `VectorDiff::PopBack`.
                index if index == last_index => {
                    buffered_vector.pop_back();
                    result.push(VectorDiff::PopBack);
                }
                // Somewhere in the middle? Let's emit a `VectorDiff::Remove`.
                index => {
                    buffered_vector.remove(index);
                    result.push(VectorDiff::Remove { index });
                }
            }
        }
        VectorDiff::Set { index: new_unsorted_index, value: new_value } => {
            // We need to _update_ the value to `new_value`, and to _move_ it (since it is a
            // new value, we need to sort it).
            //
            // Find the `old_index` and the `new_index`, respectively representing the
            // _from_ and _to_ positions of the value to move.
            let old_index = buffered_vector
                .iter()
                .position(|(unsorted_index, _)| *unsorted_index == new_unsorted_index)
                .expect("`buffered_vector` must contain an item with an unsorted index of `new_unsorted_index`");

            let new_index =
                match buffered_vector.binary_search_by(|(_, value)| compare(value, &new_value)) {
                    Ok(index) => index,
                    Err(index) => index,
                };

            match old_index.cmp(&new_index) {
                // `old_index` is before `new_index`.
                // Remove value at `old_index`, and insert the new value at `new_index - 1`: we need
                // to subtract 1 because `old_index` has been removed before `new_insert`, which
                // has shifted the indices.
                //
                // SAFETY: `new_index - 1` won't underflow because `new_index` is necessarily
                // greater than `old_index` here. `old_index` cannot be lower than 0, so
                // `new_index` cannot be lower than 1, hence `new_index - 1` cannot be lower
                // than 0.
                Ordering::Less => {
                    let new_index = new_index - 1;
                    let new_unsorted_index_with_value = (new_unsorted_index, new_value.clone());

                    // If `old_index == new_index`, we are clearly updating the same index.
                    // Then, let's emit a `VectorDiff::Set`.
                    if old_index == new_index {
                        buffered_vector.set(old_index, new_unsorted_index_with_value);

                        result.push(VectorDiff::Set { index: old_index, value: new_value });
                    } else {
                        buffered_vector.remove(old_index);
                        buffered_vector.insert(new_index, new_unsorted_index_with_value);

                        result.push(VectorDiff::Remove { index: old_index });
                        result.push(VectorDiff::Insert { index: new_index, value: new_value });
                    }
                }
                // `old_index` is the same as `new_index`.
                Ordering::Equal => {
                    buffered_vector.set(new_index, (new_unsorted_index, new_value.clone()));
                    result.push(VectorDiff::Set { index: new_index, value: new_value });
                }
                // `old_index` is after `new_index`.
                // Remove value at `old_index`, and insert the new value at `new_index`. No shifting
                // here.
                Ordering::Greater => {
                    buffered_vector.remove(old_index);
                    buffered_vector.insert(new_index, (new_unsorted_index, new_value.clone()));

                    result.push(VectorDiff::Remove { index: old_index });
                    result.push(VectorDiff::Insert { index: new_index, value: new_value });
                }
            }
        }
        VectorDiff::Truncate { length: new_length } => {
            // Keep values where their `unsorted_index` is lower than the `new_length`.
            buffered_vector.retain(|(unsorted_index, _)| *unsorted_index < new_length);
            result.push(VectorDiff::Truncate { length: new_length });
        }
        VectorDiff::Reset { values: new_values } => {
            // Calculate the `new_values` with their `unsorted_index`.
            let mut new_values = new_values.into_iter().enumerate().collect::<Vector<_>>();

            // Now, we can sort `new_values`.
            new_values.sort_by(|(_, left), (_, right)| compare(left, right));

            // Finally, update `buffered_vector` and create the `VectorDiff::Reset`.
            *buffered_vector = new_values.clone();
            result.push(VectorDiff::Reset {
                values: new_values.into_iter().map(|(_, value)| value).collect(),
            });
        }
    }

    result
}
