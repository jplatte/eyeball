use smallvec::SmallVec;
use std::{
    cmp::{min, Ordering},
    iter::repeat,
    pin::Pin,
    task::{self, ready, Poll},
};

use super::{
    VectorDiffContainer, VectorDiffContainerOps, VectorDiffContainerStreamElement,
    VectorDiffContainerStreamSkipBuf, VectorObserver,
};
use eyeball_im::VectorDiff;
use futures_core::Stream;
use imbl::Vector;
use pin_project_lite::pin_project;

pin_project! {
    /// A [`VectorDiff`] stream adapter that presents a limited view of the
    /// underlying [`ObservableVector`]s items. The view starts after `count`
    /// number of values are skipped, until the end. It must not be confused
    /// with [`Tail`](super::Tail) where `Tail` keeps the last values, while
    /// `Skip` skips the first values.
    ///
    /// For example, let `S` be a `Stream<Item = VectorDiff>`. The [`Vector`]
    /// represented by `S` can have any length, but one may want to virtually
    /// skip the first `count` values. Then this `Skip` adapter is appropriate.
    ///
    /// An internal buffered vector is kept so that the adapter knows which
    /// values can be added when the index is decreased, or when values are
    /// removed and new values must be inserted. This fact is important if the
    /// items of the `Vector` have a non-negligible size.
    ///
    /// It's okay to have an index larger than the length of the observed
    /// `Vector`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use eyeball_im::{ObservableVector, VectorDiff};
    /// use eyeball_im_util::vector::VectorObserverExt;
    /// use imbl::vector;
    /// use stream_assert::{assert_closed, assert_next_eq, assert_pending};
    ///
    /// // Our vector.
    /// let mut ob = ObservableVector::<char>::new();
    /// let (values, mut sub) = ob.subscribe().skip(3);
    ///
    /// assert!(values.is_empty());
    /// assert_pending!(sub);
    ///
    /// // Append multiple values.
    /// ob.append(vector!['a', 'b', 'c', 'd', 'e']);
    /// // We get a `VectorDiff::Append` with the latest 2 values because the
    /// // first 3 values have been skipped!
    /// assert_next_eq!(sub, VectorDiff::Append { values: vector!['d', 'e'] });
    ///
    /// // Let's recap what we have. `ob` is our `ObservableVector`,
    /// // `sub` is the “limited view” of `ob`:
    /// // | `ob`  | a b c d e |
    /// // | `sub` | _ _ _ d e |
    /// // “_” means the item has been skipped.
    ///
    /// // Append multiple values.
    /// ob.append(vector!['f', 'g']);
    /// // We get a single `VectorDiff`.
    /// assert_next_eq!(sub, VectorDiff::Append { values: vector!['f', 'g'] });
    ///
    /// // Let's recap what we have:
    /// // | `ob`  | a b c d e f g |
    /// // | `sub` | _ _ _ d e f g |
    /// //                     ^^^
    /// //                     |
    /// //                     `VectorDiff::Append { .. }`
    ///
    /// // Insert a single value.
    /// ob.insert(1, 'h');
    /// // We get a single `VectorDiff::PushFront`. Indeed, `h` is inserted at
    /// // index 1, so every value after that is shifted to the right, thus `c`
    /// // “enters the view” via a `PushFront`.
    /// assert_next_eq!(sub, VectorDiff::PushFront { value: 'c' });
    ///
    /// // Let's recap what we have:
    /// // | `ob`  | a h b c d e f g |
    /// // | `sub` | _ _ _ c d e f g |
    /// //                 ^
    /// //                 |
    /// //                 `VectorDiff::PushFront { .. }`
    ///
    /// assert_pending!(sub);
    /// drop(ob);
    /// assert_closed!(sub);
    /// ```
    ///
    /// [`ObservableVector`]: eyeball_im::ObservableVector
    #[project = SkipProj]
    pub struct Skip<S, C>
    where
        S: Stream,
        S::Item: VectorDiffContainer,
    {
        // The main stream to poll items from.
        #[pin]
        inner_stream: S,

        // The count stream to poll new count values from.
        #[pin]
        count_stream: C,

        // The buffered vector that is updated with the main stream's items.
        // It's used to provide missing items, e.g. when the count decreases or
        // when values must be filled.
        buffered_vector: Vector<VectorDiffContainerStreamElement<S>>,

        // The current count.
        //
        // This is an option because it can be uninitialized. It's incorrect
        // to use a default value for `count`, contrary to [`Head`] or [`Tail`]
        // where `limit` can be 0.
        count: Option<usize>,

        // This adapter is not a basic filter: It can produce many items per
        // item of the underlying stream.
        //
        // Thus, if the item type is just `VectorDiff<_>` (non-bached, can't
        // just add diffs to a poll_next result), we need a buffer to store the
        // possible extra item in.
        ready_values: VectorDiffContainerStreamSkipBuf<S>,
    }
}

