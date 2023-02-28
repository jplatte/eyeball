use std::{
    hash::{Hash, Hasher},
    mem,
    sync::RwLock,
    task::Waker,
};

#[derive(Debug)]
pub(crate) struct ObservableState<T> {
    /// The inner value.
    value: T,

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
    wakers: RwLock<Vec<Waker>>,
}

impl<T> ObservableState<T> {
    pub(crate) fn new(value: T) -> Self {
        Self { value, version: 1, wakers: Default::default() }
    }

    /// Get a reference to the inner value.
    pub(crate) fn get(&self) -> &T {
        &self.value
    }

    /// Get the current version of the inner value.
    pub(crate) fn version(&self) -> u64 {
        self.version
    }

    pub(crate) fn add_waker(&self, waker: Waker) {
        self.wakers.write().unwrap().push(waker);
    }

    pub(crate) fn set(&mut self, value: T) {
        self.value = value;
        self.incr_version_and_wake();
    }

    pub(crate) fn replace(&mut self, value: T) -> T {
        let result = mem::replace(&mut self.value, value);
        self.incr_version_and_wake();
        result
    }

    pub fn update(&mut self, f: impl FnOnce(&mut T)) {
        f(&mut self.value);
        self.incr_version_and_wake();
    }

    pub fn update_eq(&mut self, f: impl FnOnce(&mut T))
    where
        T: Clone + PartialEq,
    {
        let prev = self.value.clone();
        f(&mut self.value);
        if self.value != prev {
            self.incr_version_and_wake();
        }
    }

    pub fn update_hash(&mut self, f: impl FnOnce(&mut T))
    where
        T: Hash,
    {
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        self.value.hash(&mut hasher);
        let prev_hash = hasher.finish();

        f(&mut self.value);

        let mut hasher = DefaultHasher::new();
        self.value.hash(&mut hasher);
        let new_hash = hasher.finish();

        if prev_hash != new_hash {
            self.incr_version_and_wake();
        }
    }

    /// "Close" the state – indicate that no further updates will happen.
    pub(crate) fn close(&mut self) {
        self.version = 0;
        // Clear the backing buffer for the wakers, no new ones will be added.
        wake(mem::take(self.wakers.get_mut().unwrap()));
    }

    fn incr_version_and_wake(&mut self) {
        self.version += 1;
        wake(self.wakers.get_mut().unwrap().drain(..));
    }
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
        }
    }
    for waker in iter {
        waker.wake();
    }
}
