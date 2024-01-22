use std::{
    cmp::Ordering,
    ops::{ControlFlow, Not},
    pin::Pin,
    task::{self, ready, Poll},
};

use eyeball_im::{Vector, VectorDiff};
use futures_core::Stream;
use pin_project_lite::pin_project;
use smallvec::SmallVec;

use super::{
    VectorDiffContainer, VectorDiffContainerOps, VectorDiffContainerStreamBuffer,
    VectorDiffContainerStreamElement,
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
    /// use std::cmp::Ordering;
    /// use stream_assert::{assert_closed, assert_next_eq, assert_pending};
    ///
    /// // A comparison function that is used to sort our
    /// // `ObservableVector` values.
    /// fn cmp<T>(left: &T, right: &T) -> Ordering
    /// where
    ///     T: Ord,
    /// {
    ///     left.cmp(right)
    /// }
    ///
    /// # fn main() {
    /// // Our vector.
    /// let mut ob = ObservableVector::<char>::new();
    /// let (values, mut sub) = ob.subscribe().sort_by(&cmp);
    /// //                                             ^^^^
    /// //                                             | our comparison function
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
    /// // `sub` is the “sorted view”/“sorted stream” of `ob`:
    /// // | `ob`  | d b e |
    /// // | `sub` | b d e |
    ///
    /// // Append other multiple values.
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
    /// // Technically, `SortBy` emits `VectorDiff`s that mimic a sorted `Vector`.
    ///
    /// drop(ob);
    /// assert_closed!(sub);
    /// # }
    /// ```
    ///
    /// [`ObservableVector`]: eyeball_im::ObservableVector
    pub struct SortBy<'a, S, F>
    where
        S: Stream,
        S::Item: VectorDiffContainer,
    {
        // The main stream to poll items from.
        #[pin]
        inner_stream: S,

        // The comparison function to sort items.
        compare: &'a F,

        // This is the **sorted** buffered vector.
        buffered_vector: Vector<(UnsortedIndex, VectorDiffContainerStreamElement<S>)>,

        // This adapter can produce many items per item of the underlying stream.
        //
        // Thus, if the item type is just `VectorDiff<_>` (non-bached, can't
        // just add diffs to a `poll_next` result), we need a buffer to store the
        // possible extra items in.
        ready_values: VectorDiffContainerStreamBuffer<S>,
    }
}

impl<'a, S, F> SortBy<'a, S, F>
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
        compare: &'a F,
    ) -> (Vector<VectorDiffContainerStreamElement<S>>, Self) {
        let mut initial_values = initial_values.into_iter().enumerate().collect::<Vector<_>>();
        initial_values.sort_by(|(_, left), (_, right)| compare(left, right));

        (
            initial_values.iter().map(|(_, value)| value.clone()).collect(),
            Self {
                inner_stream,
                compare,
                buffered_vector: initial_values,
                ready_values: Default::default(),
            },
        )
    }

    /// For testing purposes only.
    #[doc(hidden)]
    pub fn buffered_vector(&self) -> &Vector<(UnsortedIndex, VectorDiffContainerStreamElement<S>)> {
        &self.buffered_vector
    }
}

impl<'a, S, F> Stream for SortBy<'a, S, F>
where
    S: Stream,
    S::Item: VectorDiffContainer,
    F: Fn(&VectorDiffContainerStreamElement<S>, &VectorDiffContainerStreamElement<S>) -> Ordering,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        loop {
            // First off, if any values are ready, return them.
            if let Some(value) = S::Item::pop_from_buffer(this.ready_values) {
                return Poll::Ready(Some(value));
            }

            // Poll `VectorDiff`s from the `inner_stream`.
            let Some(diffs) = ready!(this.inner_stream.as_mut().poll_next(cx)) else {
                return Poll::Ready(None);
            };

            // Consume and apply the diffs if possible.
            let ready = diffs.push_into_buffer(this.ready_values, |diff| {
                handle_diff_and_update_buffered_vector(diff, this.compare, this.buffered_vector)
            });

            if let Some(diff) = ready {
                return Poll::Ready(Some(diff));
            }

            // Else loop and poll the streams again.
        }
    }
}

