use std::{
    hash::{Hash, Hasher},
    mem,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        RwLock, Weak,
    },
    task::Waker,
};

#[derive(Debug)]
pub struct ObservableState<T> {
    /// The inner value.
    value: T,

    /// The version of the value.
    ///
    /// Starts at 1 and is incremented by 1 each time the value is updated.
    /// When the observable is dropped, this is set to 0 to indicate no further
    /// updates will happen.
    version: AtomicU64,

    /// List of wakers.
    ///
    /// This is part of `ObservableState` and uses extra locking so that it is
    /// guaranteed that it's only updated by subscribers while the value is
    /// locked for reading. This way, it is guaranteed that between a subscriber
    /// reading the value and adding a waker because the value hasn't changed
    /// yet, no updates to the value could have happened.
    ///
    /// It contains weak references to wakers, so it does not keep references to
    /// [`Subscriber`](crate::Subscriber) or [`Next`](crate::subscriber::Next)
    /// that would otherwise be dropped and won't be awaited again (eg. as part
    /// of a future being cancelled).
    wakers: RwLock<Vec<Weak<Waker>>>,

    /// Whenever wakers.len() reaches this size, iterate through it and remove
    /// dangling weak references.
    /// This is updated in order to only cleanup every time the list of wakers
    /// doubled in size since the previous cleanup, allowing a O(1) amortized
    /// time complexity.
    next_wakers_cleanup_at_len: AtomicUsize,
}

impl<T> ObservableState<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            value,
            version: AtomicU64::new(1),
            wakers: Default::default(),
            next_wakers_cleanup_at_len: AtomicUsize::new(64), // Arbitrary constant
        }
    }

    /// Get a reference to the inner value.
    pub(crate) fn get(&self) -> &T {
        &self.value
    }

    /// Get the current version of the inner value.
    pub(crate) fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }

    pub(crate) fn add_waker(&self, waker: Weak<Waker>) {
        // TODO: clean up dangling Weak references in the vector if there are too many
        let mut wakers = self.wakers.write().unwrap();
        wakers.push(waker);
        if wakers.len() >= self.next_wakers_cleanup_at_len.load(Ordering::Relaxed) {
            // Remove dangling Weak references from the vector to free any
            // cancelled future that awaited on a `Subscriber` of this
            // observable.
            let mut new_wakers = Vec::with_capacity(wakers.len());
            for waker in wakers.iter() {
                if waker.strong_count() > 0 {
                    new_wakers.push(waker.clone());
                }
            }
            if new_wakers.len() == wakers.len() {
                #[cfg(feature = "tracing")]
                tracing::debug!("No dangling wakers among set of {}", wakers.len());
            } else {
                #[cfg(feature = "tracing")]
                tracing::debug!(
                    "Removed {} dangling wakers from a set of {}",
                    wakers.len() - new_wakers.len(),
                    wakers.len()
                );
                std::mem::swap(&mut *wakers, &mut new_wakers);
            }
            self.next_wakers_cleanup_at_len.store(wakers.len() * 2, Ordering::Relaxed);
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
        self.version.store(0, Ordering::Release);
        // Clear the backing buffer for the wakers, no new ones will be added.
        wake(mem::take(&mut *self.wakers.write().unwrap()));
    }

    fn incr_version_and_wake(&mut self) {
        self.version.fetch_add(1, Ordering::Release);
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
    I: IntoIterator<Item = Weak<Waker>>,
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
    let mut num_alive_wakers = 0;
    for waker in iter {
        if let Some(waker) = waker.upgrade() {
            num_alive_wakers += 1;
            waker.wake_by_ref();
        }
    }

    #[cfg(feature = "tracing")]
    {
        tracing::debug!("Woke up {num_alive_wakers} waiting subscribers");
    }
    #[cfg(not(feature = "tracing"))]
    {
        let _ = num_alive_wakers; // For Clippy
    }
}
