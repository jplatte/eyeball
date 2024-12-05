use std::{
    hash::{Hash, Hasher},
    mem,
    sync::RwLock,
    task::{Context, Poll, Waker},
};

use slab::Slab;

#[derive(Debug)]
pub struct ObservableState<T> {
    /// The wrapped value.
    value: T,

    /// The attached observable metadata.
    metadata: RwLock<ObservableStateMetadata>,
}

#[derive(Debug)]
struct ObservableStateMetadata {
    /// The version of the value.
    ///
    /// Starts at 1 and is incremented by 1 each time the value is updated.
    /// When the observable is dropped, this is set to 0 to indicate no further
    /// updates will happen.
    version: u64,

    /// List of wakers.
    ///
    /// This is part of `ObservableState` and uses extra locking so that it is
    /// guaranteed that it's only updated by subscribers while the value is
    /// locked for reading. This way, it is guaranteed that between a subscriber
    /// reading the value and adding a waker because the value hasn't changed
    /// yet, no updates to the value could have happened.
    wakers: Slab<Waker>,
}

impl Default for ObservableStateMetadata {
    fn default() -> Self {
        Self { version: 1, wakers: Slab::new() }
    }
}

impl<T> ObservableState<T> {
    pub(crate) fn new(value: T) -> Self {
        Self { value, metadata: Default::default() }
    }

    /// Get a reference to the inner value.
    pub(crate) fn get(&self) -> &T {
        &self.value
    }

    /// Get the current version of the inner value.
    pub(crate) fn version(&self) -> u64 {
        self.metadata.read().unwrap().version
    }

    pub(crate) fn poll_update(
        &self,
        observed_version: &mut u64,
        waker_key: &mut Option<usize>,
        cx: &Context<'_>,
    ) -> Poll<Option<()>> {
        let mut metadata = self.metadata.write().unwrap();

        if metadata.version == 0 {
            *waker_key = None;
            Poll::Ready(None)
        } else if *observed_version < metadata.version {
            *waker_key = None;
            *observed_version = metadata.version;
            Poll::Ready(Some(()))
        } else {
            *waker_key = Some(metadata.wakers.insert(cx.waker().clone()));
            Poll::Pending
        }
    }

    pub(crate) fn drop_waker(&self, observed_version: u64, waker_key: usize) {
        let mut metadata = self.metadata.write().unwrap();
        if metadata.version == observed_version {
            let _res = metadata.wakers.try_remove(waker_key);
            debug_assert!(_res.is_some());
        }
    }

    pub(crate) fn set(&mut self, value: T) -> T {
        let result = mem::replace(&mut self.value, value);
        self.incr_version_and_wake();
        result
    }

    pub(crate) fn set_if_not_eq(&mut self, value: T) -> Option<T>
    where
        T: PartialEq,
    {
        if self.value != value {
            Some(self.set(value))
        } else {
            None
        }
    }

    pub(crate) fn set_if_hash_not_eq(&mut self, value: T) -> Option<T>
    where
        T: Hash,
    {
        if hash(&self.value) != hash(&value) {
            Some(self.set(value))
        } else {
            None
        }
    }

    pub(crate) fn update(&mut self, f: impl FnOnce(&mut T)) {
        f(&mut self.value);
        self.incr_version_and_wake();
    }

    pub(crate) fn update_if(&mut self, f: impl FnOnce(&mut T) -> bool) {
        if f(&mut self.value) {
            self.incr_version_and_wake();
        }
    }

    /// "Close" the state – indicate that no further updates will happen.
    pub(crate) fn close(&self) {
        let mut metadata = self.metadata.write().unwrap();
        metadata.version = 0;
        // Clear the backing buffer for the wakers, no new ones will be added.
        wake(mem::take(&mut metadata.wakers).into_iter().map(|(_, val)| val));
    }

    fn incr_version_and_wake(&mut self) {
        let metadata = self.metadata.get_mut().unwrap();
        metadata.version += 1;
        wake(metadata.wakers.drain());
    }
}

fn hash<T: Hash>(value: &T) -> u64 {
    use std::collections::hash_map::DefaultHasher;

    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn wake<I>(wakers: I)
where
    I: IntoIterator<Item = Waker>,
    I::IntoIter: ExactSizeIterator,
{
    let iter = wakers.into_iter();
    #[cfg(feature = "tracing")]
    {
        let num_wakers = iter.len();
        if num_wakers > 0 {
            tracing::debug!("Waking up {num_wakers} waiting subscribers");
        } else {
            tracing::debug!("No wakers");
        }
    }
    for waker in iter {
        waker.wake();
    }
}
