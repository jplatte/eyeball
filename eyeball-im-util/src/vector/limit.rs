use std::{
    cmp::{min, Ordering},
    collections::VecDeque,
    pin::Pin,
    task::{self, ready, Poll},
};

use super::{
    VectorDiffContainer, VectorDiffContainerFamily, VectorDiffContainerOps,
    VectorDiffContainerStreamElement, VectorDiffContainerStreamFamily,
};
use eyeball_im::VectorDiff;
use futures_core::Stream;
use imbl::{vector, Vector};
use pin_project_lite::pin_project;

pin_project! {
    /// A [`VectorDiff`] stream adapter that presents a limited view of the
    /// underlying [`ObservableVector`]s items.
    ///
    /// For example, let `S` be a `Stream<Item = VectorDiff>`. The `Vector` represented
    /// by `S` can have any length, but one may want to virtually _limit_ this `Vector`
    /// to a certain size. Then this `DynamicLimit` adapter is well appropriate.
    /// The limit is dynamic, i.e. it changes over time based on values that are polled
    /// from another `Stream` (ref. [`Self::limit_stream`]).
    ///
    /// Because the limit is dynamic, an internal buffered vector is kept, so that
    /// the adapter knows which values can be added when the limit is increased, or
    /// when values are removed and new values must be inserted. This fact is important
    /// if the items of the `Vector` have a non-negligible size.
    ///
    /// It's OK to have a limit larger than the length of the observed `Vector`.
    #[project = DynamicLimitProj]
    pub struct DynamicLimit<S, L>
    where
        S: Stream,
        S::Item: VectorDiffContainer,
    {
        // The main stream to poll items from.
        #[pin]
        inner_stream: S,

        // The limit stream to poll new limits from.
        #[pin]
        limit_stream: L,

        // The buffered vector that is updated with the main stream's items. It's
        // used to provide missing items, e.g. when the limit increases.
        buffered_vector: Vector<VectorDiffContainerStreamElement<S>>,

        // The current limit.
        limit: usize,

        // This adapter is not a basic filter: It can produce items. For example, if the
        // vector is [10, 11, 12, 13], with a limit of 2; then if an item is popped
        // at the front, 10 is removed, but 12 is pushed back as it “enters” the “view”. That's
        // 2 items to produce. This field contains all items that must be polled before
        // anything.
        ready_values: VecDeque<S::Item>,
    }
}

impl<S, L> DynamicLimit<S, L>
where
    S: Stream,
    S::Item: VectorDiffContainer,
    VectorDiffContainerStreamElement<S>: Clone + Send + Sync + 'static,
    VectorDiffContainerStreamFamily<S>:
        VectorDiffContainerFamily<Member<VectorDiffContainerStreamElement<S>> = S::Item>,
    L: Stream<Item = usize>,
{
    /// Create a new [`DynamicLimit`] with the given (unlimited) initial values,
    /// stream of `VectorDiff` updates for those values, and a stream of
    /// limits.
    ///
    /// Note that this adapter won't produce anything until a new limit is
    /// polled.
    pub fn new(
        initial_values: Vector<VectorDiffContainerStreamElement<S>>,
        inner_stream: S,
        limit_stream: L,
    ) -> Self {
        Self {
            inner_stream,
            limit_stream,
            buffered_vector: initial_values,
            limit: 0,
            ready_values: VecDeque::new(),
        }
    }
}

impl<S, L> Stream for DynamicLimit<S, L>
where
    S: Stream,
    S::Item: VectorDiffContainer,
    VectorDiffContainerStreamElement<S>: Clone + Send + Sync + 'static,
    L: Stream<Item = usize>,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().poll_next(cx)
    }
}

