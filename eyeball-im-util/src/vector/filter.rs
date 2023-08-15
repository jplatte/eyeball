use std::{
    collections::VecDeque,
    ops::Not,
    pin::Pin,
    task::{self, ready, Poll},
};

use eyeball_im::{Vector, VectorDiff, VectorSubscriber};
use futures_core::Stream;
use pin_project_lite::pin_project;

pin_project! {
    /// A [`VectorSubscriber`] that presents a filtered view of the underlying
    /// [`ObservableVector`]s items.
    ///
    /// Created through [`VectorExt::subscribe_filtered`].
    pub struct Filter<T, F> {
        #[pin]
        pub(super) inner: FilterImpl<T>,
        pub(super) filter: F,
    }
}

pin_project! {
    /// A [`VectorSubscriber`] that presents a filter+mapped view of the
    /// underlying [`ObservableVector`]s items.
    ///
    /// Created through [`VectorExt::subscribe_filter_mapped`].
    pub struct FilterMap<T, F> {
        #[pin]
        pub(super) inner: FilterImpl<T>,
        pub(super) filter: F,
    }
}

pin_project! {
    pub(super) struct FilterImpl<T> {
        #[pin]
        pub(super) inner: VectorSubscriber<T>,
        pub(super) filtered_indices: VecDeque<usize>,
        pub(super) original_len: usize,
    }
}