impl<S> Skip<S, EmptyCountStream>
where
    S: Stream,
    S::Item: VectorDiffContainer,
{
    /// Create a new [`Skip`] with the given (unlimited) initial values,
    /// stream of `VectorDiff` updates for those values, and a fixed count.
    ///
    /// Returns the initial values as well as a stream of updates that ensure
    /// that the resulting vector never includes the first `count` items.
    pub fn new(
        initial_values: Vector<VectorDiffContainerStreamElement<S>>,
        inner_stream: S,
        count: usize,
    ) -> (Vector<VectorDiffContainerStreamElement<S>>, Self) {
        Self::dynamic_with_initial_count(initial_values, inner_stream, count, EmptyCountStream)
    }
}

impl<S, C> Skip<S, C>
where
    S: Stream,
    S::Item: VectorDiffContainer,
    C: Stream<Item = usize>,
{
    /// Create a new [`Skip`] with the given (unlimited) initial values,
    /// stream of `VectorDiff` updates for those values, and a stream of
    /// indices.
    ///
    /// This is equivalent to `dynamic_with_initial_count` where the
    /// `initial_count` is `usize::MAX`, except that it doesn't return the
    /// limited vector as it would be empty anyways.
    ///
    /// Note that the returned `Skip` won't produce anything until the first
    /// count is produced by the index stream.
    pub fn dynamic(
        initial_values: Vector<VectorDiffContainerStreamElement<S>>,
        inner_stream: S,
        count_stream: C,
    ) -> Self {
        Self {
            inner_stream,
            count_stream,
            buffered_vector: initial_values,
            count: None,
            ready_values: Default::default(),
        }
    }

    /// Create a new [`Skip`] with the given (unlimited) initial values,
    /// stream of `VectorDiff` updates for those values, and an initial
    /// count as well as a stream of new count values.
    pub fn dynamic_with_initial_count(
        initial_values: Vector<VectorDiffContainerStreamElement<S>>,
        inner_stream: S,
        initial_count: usize,
        count_stream: C,
    ) -> (Vector<VectorDiffContainerStreamElement<S>>, Self) {
        let buffered_vector = initial_values.clone();

        let initial_values = initial_values.skeep(initial_count);

        let stream = Self {
            inner_stream,
            count_stream,
            buffered_vector,
            count: Some(initial_count),
            ready_values: Default::default(),
        };

        (initial_values, stream)
    }
}

impl<S, C> Stream for Skip<S, C>
where
    S: Stream,
    S::Item: VectorDiffContainer,
    C: Stream<Item = usize>,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().poll_next(cx)
    }
}

impl<S, C> VectorObserver<VectorDiffContainerStreamElement<S>> for Skip<S, C>
where
    S: Stream,
    S::Item: VectorDiffContainer,
    C: Stream<Item = usize>,
{
    type Stream = Self;

    fn into_parts(self) -> (Vector<VectorDiffContainerStreamElement<S>>, Self::Stream) {
        (self.buffered_vector.clone(), self)
    }
}