// Map a `VectorDiff` to potentially `VectorDiff`s. Keep in mind that
// `buffered_vector` contains the sorted values.
fn handle_diff_and_update_buffered_vector<'a, T, F>(
    diff: VectorDiff<T>,
    compare: &'a F,
    buffered_vector: &mut Vector<(usize, T)>,
) -> SmallVec<[VectorDiff<T>; 2]>
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
                let mut last_insertion_index = 0;

                // Read the first item of `new_values`. We get a reference to it.
                //
                // Why using `Vector::get`? We _could_ use `new_values.pop_front()` to get
                // ownership of `new_value`. But in the slow path, in the `None` branch, we
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
                        // Skip the first items up to the `last_insertion_index`, and find the
                        // position where to insert `new_value`.
                        match buffered_vector
                            .iter()
                            .skip(last_insertion_index)
                            .position(|(_, value)| compare(value, new_value).is_ge())
                        {
                            Some(index) => {
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

                                // Finally, let's update the `last_insertion_index`.
                                last_insertion_index = index;
                            }
                            None => {
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
            // Also, find where to insert the `new_value`.
            match buffered_vector
                .iter_mut()
                .enumerate()
                .fold(None, |mut position, (index, (unsorted_index, value))| {
                    if position.is_none() && compare(value, &new_value).is_ge() {
                        position = Some(index);
                    }

                    *unsorted_index += 1;

                    position
                })
                // If `buffered_vector` is empty, we want to emit a `VectorDiff::PushFront`. Let's
                // map `position = None` to `position = Some(0)`.
                .or_else(|| buffered_vector.is_empty().then_some(0))
            {
                // At the beginning? Let's emit a `VectorDiff::PushFront`.
                Some(0) => {
                    buffered_vector.push_front((unsorted_index, new_value.clone()));
                    result.push(VectorDiff::PushFront { value: new_value });
                }
                // Somewhere in the middle? Let's emit a `VectorDiff::Insert`.
                Some(index) => {
                    buffered_vector.insert(index, (unsorted_index, new_value.clone()));
                    result.push(VectorDiff::Insert { index, value: new_value });
                }
                // At the end? Let's emit a `VectorDiff::PushBack`.
                None => {
                    buffered_vector.push_back((unsorted_index, new_value.clone()));
                    result.push(VectorDiff::PushBack { value: new_value });
                }
            }
        }
        VectorDiff::PushBack { value: new_value } => {
            // The unsorted index is inevitably the size of `buffered_vector`, because
            // we push a new item at the back of the vector.
            let unsorted_index = buffered_vector.len();

            // Find where to insert the `new_value`.
            match buffered_vector.iter().position(|(_, value)| compare(value, &new_value).is_ge()) {
                // At the beginning? Let's emit a `VectorDiff::PushFront`.
                Some(0) => {
                    buffered_vector.push_front((unsorted_index, new_value.clone()));
                    result.push(VectorDiff::PushFront { value: new_value });
                }
                // Somewhere in the middle? Let's emit a `VectorDiff::Insert`.
                Some(index) => {
                    buffered_vector.insert(index, (unsorted_index, new_value.clone()));
                    result.push(VectorDiff::Insert { index, value: new_value });
                }
                // At the end? Let's emit a `VectorDiff::PushBack`.
                None => {
                    buffered_vector.push_back((unsorted_index, new_value.clone()));
                    result.push(VectorDiff::PushBack { value: new_value });
                }
            }
        }
        VectorDiff::Insert { index: new_unsorted_index, value: new_value } => {
            // Shift all unsorted indices after `new_unsorted_index` to the right.
            // Also, find where to insert the `new_value`.
            match buffered_vector.iter_mut().enumerate().fold(
                None,
                |mut position, (index, (unsorted_index, value))| {
                    if position.is_none() && compare(value, &new_value).is_ge() {
                        position = Some(index);
                    }

                    if *unsorted_index >= new_unsorted_index {
                        *unsorted_index += 1;
                    }

                    position
                },
            ) {
                // At the beginning? Let's emit a `VectorDiff::PushFront`.
                Some(0) => {
                    buffered_vector.push_front((new_unsorted_index, new_value.clone()));
                    result.push(VectorDiff::PushFront { value: new_value });
                }
                // Somewhere in the middle? Let's emit a `VectorDiff::Insert`.
                Some(index) => {
                    buffered_vector.insert(index, (new_unsorted_index, new_value.clone()));
                    result.push(VectorDiff::Insert { index, value: new_value });
                }
                // At the end? Let's emit a `VectorDiff::PushBack`.
                None => {
                    buffered_vector.push_back((new_unsorted_index, new_value.clone()));
                    result.push(VectorDiff::PushBack { value: new_value });
                }
            }
        }
        VectorDiff::PopFront => {
            let last_index = buffered_vector.len() - 1;

            // Find the position and shift all unsorted indices to the left safely.
            // Also, find the value to remove.
            match buffered_vector
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
                .expect("`buffered_vector` must have an item with an unsorted index of 0")
            {
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
            match buffered_vector
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
                .expect("`buffered_vector` must contain an item with an unsorted index of `new_unsorted_index`")
            {
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
            let last_index = buffered_vector.len();
            // We need to _update_ the value to `new_value`, and to _move_ it (since it is a
            // new value, we need to sort it).
            //
            // Find the `old_index` and the `new_index`, respectively representing the
            // _from_ and _to_ positions of the value to move.
            let (old_index, new_index) = match buffered_vector.iter().enumerate().try_fold(
                (None, None),
                |(mut old_index, mut new_index), (index, (unsorted_index, value))| {
                    // `unsorted_index`s are unique. `old_index` can be written only once: no
                    // need to check if `old_index` is `None`.
                    if *unsorted_index == new_unsorted_index {
                        old_index = Some(index);
                    }

                    // Write `new_index` only once.
                    if new_index.is_none() && compare(value, &new_value).is_ge() {
                        new_index = Some(index);
                    }

                    // We found our two positions? Great! Let's break `try_fold`.
                    if old_index.is_some() && new_index.is_some() {
                        ControlFlow::Break((old_index, new_index))
                    } else {
                        ControlFlow::Continue((old_index, new_index))
                    }
                },
            ) {
                ControlFlow::Break((old_index, new_index))
                | ControlFlow::Continue((old_index, new_index)) => (
                    old_index.expect("`buffered_vector` must contain an item with an unsorted index of `new_unsorted_index`"),
                    new_index.unwrap_or(last_index),
                )
            };

            match old_index.cmp(&new_index) {
                // `old_index` is before `new_index`.
                // Remove value at `old_index`, and insert the new value at `new_index - 1`: we need
                // to subtract 1 because `old_index` has been removed before `new_insert`, which
                // has shifted the indices.
                Ordering::Less => {
                    buffered_vector.remove(old_index);
                    buffered_vector.insert(new_index - 1, (new_unsorted_index, new_value.clone()));

                    result.push(VectorDiff::Remove { index: old_index });
                    result.push(VectorDiff::Insert { index: new_index - 1, value: new_value });
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