impl<T> FilterImpl<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn append_filter<F>(mut self: Pin<&mut Self>, mut values: Vector<T>, f: &F) -> Option<Vector<T>>
    where
        F: Fn(&T) -> bool,
    {
        let mut original_idx = self.original_len;
        self.original_len += values.len();
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
        mut self: Pin<&mut Self>,
        values: Vector<T>,
        f: &F,
    ) -> Option<Vector<U>>
    where
        U: Clone,
        F: Fn(T) -> Option<U>,
    {
        let mut original_idx = self.original_len;
        self.original_len += values.len();
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
        self: Pin<&mut Self>,
        values: Vector<T>,
        f: &F,
    ) -> Option<VectorDiff<T>>
    where
        F: Fn(&T) -> bool,
    {
        self.append_filter(values, f).map(|values| VectorDiff::Append { values })
    }

    fn handle_append_filter_map<U, F>(
        self: Pin<&mut Self>,
        values: Vector<T>,
        f: &F,
    ) -> Option<VectorDiff<U>>
    where
        U: Clone,
        F: Fn(T) -> Option<U>,
    {
        self.append_filter_map(values, f).map(|values| VectorDiff::Append { values })
    }

    fn handle_clear<U>(mut self: Pin<&mut Self>) -> Option<VectorDiff<U>> {
        self.filtered_indices.clear();
        self.original_len = 0;
        Some(VectorDiff::Clear)
    }

    fn handle_push_front<U, F>(mut self: Pin<&mut Self>, value: T, f: &F) -> Option<VectorDiff<U>>
    where
        U: Clone,
        F: Fn(T) -> Option<U>,
    {
        self.original_len += 1;
        for idx in &mut self.filtered_indices {
            *idx += 1;
        }

        f(value).map(|value| {
            self.filtered_indices.push_front(0);
            VectorDiff::PushFront { value }
        })
    }

    fn handle_push_back<U, F>(mut self: Pin<&mut Self>, value: T, f: &F) -> Option<VectorDiff<U>>
    where
        U: Clone,
        F: Fn(T) -> Option<U>,
    {
        let original_idx = self.original_len;
        self.original_len += 1;
        f(value).map(|value| {
            self.filtered_indices.push_back(original_idx);
            VectorDiff::PushBack { value }
        })
    }

    fn handle_pop_front<U>(mut self: Pin<&mut Self>) -> Option<VectorDiff<U>> {
        self.original_len -= 1;
        let result = self.filtered_indices.front().map_or(false, |&idx| idx == 0).then(|| {
            assert!(self.filtered_indices.pop_front().is_some());
            VectorDiff::PopFront
        });
        for idx in &mut self.filtered_indices {
            *idx -= 1;
        }

        result
    }

    fn handle_pop_back<U>(mut self: Pin<&mut Self>) -> Option<VectorDiff<U>> {
        self.original_len -= 1;
        self.filtered_indices.back().map_or(false, |&idx| idx == self.original_len).then(|| {
            assert!(self.filtered_indices.pop_back().is_some());
            VectorDiff::PopBack
        })
    }

    fn handle_insert<U, F>(
        mut self: Pin<&mut Self>,
        index: usize,
        value: T,
        f: &F,
    ) -> Option<VectorDiff<U>>
    where
        U: Clone,
        F: Fn(T) -> Option<U>,
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
        mut self: Pin<&mut Self>,
        index: usize,
        value: T,
        f: &F,
    ) -> Option<VectorDiff<U>>
    where
        U: Clone,
        F: Fn(T) -> Option<U>,
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

    fn handle_remove<U>(mut self: Pin<&mut Self>, index: usize) -> Option<VectorDiff<U>> {
        let original_idx = index;
        self.original_len -= 1;

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
        mut self: Pin<&mut Self>,
        values: Vector<T>,
        f: &F,
    ) -> Option<VectorDiff<T>>
    where
        F: Fn(&T) -> bool,
    {
        self.filtered_indices.clear();
        self.original_len = 0;
        self.append_filter(values, f).map(|values| VectorDiff::Reset { values })
    }

    fn handle_reset_filter_map<U, F>(
        mut self: Pin<&mut Self>,
        values: Vector<T>,
        f: &F,
    ) -> Option<VectorDiff<U>>
    where
        U: Clone,
        F: Fn(T) -> Option<U>,
    {
        self.filtered_indices.clear();
        self.original_len = 0;
        self.append_filter_map(values, f).map(|values| VectorDiff::Reset { values })
    }

    fn handle_diff_filter<F>(
        mut self: Pin<&mut Self>,
        f: &F,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<VectorDiff<T>>>
    where
        F: Fn(&T) -> bool,
    {
        // Transform filter function into filter_map function.
        let f2 = |value| f(&value).then_some(value);
        loop {
            let Some(diff) = ready!(self.as_mut().project().inner.poll_next(cx)) else {
                return Poll::Ready(None);
            };

            let this = self.as_mut();
            let result = match diff {
                VectorDiff::Append { values } => this.handle_append_filter(values, f),
                VectorDiff::Clear => this.handle_clear(),
                VectorDiff::PushFront { value } => this.handle_push_front(value, &f2),
                VectorDiff::PushBack { value } => this.handle_push_back(value, &f2),
                VectorDiff::PopFront => this.handle_pop_front(),
                VectorDiff::PopBack => this.handle_pop_back(),
                VectorDiff::Insert { index, value } => this.handle_insert(index, value, &f2),
                VectorDiff::Set { index, value } => this.handle_set(index, value, &f2),
                VectorDiff::Remove { index } => this.handle_remove(index),
                VectorDiff::Reset { values } => this.handle_reset_filter(values, f),
            };

            if let Some(diff) = result {
                return Poll::Ready(Some(diff));
            }
        }
    }

    fn handle_diff_filter_map<U, F>(
        mut self: Pin<&mut Self>,
        f: &F,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<VectorDiff<U>>>
    where
        U: Clone,
        F: Fn(T) -> Option<U>,
    {
        loop {
            let Some(diff) = ready!(self.as_mut().project().inner.poll_next(cx)) else {
                return Poll::Ready(None);
            };

            let this = self.as_mut();
            let result = match diff {
                VectorDiff::Append { values } => this.handle_append_filter_map(values, f),
                VectorDiff::Clear => this.handle_clear(),
                VectorDiff::PushFront { value } => this.handle_push_front(value, f),
                VectorDiff::PushBack { value } => this.handle_push_back(value, f),
                VectorDiff::PopFront => this.handle_pop_front(),
                VectorDiff::PopBack => this.handle_pop_back(),
                VectorDiff::Insert { index, value } => this.handle_insert(index, value, f),
                VectorDiff::Set { index, value } => this.handle_set(index, value, f),
                VectorDiff::Remove { index } => this.handle_remove(index),
                VectorDiff::Reset { values } => this.handle_reset_filter_map(values, f),
            };

            if let Some(diff) = result {
                return Poll::Ready(Some(diff));
            }
        }
    }
}

impl<T: Clone + Send + Sync + 'static, F> Stream for Filter<T, F>
where
    F: Fn(&T) -> bool,
{
    type Item = VectorDiff<T>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        let projected = self.project();
        projected.inner.handle_diff_filter(&*projected.filter, cx)
    }
}

impl<T: Clone + Send + Sync + 'static, U: Clone, F> Stream for FilterMap<T, F>
where
    U: Clone,
    F: Fn(T) -> Option<U>,
{
    type Item = VectorDiff<U>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        let projected = self.project();
        projected.inner.handle_diff_filter_map(&*projected.filter, cx)
    }
}