impl<S, C> SkipProj<'_, S, C>
where
    S: Stream,
    S::Item: VectorDiffContainer,
    C: Stream<Item = usize>,
{
    fn poll_next(&mut self, cx: &mut task::Context<'_>) -> Poll<Option<S::Item>> {
        loop {
            // First off, if any values are ready, return them.
            if let Some(value) = S::Item::pop_from_skip_buf(self.ready_values) {
                return Poll::Ready(Some(value));
            }

            // Poll a new count value from `count_stream` before polling `inner_stream`.
            while let Poll::Ready(Some(next_count)) = self.count_stream.as_mut().poll_next(cx) {
                // Update the count value and emit `VectorDiff`s accordingly.
                if let Some(diffs) = self.update_count(next_count) {
                    return Poll::Ready(S::Item::extend_skip_buf(diffs, self.ready_values));
                }

                // If `update_count` returned `None`, poll the count stream
                // again.
            }

            // Poll `VectorDiff`s from the `inner_stream`.
            let Some(diffs) = ready!(self.inner_stream.as_mut().poll_next(cx)) else {
                return Poll::Ready(None);
            };

            // Consume and apply the diffs if possible.
            let ready = diffs.push_into_skip_buf(self.ready_values, |diff| {
                let count = *self.count;
                let previous_length = self.buffered_vector.len();

                // Update the `buffered_vector`. It's a replica of the original observed
                // `Vector`. We need to maintain it in order to be able to produce valid
                // `VectorDiff`s when items are missing.
                diff.clone().apply(self.buffered_vector);

                // Handle the `diff` if and only if there is a count.
                if let Some(count) = count {
                    handle_diff(diff, count, previous_length, self.buffered_vector)
                } else {
                    SmallVec::new()
                }
            });

            if let Some(diff) = ready {
                return Poll::Ready(Some(diff));
            }

            // Else loop and poll the streams again.
        }
    }

    /// Update the count value if necessary.
    ///
    /// * If the buffered vector is empty, it returns `None`.
    /// * If the count decreases, `VectorDiff::PushFront`s are produced if any
    ///   items exist.
    /// * If the count increases, `VectorDiff::PopFront`s are produced.
    ///
    /// It's OK to have a `new_count` larger than the length of the `Vector`.
    /// The `new_count` won't be capped.
    fn update_count(
        &mut self,
        new_count: usize,
    ) -> Option<Vec<VectorDiff<VectorDiffContainerStreamElement<S>>>> {
        // Let's update the count.
        let old_count = self.count.replace(new_count);

        if self.buffered_vector.is_empty() {
            // If empty, nothing to do.
            return None;
        }

        let old_count = match old_count {
            // First time `count` is initialized.
            None => {
                return Some(vec![VectorDiff::Append {
                    values: self.buffered_vector.clone().skeep(new_count),
                }])
            }

            // Other updates of `count`.
            Some(old_count) => old_count,
        };

        // Clamp `old_count` and `new_count` to the length of `buffered_vector` in case
        // it is larger.
        let buffered_vector_length = self.buffered_vector.len();
        let old_count = min(old_count, buffered_vector_length);
        let new_count = min(new_count, buffered_vector_length);

        match old_count.cmp(&new_count) {
            // old < new, count is shifting to the right
            Ordering::Less => {
                // Skip more items than the buffer contains: we must clear all existing items.
                if buffered_vector_length <= new_count {
                    Some(vec![VectorDiff::Clear])
                } else {
                    // Let's remove the extra items.
                    Some(repeat(VectorDiff::PopFront).take(new_count - old_count).collect())
                }
            }

            // old > new, count is shifting to the left
            Ordering::Greater => {
                // Optimisation: when `old_count` is at the end of `buffered_vector`, and
                // `new_count` is zero, we can emit a single `VectorDiff::Append` instead of
                // many `VectorDiff::PushFront`.
                if old_count == buffered_vector_length && new_count == 0 {
                    Some(vec![VectorDiff::Append { values: self.buffered_vector.clone() }])
                } else {
                    let mut missing_items = self
                        .buffered_vector
                        .iter()
                        .rev()
                        .skip(buffered_vector_length - old_count)
                        .take(old_count - new_count)
                        .cloned()
                        .peekable();

                    if missing_items.peek().is_none() {
                        None
                    } else {
                        // Let's add the missing items.
                        Some(
                            missing_items
                                .map(|missing_item| VectorDiff::PushFront { value: missing_item })
                                .collect(),
                        )
                    }
                }
            }

            // old == new
            Ordering::Equal => {
                // Nothing to do.
                None
            }
        }
    }
}

