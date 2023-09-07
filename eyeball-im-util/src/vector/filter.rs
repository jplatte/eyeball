use std::{
    collections::VecDeque,
    ops::Not,
    pin::Pin,
    task::{self, ready, Poll},
};

use eyeball_im::{Vector, VectorDiff};
use futures_core::Stream;
use pin_project_lite::pin_project;

use super::{
    VectorDiffContainer, VectorDiffContainerFamily, VectorDiffContainerOps,
    VectorDiffContainerStreamElement, VectorDiffContainerStreamFamily,
    VectorDiffContainerStreamMappedItem,
};

pin_project! {
    /// A [`VectorDiff`] stream adapter that presents a filtered view of the
    /// underlying [`ObservableVector`]s items.
    pub struct Filter<S, F> {
        #[pin]
        inner: FilterImpl<S>,
        filter: F,
    }
}

impl<S, F> Filter<S, F>
where
    S: Stream,
    S::Item: VectorDiffContainer,
    VectorDiffContainerStreamElement<S>: Clone + Send + Sync + 'static,
    VectorDiffContainerStreamFamily<S>:
        VectorDiffContainerFamily<Member<VectorDiffContainerStreamElement<S>> = S::Item>,
    F: Fn(&VectorDiffContainerStreamElement<S>) -> bool,
{
    /// Create a new `Filter` with the given (unfiltered) initial values, stream
    /// of `VectorDiff` updates for those values, and filter.
    pub fn new(
        mut values: Vector<VectorDiffContainerStreamElement<S>>,
        inner: S,
        filter: F,
    ) -> (Vector<VectorDiffContainerStreamElement<S>>, Self) {
        let original_len = values.len();
        let mut filtered_indices = VecDeque::new();

        let mut original_idx = 0;
        values.retain(|val| {
            let keep = filter(val);
            if keep {
                filtered_indices.push_back(original_idx);
            }
            original_idx += 1;
            keep
        });

        let inner = FilterImpl { inner, filtered_indices, original_len };
        (values, Self { inner, filter })
    }
}

impl<S, F> Stream for Filter<S, F>
where
    S: Stream,
    S::Item: VectorDiffContainer,
    VectorDiffContainerStreamElement<S>: Clone + Send + Sync + 'static,
    VectorDiffContainerStreamFamily<S>:
        VectorDiffContainerFamily<Member<VectorDiffContainerStreamElement<S>> = S::Item>,
    F: Fn(&VectorDiffContainerStreamElement<S>) -> bool,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        let projected = self.project();
        projected.inner.project().handle_diff_filter(&*projected.filter, cx)
    }
}

pin_project! {
    /// A [`VectorDiff`] stream adapter that presents a filter+mapped view of
    /// the underlying [`ObservableVector`]s items.
    pub struct FilterMap<S, F> {
        #[pin]
        inner: FilterImpl<S>,
        filter: F,
    }
}

impl<S, U, F> FilterMap<S, F>
where
    S: Stream,
    S::Item: VectorDiffContainer,
    VectorDiffContainerStreamElement<S>: Clone + Send + Sync + 'static,
    U: Clone,
    F: Fn(VectorDiffContainerStreamElement<S>) -> Option<U>,
{
    /// Create a new `Filter` with the given (un-filter+mapped) initial values,
    /// stream of `VectorDiff` updates for those values, and filter.
    pub fn new(
        values: Vector<VectorDiffContainerStreamElement<S>>,
        inner: S,
        filter: F,
    ) -> (Vector<U>, Self) {
        let original_len = values.len();
        let (values, filtered_indices) = values
            .iter()
            .enumerate()
            .filter_map(|(original_idx, val)| {
                filter(val.clone()).map(|mapped| (mapped, original_idx))
            })
            .unzip();

        let inner = FilterImpl { inner, filtered_indices, original_len };
        (values, Self { inner, filter })
    }
}

impl<S, U, F> Stream for FilterMap<S, F>
where
    S: Stream,
    S::Item: VectorDiffContainer,
    VectorDiffContainerStreamElement<S>: Clone + Send + Sync + 'static,
    U: Clone,
    F: Fn(VectorDiffContainerStreamElement<S>) -> Option<U>,
{
    type Item = VectorDiffContainerStreamMappedItem<S, U>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        let projected = self.project();
        projected.inner.project().handle_diff_filter_map(&*projected.filter, cx)
    }
}

pin_project! {
    #[project = FilterImplProj]
    pub(super) struct FilterImpl<S> {
        #[pin]
        inner: S,
        filtered_indices: VecDeque<usize>,
        original_len: usize,
    }
}

