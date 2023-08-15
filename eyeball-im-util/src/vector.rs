//! Utilities around [`ObservableVector`].

use eyeball_im::{ObservableVector, Vector, VectorSubscriber};

mod filter;

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
    fn subscribe_filter<F>(&self, f: F) -> (Vector<T>, Filter<VectorSubscriber<T>, F>)
    where
        F: Fn(&T) -> bool;

    /// Obtain a new subscriber that filters and maps items with the given
    /// function.
    ///
    /// Returns a filtered + mapped version of the current vector, and a
    /// subscriber to get updates through.
    fn subscribe_filter_map<U, F>(&self, f: F) -> (Vector<U>, FilterMap<VectorSubscriber<T>, F>)
    where
        U: Clone,
        F: Fn(T) -> Option<U>;
}

impl<T> VectorExt<T> for ObservableVector<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn subscribe_filter<F>(&self, f: F) -> (Vector<T>, Filter<VectorSubscriber<T>, F>)
    where
        F: Fn(&T) -> bool,
    {
        Filter::new((*self).clone(), self.subscribe(), f)
    }

    fn subscribe_filter_map<U, F>(&self, f: F) -> (Vector<U>, FilterMap<VectorSubscriber<T>, F>)
    where
        U: Clone,
        F: Fn(T) -> Option<U>,
    {
        FilterMap::new((*self).clone(), self.subscribe(), f)
    }
}
