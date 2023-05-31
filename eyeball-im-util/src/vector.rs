use std::{
    collections::VecDeque,
    ops::Not,
    pin::Pin,
    task::{self, ready, Poll},
};

use eyeball_im::{ObservableVector, Vector, VectorDiff, VectorSubscriber};
use futures_core::Stream;
use pin_project_lite::pin_project;

/// Extension trait for [`Vector`].
pub trait VectorExt<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// Obtain a new subscriber that filters items by the given filter function.
    ///
    /// Returns a filtered version of the current vector, and a subscriber to
    /// get updates through.
    fn subscribe_filtered<F>(&self, filter: F) -> (Vector<T>, FilteredVectorSubscriber<T, F>)
    where
        F: Fn(&T) -> bool + Unpin;
}

impl<T> VectorExt<T> for ObservableVector<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn subscribe_filtered<F>(&self, filter: F) -> (Vector<T>, FilteredVectorSubscriber<T, F>)
    where
        F: Fn(&T) -> bool + Unpin,
    {
        let mut filtered_indices = VecDeque::new();
        let mut v = (*self).clone();

        let mut original_idx = 0;
        v.retain(|val| {
            let keep = filter(val);
            if keep {
                filtered_indices.push_back(original_idx);
            }
            original_idx += 1;
            keep
        });

        let inner = self.subscribe();
        let original_len = self.len();
        let sub = FilteredVectorSubscriber { inner, filter, filtered_indices, original_len };

        (v, sub)
    }
}

pin_project! {
    /// A [`VectorSubscriber`] that presents a filtered view of the underlying
    /// [`ObservableVector`]s items.
    ///
    /// Created through [`VectorExt::subscribe_filtered`].
    pub struct FilteredVectorSubscriber<T, F> {
        #[pin]
        inner: VectorSubscriber<T>,
        filter: F,
        filtered_indices: VecDeque<usize>,
        original_len: usize,
    }
}

impl<T, F> FilteredVectorSubscriber<T, F>
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T) -> bool,
{
    fn append(mut self: Pin<&mut Self>, mut values: Vector<T>) -> Option<Vector<T>> {
        let mut original_idx = self.original_len;
        self.original_len += values.len();
        values.retain(|value| {
            let keep = (self.filter)(value);
            if keep {
                self.filtered_indices.push_back(original_idx);
            }
            original_idx += 1;
            keep
        });

        values.is_empty().not().then_some(values)
    }

    fn handle_append(self: Pin<&mut Self>, values: Vector<T>) -> Option<VectorDiff<T>> {
        self.append(values).map(|values| VectorDiff::Append { values })
    }

    fn handle_clear(mut self: Pin<&mut Self>) -> Option<VectorDiff<T>> {
        self.filtered_indices.clear();
        self.original_len = 0;
        Some(VectorDiff::Clear)
    }

    fn handle_push_front(mut self: Pin<&mut Self>, value: T) -> Option<VectorDiff<T>> {
        self.original_len += 1;
        for idx in &mut self.filtered_indices {
            *idx += 1;
        }

        (self.filter)(&value).then(|| {
            self.filtered_indices.push_front(0);
            VectorDiff::PushFront { value }
        })
    }

    fn handle_push_back(mut self: Pin<&mut Self>, value: T) -> Option<VectorDiff<T>> {
        let original_idx = self.original_len;
        self.original_len += 1;
        (self.filter)(&value).then(|| {
            self.filtered_indices.push_back(original_idx);
            VectorDiff::PushBack { value }
        })
    }

    fn handle_pop_front(mut self: Pin<&mut Self>) -> Option<VectorDiff<T>> {
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

    fn handle_pop_back(mut self: Pin<&mut Self>) -> Option<VectorDiff<T>> {
        self.original_len -= 1;
        self.filtered_indices.back().map_or(false, |&idx| idx == self.original_len).then(|| {
            assert!(self.filtered_indices.pop_back().is_some());
            VectorDiff::PopBack
        })
    }

    fn handle_insert(mut self: Pin<&mut Self>, index: usize, value: T) -> Option<VectorDiff<T>> {
        let original_idx = index;
        let index = self.filtered_indices.partition_point(|&i| i < original_idx);
        for idx in self.filtered_indices.iter_mut().skip(index) {
            *idx += 1;
        }

        (self.filter)(&value).then(|| {
            self.filtered_indices.insert(index, original_idx);
            VectorDiff::Insert { index, value }
        })
    }

    fn handle_set(mut self: Pin<&mut Self>, index: usize, value: T) -> Option<VectorDiff<T>> {
        let original_idx = index;
        let new_value_matches = (self.filter)(&value);

        let index = self.filtered_indices.partition_point(|&i| i < original_idx);
        if self.filtered_indices.get(index).map_or(false, |&i| i == original_idx) {
            // The previous value matched the filter
            Some(if new_value_matches {
                VectorDiff::Set { index, value }
            } else {
                self.filtered_indices.remove(index);
                VectorDiff::Remove { index }
            })
        } else {
            // The previous value didn't match the filter
            new_value_matches.then(|| {
                self.filtered_indices.insert(index, original_idx);
                VectorDiff::Insert { index, value }
            })
        }
    }

    fn handle_remove(mut self: Pin<&mut Self>, index: usize) -> Option<VectorDiff<T>> {
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

    fn handle_reset(mut self: Pin<&mut Self>, values: Vector<T>) -> Option<VectorDiff<T>> {
        self.filtered_indices.clear();
        self.original_len = 0;
        self.append(values).map(|values| VectorDiff::Reset { values })
    }
}

impl<T: Clone + Send + Sync + 'static, F> Stream for FilteredVectorSubscriber<T, F>
where
    F: Fn(&T) -> bool + Unpin,
{
    type Item = VectorDiff<T>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            let Some(diff) = ready!(self.as_mut().project().inner.poll_next(cx)) else {
                return Poll::Ready(None);
            };

            let result = match diff {
                VectorDiff::Append { values } => self.as_mut().handle_append(values),
                VectorDiff::Clear => self.as_mut().handle_clear(),
                VectorDiff::PushFront { value } => self.as_mut().handle_push_front(value),
                VectorDiff::PushBack { value } => self.as_mut().handle_push_back(value),
                VectorDiff::PopFront => self.as_mut().handle_pop_front(),
                VectorDiff::PopBack => self.as_mut().handle_pop_back(),
                VectorDiff::Insert { index, value } => self.as_mut().handle_insert(index, value),
                VectorDiff::Set { index, value } => self.as_mut().handle_set(index, value),
                VectorDiff::Remove { index } => self.as_mut().handle_remove(index),
                VectorDiff::Reset { values } => self.as_mut().handle_reset(values),
            };

            if let Some(diff) = result {
                return Poll::Ready(Some(diff));
            }
        }
    }
}