impl<S> FilterImplProj<'_, S>
where
    S: Stream,
    S::Item: VectorDiffContainer,
    VectorDiffContainerStreamElement<S>: Clone + Send + Sync + 'static,
{
    fn append_filter<F>(
        &mut self,
        mut values: Vector<VectorDiffContainerStreamElement<S>>,
        f: &F,
    ) -> Option<Vector<VectorDiffContainerStreamElement<S>>>
    where
        F: Fn(&VectorDiffContainerStreamElement<S>) -> bool,
    {
        let mut original_idx = *self.original_len;
        *self.original_len += values.len();
        values.retain(|value| {
            let keep = f(value);
            if keep {
                self.filtered_indices.push_back(original_idx);
            }
            original_idx += 1;
            keep
        });

        values.is_empty().not().then_some(values)
    }

    fn append_filter_map<U, F>(
        &mut self,
        values: Vector<VectorDiffContainerStreamElement<S>>,
        f: &F,
    ) -> Option<Vector<U>>
    where
        U: Clone,
        F: Fn(VectorDiffContainerStreamElement<S>) -> Option<U>,
    {
        let mut original_idx = *self.original_len;
        *self.original_len += values.len();
        let mapped_values: Vector<_> = values
            .into_iter()
            .filter_map(|val| {
                let result = f(val).map(|mapped| {
                    self.filtered_indices.push_back(original_idx);
                    mapped
                });
                original_idx += 1;
                result
            })
            .collect();

        mapped_values.is_empty().not().then_some(mapped_values)
    }

    fn handle_append_filter<F>(
        &mut self,
        values: Vector<VectorDiffContainerStreamElement<S>>,
        f: &F,
    ) -> Option<VectorDiff<VectorDiffContainerStreamElement<S>>>
    where
        F: Fn(&VectorDiffContainerStreamElement<S>) -> bool,
    {
        self.append_filter(values, f).map(|values| VectorDiff::Append { values })
    }

    fn handle_append_filter_map<U, F>(
        &mut self,
        values: Vector<VectorDiffContainerStreamElement<S>>,
        f: &F,
    ) -> Option<VectorDiff<U>>
    where
        U: Clone,
        F: Fn(VectorDiffContainerStreamElement<S>) -> Option<U>,
    {
        self.append_filter_map(values, f).map(|values| VectorDiff::Append { values })
    }

    fn handle_clear<U>(&mut self) -> Option<VectorDiff<U>> {
        self.filtered_indices.clear();
        *self.original_len = 0;
        Some(VectorDiff::Clear)
    }

    fn handle_push_front<U, F>(
        &mut self,
        value: VectorDiffContainerStreamElement<S>,
        f: &F,
    ) -> Option<VectorDiff<U>>
    where
        U: Clone,
        F: Fn(VectorDiffContainerStreamElement<S>) -> Option<U>,
    {
        *self.original_len += 1;
        for idx in &mut *self.filtered_indices {
            *idx += 1;
        }

        f(value).map(|value| {
            self.filtered_indices.push_front(0);
            VectorDiff::PushFront { value }
        })
    }

    fn handle_push_back<U, F>(
        &mut self,
        value: VectorDiffContainerStreamElement<S>,
        f: &F,
    ) -> Option<VectorDiff<U>>
    where
        U: Clone,
        F: Fn(VectorDiffContainerStreamElement<S>) -> Option<U>,
    {
        let original_idx = *self.original_len;
        *self.original_len += 1;
        f(value).map(|value| {
            self.filtered_indices.push_back(original_idx);
            VectorDiff::PushBack { value }
        })
    }

    fn handle_pop_front<U>(&mut self) -> Option<VectorDiff<U>> {
        *self.original_len -= 1;
        let result = self.filtered_indices.front().map_or(false, |&idx| idx == 0).then(|| {
            assert!(self.filtered_indices.pop_front().is_some());
            VectorDiff::PopFront
        });
        for idx in &mut *self.filtered_indices {
            *idx -= 1;
        }

        result
    }

    fn handle_pop_back<U>(&mut self) -> Option<VectorDiff<U>> {
        *self.original_len -= 1;
        self.filtered_indices.back().map_or(false, |&idx| idx == *self.original_len).then(|| {
            assert!(self.filtered_indices.pop_back().is_some());
            VectorDiff::PopBack
        })
    }

    fn handle_insert<U, F>(
        &mut self,
        index: usize,
        value: VectorDiffContainerStreamElement<S>,
        f: &F,
    ) -> Option<VectorDiff<U>>
    where
        U: Clone,
        F: Fn(VectorDiffContainerStreamElement<S>) -> Option<U>,
    {
        let original_idx = index;
        let index = self.filtered_indices.partition_point(|&i| i < original_idx);
        for idx in self.filtered_indices.iter_mut().skip(index) {
            *idx += 1;
        }

        f(value).map(|value| {
            self.filtered_indices.insert(index, original_idx);
            VectorDiff::Insert { index, value }
        })
    }

    fn handle_set<U, F>(
        &mut self,
        index: usize,
        value: VectorDiffContainerStreamElement<S>,
        f: &F,
    ) -> Option<VectorDiff<U>>
    where
        U: Clone,
        F: Fn(VectorDiffContainerStreamElement<S>) -> Option<U>,
    {
        let original_idx = index;
        let new_value = f(value);

        let index = self.filtered_indices.partition_point(|&i| i < original_idx);
        if self.filtered_indices.get(index).map_or(false, |&i| i == original_idx) {
            // The previous value matched the filter
            Some(if let Some(value) = new_value {
                VectorDiff::Set { index, value }
            } else {
                self.filtered_indices.remove(index);
                VectorDiff::Remove { index }
            })
        } else {
            // The previous value didn't match the filter
            new_value.map(|value| {
                self.filtered_indices.insert(index, original_idx);
                VectorDiff::Insert { index, value }
            })
        }
    }

    fn handle_remove<U>(&mut self, index: usize) -> Option<VectorDiff<U>> {
        let original_idx = index;
        *self.original_len -= 1;

        let index = self.filtered_indices.partition_point(|&i| i < original_idx);
        let result =
            self.filtered_indices.get(index).map_or(false, |&i| i == original_idx).then(|| {
                // The value that was removed matched the filter
                self.filtered_indices.remove(index);
                VectorDiff::Remove { index }
            });

        for idx in self.filtered_indices.iter_mut().skip(index) {
            *idx -= 1;
        }

        result
    }

    fn handle_reset_filter<F>(
        &mut self,
        values: Vector<VectorDiffContainerStreamElement<S>>,
        f: &F,
    ) -> Option<VectorDiff<VectorDiffContainerStreamElement<S>>>
    where
        F: Fn(&VectorDiffContainerStreamElement<S>) -> bool,
    {
        self.filtered_indices.clear();
        *self.original_len = 0;
        self.append_filter(values, f).map(|values| VectorDiff::Reset { values })
    }

    fn handle_reset_filter_map<U, F>(
        &mut self,
        values: Vector<VectorDiffContainerStreamElement<S>>,
        f: &F,
    ) -> Option<VectorDiff<U>>
    where
        U: Clone,
        F: Fn(VectorDiffContainerStreamElement<S>) -> Option<U>,
    {
        self.filtered_indices.clear();
        *self.original_len = 0;
        self.append_filter_map(values, f).map(|values| VectorDiff::Reset { values })
    }

    fn handle_diff_filter<F>(&mut self, f: &F, cx: &mut task::Context<'_>) -> Poll<Option<S::Item>>
    where
        F: Fn(&VectorDiffContainerStreamElement<S>) -> bool,
        VectorDiffContainerStreamFamily<S>:
            VectorDiffContainerFamily<Member<VectorDiffContainerStreamElement<S>> = S::Item>,
    {
        // Transform filter function into filter_map function.
        let f2 = |value| f(&value).then_some(value);
        loop {
            let Some(diffs) = ready!(self.inner.as_mut().poll_next(cx)) else {
                return Poll::Ready(None);
            };

            let result = diffs.filter_map(|diff| match diff {
                VectorDiff::Append { values } => self.handle_append_filter(values, f),
                VectorDiff::Clear => self.handle_clear(),
                VectorDiff::PushFront { value } => self.handle_push_front(value, &f2),
                VectorDiff::PushBack { value } => self.handle_push_back(value, &f2),
                VectorDiff::PopFront => self.handle_pop_front(),
                VectorDiff::PopBack => self.handle_pop_back(),
                VectorDiff::Insert { index, value } => self.handle_insert(index, value, &f2),
                VectorDiff::Set { index, value } => self.handle_set(index, value, &f2),
                VectorDiff::Remove { index } => self.handle_remove(index),
                VectorDiff::Reset { values } => self.handle_reset_filter(values, f),
            });

            if let Some(diffs) = result {
                return Poll::Ready(Some(diffs));
            }
        }
    }

    fn handle_diff_filter_map<U, F>(
        &mut self,
        f: &F,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<VectorDiffContainerStreamMappedItem<S, U>>>
    where
        U: Clone,
        F: Fn(VectorDiffContainerStreamElement<S>) -> Option<U>,
    {
        loop {
            let Some(diffs) = ready!(self.inner.as_mut().poll_next(cx)) else {
                return Poll::Ready(None);
            };

            let result = diffs.filter_map(|diff| match diff {
                VectorDiff::Append { values } => self.handle_append_filter_map(values, f),
                VectorDiff::Clear => self.handle_clear(),
                VectorDiff::PushFront { value } => self.handle_push_front(value, f),
                VectorDiff::PushBack { value } => self.handle_push_back(value, f),
                VectorDiff::PopFront => self.handle_pop_front(),
                VectorDiff::PopBack => self.handle_pop_back(),
                VectorDiff::Insert { index, value } => self.handle_insert(index, value, f),
                VectorDiff::Set { index, value } => self.handle_set(index, value, f),
                VectorDiff::Remove { index } => self.handle_remove(index),
                VectorDiff::Reset { values } => self.handle_reset_filter_map(values, f),
            });

            if let Some(diffs) = result {
                return Poll::Ready(Some(diffs));
            }
        }
    }
}
