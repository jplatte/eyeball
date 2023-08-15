//! Utilities around [`ObservableVector`].

use std::collections::VecDeque;

use eyeball_im::{ObservableVector, Vector};

mod filter;

use self::filter::FilterImpl;
pub use self::filter::{Filter, FilterMap};

/// Extension trait for [`ObservableVector`].
pub trait VectorExt<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// Obtain a new subscriber that filters items by the given filter function.
    ///
    /// Returns a filtered version of the current vector, and a subscriber to
    /// get updates through.
    fn subscribe_filter<F>(&self, filter: F) -> (Vector<T>, Filter<T, F>)
    where
        F: Fn(&T) -> bool;

    /// Obtain a new subscriber that filters and maps items with the given
    /// function.
    ///
    /// Returns a filtered + mapped version of the current vector, and a
    /// subscriber to get updates through.
    fn subscribe_filter_map<U, F>(&self, filter: F) -> (Vector<U>, FilterMap<T, F>)
    where
        U: Clone,
        F: Fn(T) -> Option<U>;
}

impl<T> VectorExt<T> for ObservableVector<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn subscribe_filter<F>(&self, filter: F) -> (Vector<T>, Filter<T, F>)
    where
        F: Fn(&T) -> bool,
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
        let inner = FilterImpl { inner, filtered_indices, original_len };
        let sub = Filter { inner, filter };

        (v, sub)
    }

    fn subscribe_filter_map<U, F>(&self, filter: F) -> (Vector<U>, FilterMap<T, F>)
    where
        U: Clone,
        F: Fn(T) -> Option<U>,
    {
        let (v, filtered_indices) = self
            .iter()
            .enumerate()
            .filter_map(|(original_idx, val)| {
                filter(val.clone()).map(|mapped| (mapped, original_idx))
            })
            .unzip();

        let inner = self.subscribe();
        let original_len = self.len();
        let inner = FilterImpl { inner, filtered_indices, original_len };
        let sub = FilterMap { inner, filter };

        (v, sub)
    }
}
