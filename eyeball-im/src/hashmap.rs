#![allow(missing_docs)] // FIXME

use std::{
    collections::hash_map::RandomState,
    fmt,
    hash::{BuildHasher, Hash},
    ops,
    sync::Arc,
};

use imbl::HashMap;
use tokio::sync::broadcast::{self, Sender};

pub struct ObservableHashMap<K, V, S = RandomState> {
    values: HashMap<K, V, S>,
    sender: Sender<BroadcastMessage<K, V, S>>,
}

impl<K, V> ObservableHashMap<K, V>
where
    K: Clone + Hash + Eq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// Create a new `ObservableHashMap`.
    ///
    /// As of the time of writing, this is equivalent to
    /// `ObservableHashMap::with_capacity(16)`, but the internal buffer capacity
    /// is subject to change in non-breaking releases.
    ///
    /// See [`with_capacity`][Self::with_capacity] for details about the buffer
    /// capacity.
    pub fn new() -> Self {
        Self::with_hasher(RandomState::new())
    }

    /// Create a new `ObservableHashMap` with the given capacity for the inner
    /// buffer.
    ///
    /// Up to `capacity` updates that have not been received by all of the
    /// subscribers yet will be retained in the inner buffer. If an update
    /// happens while the buffer is at capacity, the oldest update is discarded
    /// from it and all subscribers that have not yet received it will instead
    /// see [`VectorDiff::Reset`] as the next update.
    ///
    /// # Panics
    ///
    /// Panics if the capacity is `0`, or larger than `usize::MAX / 2`.
    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_hasher_and_capacity(RandomState::new(), capacity)
    }
}

impl<K, V, S> ObservableHashMap<K, V, S>
where
    K: Clone + Hash + Eq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    pub fn with_hasher<RS>(hasher: RS) -> Self
    where
        Arc<S>: From<RS>,
    {
        Self::with_hasher_and_capacity(hasher, 16)
    }

    pub fn with_hasher_and_capacity<RS>(hasher: RS, capacity: usize) -> Self
    where
        Arc<S>: From<RS>,
    {
        let (sender, _) = broadcast::channel(capacity);
        Self { values: HashMap::with_hasher(hasher), sender }
    }

    pub fn clear(&mut self) {
        #[cfg(feature = "tracing")]
        tracing::debug!(target: "eyeball_im::hashmap::update", "clear");

        self.values.clear();
        self.broadcast_diff(HashMapDiff::Clear);
    }

    fn broadcast_diff(&self, diff: HashMapDiff<K, V, S>) {
        if self.sender.receiver_count() != 0 {
            let msg = BroadcastMessage { diff, state: self.values.clone() };
            let _num_receivers = self.sender.send(msg).unwrap_or(0);
            #[cfg(feature = "tracing")]
            tracing::debug!(
                target: "eyeball_im::hashmap::broadcast",
                "New observable value broadcast to {_num_receivers} receivers"
            );
        }
    }
}

impl<K, V, S> ObservableHashMap<K, V, S>
where
    K: Clone + Hash + Eq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    S: BuildHasher,
{
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        #[cfg(feature = "tracing")]
        tracing::debug!(target: "eyeball_im::hashmap::update", "push_back");

        let old_value = self.values.insert(key.clone(), value.clone());
        self.broadcast_diff(HashMapDiff::Insert { key, value });
        old_value
    }
}

impl<K, V, S> Default for ObservableHashMap<K, V, S>
where
    K: Clone + Hash + Eq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    S: BuildHasher + Default,
{
    fn default() -> Self {
        let (sender, _) = broadcast::channel(16);
        Self { values: Default::default(), sender }
    }
}

impl<K, V, S> fmt::Debug for ObservableHashMap<K, V, S>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObservableHashMap").field("values", &self.values).finish_non_exhaustive()
    }
}

impl<K, V> ops::Deref for ObservableHashMap<K, V> {
    type Target = HashMap<K, V>;

    fn deref(&self) -> &Self::Target {
        &self.values
    }
}

struct BroadcastMessage<K, V, S = RandomState> {
    diff: HashMapDiff<K, V, S>,
    state: HashMap<K, V, S>,
}

impl<K, V, S> Clone for BroadcastMessage<K, V, S>
where
    K: Clone,
    V: Clone,
{
    fn clone(&self) -> Self {
        Self { diff: self.diff.clone(), state: self.state.clone() }
    }
}

/// A change to an [`ObservableHashMap`].
pub enum HashMapDiff<K, V, S = RandomState> {
    Add {
        /// The added elements.
        values: HashMap<K, V, S>,
    },
    Clear,
    Insert {
        key: K,
        value: V,
    },
    Set {
        key: K,
        value: V,
    },
    Remove {
        key: K,
    },
    Reset {
        /// The full list of elements.
        values: HashMap<K, V, S>,
    },
}

impl<K, V, S> Clone for HashMapDiff<K, V, S>
where
    K: Clone,
    V: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Self::Add { values } => Self::Add { values: values.clone() },
            Self::Clear => Self::Clear,
            Self::Insert { key, value } => Self::Insert { key: key.clone(), value: value.clone() },
            Self::Set { key, value } => Self::Set { key: key.clone(), value: value.clone() },
            Self::Remove { key } => Self::Remove { key: key.clone() },
            Self::Reset { values } => Self::Reset { values: values.clone() },
        }
    }
}

impl<K, V, S> fmt::Debug for HashMapDiff<K, V, S>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add { values } => f.debug_struct("Add").field("values", values).finish(),
            Self::Clear => write!(f, "Clear"),
            Self::Insert { key, value } => {
                f.debug_struct("Insert").field("key", key).field("value", value).finish()
            }
            Self::Set { key, value } => {
                f.debug_struct("Set").field("key", key).field("value", value).finish()
            }
            Self::Remove { key } => f.debug_struct("Remove").field("key", key).finish(),
            Self::Reset { values } => f.debug_struct("Reset").field("values", values).finish(),
        }
    }
}
