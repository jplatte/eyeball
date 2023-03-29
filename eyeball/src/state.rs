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

    pub(crate) fn set(&mut self, value: T) -> T {
        let result = mem::replace(&mut self.value, value);
        self.incr_version_and_wake();
        result
    }

    pub(crate) fn set_eq(&mut self, value: T) -> Option<T>
    where
        T: PartialEq,
    {
        if self.value != value {
            Some(self.set(value))
        } else {
            None
        }
    }

    pub fn set_hash(&mut self, value: T) -> Option<T>
    where
        T: Hash,
    {
        if hash(&self.value) != hash(&value) {
            Some(self.set(value))
        } else {
            None
        }
    }

    pub fn update(&mut self, f: impl FnOnce(&mut T)) {
        f(&mut self.value);
        self.incr_version_and_wake();
    }

    pub fn update_if(&mut self, f: impl FnOnce(&mut T) -> bool) {
        if f(&mut self.value) {
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
        }
    }
    for waker in iter {
        waker.wake();
    }
}
