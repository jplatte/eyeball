use std::{
    hash::{Hash, Hasher},
    mem::{self, MaybeUninit},
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

impl<T> ObservableState<MaybeUninit<T>> {
    /// Whether this state is initialized.
    pub(crate) fn is_initialized(&self) -> bool {
        // If the version is larger than 1, that means it was written before
        // and it is valid to move it out as long a value is put back in.
        self.version > 1
    }

    fn write(&mut self, value: T) {
        self.value.write(value);
        self.incr_version_and_wake();
    }

    /// Get a reference to the inner value, if it has been initialized.
    pub(crate) fn get_lazy(&self) -> Option<&T> {
        self.is_initialized().then(|| unsafe { self.value.assume_init_ref() })
    }

    pub(crate) fn init_or_set(&mut self, value: T) -> Option<T> {
        let old_value = self.is_initialized().then(|| {
            // SAFETY: `self.value.write(value);` below re-initializes
            // and both functions never panic.
            unsafe { self.value.assume_init_read() }
        });
        self.write(value);
        old_value
    }

    fn init_or_set_if(&mut self, value: T, f: impl FnOnce(&T, &T) -> bool) -> Option<T> {
        if self.is_initialized() {
            // SAFETY: Value is initialized, we don't use the reference after
            // moving out of `self.value` using `assume_init_read`.
            let prev_value = unsafe { self.value.assume_init_ref() };
            if f(prev_value, &value) {
                // SAFETY: `self.value.write(value);` below re-initializes
                // and both functions never panic.
                let old_value = unsafe { self.value.assume_init_read() };
                self.write(value);
                Some(old_value)
            } else {
                None
            }
        } else {
            self.write(value);
            None
        }
    }

    pub(crate) fn init_or_set_if_not_eq(&mut self, value: T) -> Option<T>
    where
        T: PartialEq,
    {
        self.init_or_set_if(value, |a, b| a != b)
    }

    pub(crate) fn init_or_set_if_hash_not_eq(&mut self, value: T) -> Option<T>
    where
        T: Hash,
    {
        self.init_or_set_if(value, |a, b| hash(a) != hash(b))
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