impl<S, L> DynamicLimitProj<'_, S, L>
where
    S: Stream,
    S::Item: VectorDiffContainer,
    VectorDiffContainerStreamElement<S>: Clone + Send + Sync + 'static,
    L: Stream<Item = usize>,
{
    fn poll_next(&mut self, cx: &mut task::Context<'_>) -> Poll<Option<S::Item>> {
        // First off, if any value is ready, let's return it.
        if let Some(ready_value) = self.ready_values.pop_front() {
            return Poll::Ready(Some(ready_value));
        }

        // Let's poll a new limit from `limit_stream` before polling `inner_stream`.
        if let Poll::Ready(Some(next_limit)) = self.limit_stream.as_mut().poll_next(cx) {
            // We have new `VectorDiff`s after the limit has been updated. Let's
            // return them.
            if let Some(diffs) = self.update_limit(next_limit) {
                return Poll::Ready(Some(diffs));
            }
        }

        // Now, let's poll `VectorDiff`s from the `inner_stream`.
        let Some(diffs) = ready!(self.inner_stream.as_mut().poll_next(cx)) else {
            return Poll::Ready(None);
        };

        // Now, let's consume and apply the diffs if possible.
        diffs.for_each(|diff| self.apply_diff(diff));

        // If any value is ready, let's return it.
        match self.ready_values.pop_front() {
            Some(diff) => Poll::Ready(Some(diff)),
            None => Poll::Pending,
        }
    }

    fn apply_diff(&mut self, diff: VectorDiff<VectorDiffContainerStreamElement<S>>) {
        let limit = *self.limit;
        let length = self.buffered_vector.len();

        // Let's update the `buffered_vector`. It's a replica of the original observed
        // `Vector`. We need to maintain it in order to be able to produce valid
        // `VectorDiff`s when items are missing.
        self.update_buffered_vector(&diff);

        // If the limit is zero, we have nothing to do.
        if limit == 0 {
            return;
        }

        let is_full = length >= limit;
        let has_values_after_limit = length > limit;

        match diff {
            VectorDiff::Append { mut values } => {
                if is_full {
                    // Let's ignore the diff.
                } else {
                    // Let's truncate the `values` to fit inside the free space.
                    values.truncate(min(limit - length, values.len()));
                    self.push_ready_value(VectorDiff::Append { values });
                }
            }
            VectorDiff::Clear => {
                self.push_ready_value(VectorDiff::Clear);
            }
            VectorDiff::PushFront { value } => {
                if is_full {
                    // Create 1 free space.
                    self.push_ready_value(VectorDiff::PopBack);
                }

                // There is space for this new item.
                self.push_ready_value(VectorDiff::PushFront { value });
            }
            VectorDiff::PushBack { value } => {
                if is_full {
                    // Let's ignore the diff.
                } else {
                    // There is space for this new item.
                    self.push_ready_value(VectorDiff::PushBack { value });
                }
            }
            VectorDiff::PopFront => {
                self.push_ready_value(VectorDiff::PopFront);

                if has_values_after_limit {
                    // Push back a new item.
                    self.push_ready_value(VectorDiff::PushBack {
                        // SAFETY: It's safe to `unwrap` here as we are sure a value exists at index
                        // `limit - 1`. We are also sure that `limit > 1`.
                        value: self.buffered_vector.get(limit - 1).unwrap().clone(),
                    });
                }
            }
            VectorDiff::PopBack => {
                if has_values_after_limit {
                    // Pop back outside the limit, let's ignore the diff.
                } else {
                    self.push_ready_value(VectorDiff::PopBack);
                }
            }
            VectorDiff::Insert { index, value } => {
                if index >= limit {
                    // Insert after `limit`, let's ignore the diff.
                } else {
                    if is_full {
                        // Create 1 free space.
                        self.push_ready_value(VectorDiff::PopBack);
                    }

                    // There is space for this new item.
                    self.push_ready_value(VectorDiff::Insert { index, value });
                }
            }
            VectorDiff::Set { index, value } => {
                if index >= limit {
                    // Update after `limit`, let's ignore the diff.
                } else {
                    self.push_ready_value(VectorDiff::Set { index, value });
                }
            }
            VectorDiff::Remove { index } => {
                if index >= limit {
                    // Remove after `limit`, let's ignore the diff.
                } else {
                    self.push_ready_value(VectorDiff::Remove { index });

                    if has_values_after_limit {
                        self.push_ready_value(VectorDiff::PushBack {
                            // SAFETY: It's safe to `unwrap` here as we are sure a value exists at
                            // index `limit - 1`. We are also sure that
                            // `limit > 1`.
                            value: self.buffered_vector.get(limit - 1).unwrap().clone(),
                        });
                    }
                }
            }
            VectorDiff::Truncate { length: new_length } => {
                if new_length >= limit {
                    // Truncate items after `limit`, let's ignore the diff.
                } else {
                    self.push_ready_value(VectorDiff::Truncate { length: new_length });
                }
            }
            VectorDiff::Reset { values: mut new_values } => {
                if new_values.len() > limit {
                    // There are too many values, truncate.
                    new_values.truncate(limit);
                }

                // There is space for these new items.
                self.push_ready_value(VectorDiff::Reset { values: new_values });
            }
        }
    }

    fn push_ready_value(&mut self, diff: VectorDiff<VectorDiffContainerStreamElement<S>>) {
        self.ready_values.push_back(S::Item::from_item(diff));
    }

    /// Update the buffered vector.
    ///
    /// All items are cloned.
    fn update_buffered_vector(&mut self, diff: &VectorDiff<VectorDiffContainerStreamElement<S>>) {
        match diff {
            VectorDiff::Append { values } => self.buffered_vector.append(values.clone()),
            VectorDiff::Clear => self.buffered_vector.clear(),
            VectorDiff::PushFront { value } => self.buffered_vector.push_front(value.clone()),
            VectorDiff::PushBack { value } => self.buffered_vector.push_back(value.clone()),
            VectorDiff::PopFront => {
                self.buffered_vector.pop_front();
            }
            VectorDiff::PopBack => {
                self.buffered_vector.pop_back();
            }
            VectorDiff::Insert { index, value } => {
                self.buffered_vector.insert(*index, value.clone());
            }
            VectorDiff::Set { index, value } => {
                self.buffered_vector.set(*index, value.clone());
            }
            VectorDiff::Remove { index } => {
                self.buffered_vector.remove(*index);
            }
            VectorDiff::Truncate { length } => self.buffered_vector.truncate(*length),
            VectorDiff::Reset { values } => {
                *self.buffered_vector = values.clone();
            }
        }
    }

    /// Update the limit if necessary.
    ///
    /// * If the buffered vector is empty, it returns `None`.
    /// * If the limit increases, a `VectorDiff::Append` is produced if any
    ///   items exist.
    /// * If the limit decreases below the length of the vector, a
    ///   `VectorDiff::Truncate` is produced.
    ///
    /// It's OK to have a `new_limit` larger than the length of the `Vector`.
    /// The `new_limit` won't be capped.
    fn update_limit(&mut self, next_limit: usize) -> Option<S::Item> {
        if self.buffered_vector.is_empty() {
            // Let's update the limit.
            *self.limit = next_limit;

            // But let's ignore any update.
            return None;
        }

        match (*self.limit).cmp(&next_limit) {
            // old < new
            Ordering::Less => {
                // Let's add the missing items.
                let mut missing_items = vector![];

                for nth in *self.limit..min(self.buffered_vector.len(), next_limit) {
                    // SAFETY: It's OK to `unwrap` here as we are sure we are iterating over defined
                    // items.
                    let item = self.buffered_vector.get(nth).unwrap();
                    missing_items.push_back(item.clone());
                }

                // Let's update the limit.
                *self.limit = next_limit;

                let missing_items =
                    S::Item::from_item(VectorDiff::Append { values: missing_items });

                Some(missing_items)
            }

            // old > new
            Ordering::Greater => {
                // Let's remove the extra items.
                let items_removal = S::Item::from_item(VectorDiff::Truncate { length: next_limit });

                // Let's update the limit.
                *self.limit = next_limit;

                Some(items_removal)
            }

            // old == new
            Ordering::Equal => {
                // Nothing to do.
                None
            }
        }
    }
}