fn handle_diff<T: Clone>(
    diff: VectorDiff<T>,
    count: usize,
    previous_length: usize,
    buffered_vector: &Vector<T>,
) -> SmallVec<[VectorDiff<T>; 2]> {
    let mut res = SmallVec::new();

    match diff {
        VectorDiff::Append { values } => {
            // Some values are appended after `count`.
            if buffered_vector.len() > count {
                // Cut `values` if they aren't all appended after `count`.
                //
                // Note: Do a `<` instead of `<=` to avoid calling `skip` with 0.

                let values = if previous_length < count {
                    values.skeep(count - previous_length)
                } else {
                    values
                };

                res.push(VectorDiff::Append { values });
            }
        }

        VectorDiff::Clear => {
            res.push(VectorDiff::Clear);
        }

        VectorDiff::PushFront { value } => {
            // The push shifts values after `count`.
            if previous_length >= count {
                // The value at `count` is the one that must be pushed front.
                //
                // If `count` is zero, let's avoid a look up + clone by forwarding `value`.
                // Otherwise, let's look up at `count`.
                if count == 0 {
                    res.push(VectorDiff::PushFront { value });
                } else if let Some(value) = buffered_vector.get(count) {
                    res.push(VectorDiff::PushFront { value: value.clone() });
                }
            }
        }

        VectorDiff::PushBack { value } => {
            // The push happens after `count`.
            if previous_length >= count {
                res.push(VectorDiff::PushBack { value });
            }
        }

        VectorDiff::PopFront => {
            // The pop shifts values after `count`.
            if previous_length > count {
                res.push(VectorDiff::PopFront);
            }
        }

        VectorDiff::PopBack => {
            // The pop happens after `count`.
            if previous_length > count {
                res.push(VectorDiff::PopBack);
            }
        }

        VectorDiff::Insert { index, value } => {
            // The insert shifts values after `count`.
            if previous_length >= count {
                // The insert happens before `count`, we need to insert a new
                // value with `PushFront`.
                if count > 0 && index < count {
                    // The value at `count` is the one that must be
                    // pushed front.
                    if let Some(value) = buffered_vector.get(count) {
                        res.push(VectorDiff::PushFront { value: value.clone() });
                    }
                }
                // The insert happens after `count`, we need to re-map `index`.
                else {
                    res.push(VectorDiff::Insert { index: index - count, value });
                }
            }
        }

        VectorDiff::Set { index, value } => {
            // The update happens after `count`.
            if index >= count {
                res.push(VectorDiff::Set { index: index - count, value });
            }
        }

        VectorDiff::Remove { index } => {
            // The removal happens inside the view.
            if previous_length > count {
                // The removal happens before `count`, we need to pop a value at
                // the front.
                if index < count {
                    res.push(VectorDiff::PopFront);
                }
                // The removal happens after `count`, we need to re-map `index`.
                else {
                    res.push(VectorDiff::Remove { index: index - count });
                }
            }
        }

        VectorDiff::Truncate { length: new_length } => {
            // The truncation removes some values after `count`.
            if previous_length > count {
                // All values removed by the truncation are after `count`.
                if new_length > count {
                    res.push(VectorDiff::Truncate { length: new_length - count });
                }
                // Some values removed by the truncation are before `count` or exactly at `count`.
                // It means that all values must be cleared.
                else {
                    res.push(VectorDiff::Clear);
                }
            }
        }

        VectorDiff::Reset { values } => {
            res.push(VectorDiff::Reset { values: values.skeep(count) });
        }
    }

    res
}

/// An empty stream with an item type of `usize`.
#[derive(Debug)]
#[non_exhaustive]
pub struct EmptyCountStream;

impl Stream for EmptyCountStream {
    type Item = usize;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(None)
    }
}

trait Skeep {
    fn skeep(self, count: usize) -> Self;
}

impl<T> Skeep for Vector<T>
where
    T: Clone,
{
    fn skeep(self, count: usize) -> Self {
        match count {
            // Skip 0 values, let's return all of them.
            0 => self,

            // Skip more values than `self` contains, let's return an empty `Vector`.
            count if count >= self.len() => Vector::new(),

            // Skip the first n values.
            count => self.skip(count),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Skeep;
    use imbl::vector;

    #[test]
    fn test_skeep() {
        // Count is 0.
        assert_eq!(vector![1, 2, 3, 4].skeep(0), vector![1, 2, 3, 4]);

        // Count is smaller than the values.
        assert_eq!(vector![1, 2, 3, 4].skeep(2), vector![3, 4]);

        // Count is equal to the number of values.
        assert_eq!(vector![1, 2, 3, 4].skeep(4), vector![]);

        // Count is larger than the number of values.
        assert_eq!(vector![1, 2, 3, 4].skeep(6), vector![]);
    }
}
