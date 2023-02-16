use std::{
    fmt, ops,
    pin::Pin,
    task::{Context, Poll},
};

use futures_channel::mpsc;
use futures_core::Stream;
use pin_project_lite::pin_project;

/// An ordered list of elements that broadcasts any changes made to it.
pub struct Vector<T: Clone> {
    values: im::Vector<T>,
    senders: Vec<mpsc::UnboundedSender<VectorDiff<T>>>,
}

impl<T: Clone> Vector<T> {
    /// Create a new `Vector`.
    pub fn new() -> Self {
        Self { values: Default::default(), senders: Default::default() }
    }

    /// Obtain a new subscriber.
    ///
    /// If the inner list of elements is non-empty, it will immediately receive an update.
    pub fn subscribe(&mut self) -> VectorSubscriber<T> {
        let (tx, rx) = mpsc::unbounded();
        if !self.values.is_empty() {
            tx.unbounded_send(VectorDiff::Append { values: self.values.clone() }).unwrap();
        }
        self.senders.push(tx);

        VectorSubscriber::new(rx)
    }

    /// Obtain a new subscriber and the current state of the inner list of elements.
    ///
    /// The subscriber will start receiving updates as further mutations are processed.
    pub fn get_subscribe(&mut self) -> (im::Vector<T>, VectorSubscriber<T>) {
        let (tx, rx) = mpsc::unbounded();
        self.senders.push(tx);
        (self.values.clone(), VectorSubscriber::new(rx))
    }

    /// Append the given elements at the end of the `Vector` and notify subscribers.
    pub fn append(&mut self, other: im::Vector<T>) {
        self.notify(|| VectorDiff::Append { values: other.clone() });
        self.values.append(other);
    }

    /// Clear out all of the elements in this `Vector` and notify subscribers.
    pub fn clear(&mut self) {
        self.notify(|| VectorDiff::Clear);
        self.values.clear();
    }

    /// Add an element at the front of the list and notify subscribers.
    pub fn push_front(&mut self, value: T) {
        self.notify(|| VectorDiff::PushFront { value: value.clone() });
        self.values.push_front(value);
    }

    /// Add an element at the front of the list and notify subscribers.
    pub fn push_back(&mut self, value: T) {
        self.notify(|| VectorDiff::PushBack { value: value.clone() });
        self.values.push_back(value);
    }

    /// Remove the first element, notify subscribers and return the element.
    ///
    /// If there are no elements, subscribers will not be notified and this method will return
    /// `None`.
    pub fn pop_front(&mut self) -> Option<T> {
        let value = self.values.pop_front();
        if value.is_some() {
            self.notify(|| VectorDiff::PopFront);
        }
        value
    }

    /// Remove the last element, notify subscribers and return the element.
    ///
    /// If there are no elements, subscribers will not be notified and this method will return
    /// `None`.
    pub fn pop_back(&mut self) -> Option<T> {
        let value = self.values.pop_back();
        if value.is_some() {
            self.notify(|| VectorDiff::PopBack);
        }
        value
    }

    /// Insert an element at the given position and notify subscribers.
    ///
    /// If the index is `0`, the notification will contain [`VectorDiff::PushFront`] instead of
    /// [`VectorDiff::Insert`]. If it is equal to the amount of elements already in the list, the
    /// notification will contain [`VectorDiff::PushBack`].
    ///
    /// # Panics
    ///
    /// Panics if `index > len`.
    #[track_caller]
    pub fn insert(&mut self, index: usize, value: T) {
        let len = self.values.len();
        if index == 0 {
            self.push_front(value);
        } else if index == len {
            self.push_back(value);
        } else if index < len {
            self.notify(|| VectorDiff::Insert { index, value: value.clone() });
            self.values.insert(index, value);
        } else {
            panic!("index out of bounds: the length is {len} but the index is {index}");
        }
    }

    /// Replace the element at the given position, notify subscribers and return the previous
    /// element at that position.
    ///
    /// # Panics
    ///
    /// Panics if `index > len - 1`.
    #[track_caller]
    pub fn set(&mut self, index: usize, value: T) -> T {
        let len = self.values.len();
        if index < len - 1 {
            self.notify(|| VectorDiff::Set { index, value: value.clone() });
            self.values.set(index, value)
        } else {
            panic!("index out of bounds: the length is {len} but the index is {index}");
        }
    }

    /// Remove the element at the given position, notify subscribers and return the element.
    ///
    /// # Panics
    ///
    /// Panics if `index > len - 1`.
    #[track_caller]
    pub fn remove(&mut self, index: usize) -> T {
        let len = self.values.len();
        if index < len - 1 {
            self.notify(|| VectorDiff::Remove { index });
            self.values.remove(index)
        } else {
            panic!("index out of bounds: the length is {len} but the index is {index}");
        }
    }

    fn notify(&mut self, get_diff: impl Fn() -> VectorDiff<T>) {
        self.senders.retain_mut(move |sender| match sender.unbounded_send(get_diff()) {
            Ok(_) => true,
            Err(e) if e.is_disconnected() => false,
            Err(e) => panic!("logic error: {e}"),
        })
    }
}

impl<T> fmt::Debug for Vector<T>
where
    T: Clone + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vector").field("values", &self.values).finish_non_exhaustive()
    }
}

impl<T: Clone> Default for Vector<T> {
    fn default() -> Self {
        Self { values: Default::default(), senders: Default::default() }
    }
}

// Note: No DerefMut because all mutating must go through inherent methods that notify subscribers
impl<T: Clone> ops::Deref for Vector<T> {
    type Target = im::Vector<T>;

    fn deref(&self) -> &Self::Target {
        &self.values
    }
}

pin_project! {
    pub struct VectorSubscriber<T: Clone> {
        #[pin]
        inner: mpsc::UnboundedReceiver<VectorDiff<T>>,
    }
}

impl<T: Clone> VectorSubscriber<T> {
    fn new(inner: mpsc::UnboundedReceiver<VectorDiff<T>>) -> Self {
        Self { inner }
    }
}

impl<T: Clone> Stream for VectorSubscriber<T> {
    type Item = VectorDiff<T>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().inner.poll_next(cx)
    }
}

/// A change to a [`Vector`].
#[derive(Clone, Debug)]
// FIXME: Clone bound really necessary?
pub enum VectorDiff<T: Clone> {
    /// A vector was appended.
    ///
    /// This is also the first diff you will receive if you subscribe to changes on a [`Vector`]
    /// that already has elements in it.
    Append {
        /// The appended elements.
        values: im::Vector<T>,
    },
    /// The vector was cleared.
    Clear,
    /// An element was added at the front.
    PushFront {
        /// The new element.
        value: T,
    },
    /// An element was added at the back.
    PushBack {
        /// The new element.
        value: T,
    },
    /// The element at the front was removed.
    PopFront,
    /// The element at the back was removed.
    PopBack,
    /// An element was inserted at the given position.
    Insert {
        /// The index of the new element.
        ///
        /// The element that was previously at that index as well as all the ones after it were
        /// shifted to the right.
        index: usize,
        /// The new element.
        value: T,
    },
    /// A replacement of the previous value at the given position.
    Set {
        /// The index of the element that was replaced.
        index: usize,
        /// The new element.
        value: T,
    },
    /// Removal of an element.
    Remove {
        /// The index that the removed element had.
        index: usize,
    },
}
