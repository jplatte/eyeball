use eyeball_im::{ObservableVector, VectorDiff};
use eyeball_im_util::vector::VectorObserverExt;
use imbl::vector;
use std::cmp::Ordering;
use stream_assert::{assert_closed, assert_next_eq, assert_pending};

/// Reversed sorting function.
///
/// `sort_by(rev_cmp)` is equivalent to `sort_by_key(std::cmp::Reverse)`
fn rev_cmp<T>(left: &T, right: &T) -> Ordering
where
    T: Ord,
{
    right.cmp(left)
}

#[test]
fn new() {
    let ob = ObservableVector::<char>::from(vector!['c', 'a', 'd', 'b']);
    let (values, mut sub) = ob.subscribe().sort_by(rev_cmp);

    assert_eq!(values, vector!['d', 'c', 'b', 'a']);
    assert_pending!(sub);

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn append() {
    let mut ob = ObservableVector::<char>::new();
    let (values, mut sub) = ob.subscribe().sort_by(rev_cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    // Append on an empty vector.
    ob.append(vector!['d', 'e', 'e']);
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['e', 'e', 'd'] });

    // Append on an non-empty vector.
    ob.append(vector!['f', 'g', 'c']);
    assert_next_eq!(sub, VectorDiff::PushFront { value: 'g' });
    assert_next_eq!(sub, VectorDiff::Insert { index: 1, value: 'f' });
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['c'] });

    // Another append.
    ob.append(vector!['i', 'h', 'j', 'a', 'b']);
    assert_next_eq!(sub, VectorDiff::PushFront { value: 'j' });
    assert_next_eq!(sub, VectorDiff::Insert { index: 1, value: 'i' });
    assert_next_eq!(sub, VectorDiff::Insert { index: 2, value: 'h' });
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['b', 'a'] });

    // Items in the vector have been appended and are not sorted.
    assert_eq!(*ob, vector!['d', 'e', 'e', 'f', 'g', 'c', 'i', 'h', 'j', 'a', 'b']);

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn clear() {
    let mut ob = ObservableVector::<char>::new();
    let (values, mut sub) = ob.subscribe().sort_by(rev_cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    ob.append(vector!['b', 'a', 'c']);
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['c', 'b', 'a'] });

    assert_eq!(*ob, vector!['b', 'a', 'c']);

    // Let's clear it.
    ob.clear();

    assert_next_eq!(sub, VectorDiff::Clear);

    // Items in the vector has been cleared out.
    assert!((*ob).is_empty());

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn push_front() {
    let mut ob = ObservableVector::<char>::new();
    let (values, mut sub) = ob.subscribe().sort_by(rev_cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    // Push front on an empty vector.
    ob.push_front('b');
    assert_next_eq!(sub, VectorDiff::PushFront { value: 'b' });

    // Push front on non-empty vector.
    // The new item should appear at the end.
    ob.push_front('a');
    assert_next_eq!(sub, VectorDiff::PushBack { value: 'a' });

    // Another push front.
    // The new item should appear at the beginning.
    ob.push_front('d');
    assert_next_eq!(sub, VectorDiff::PushFront { value: 'd' });

    // Another push front.
    // The new item should appear in the middle.
    ob.push_front('c');
    assert_next_eq!(sub, VectorDiff::Insert { index: 1, value: 'c' });

    // Items in the vector have been pushed front and are not sorted.
    assert_eq!(*ob, vector!['c', 'd', 'a', 'b']);

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn push_back() {
    let mut ob = ObservableVector::<char>::new();
    let (values, mut sub) = ob.subscribe().sort_by(rev_cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    // Push back on an empty vector.
    ob.push_back('b');
    assert_next_eq!(sub, VectorDiff::PushFront { value: 'b' });

    // Push back on non-empty vector.
    // The new item should appear at the end.
    ob.push_back('a');
    assert_next_eq!(sub, VectorDiff::PushBack { value: 'a' });

    // Another push back.
    // The new item should appear at the beginning.
    ob.push_back('d');
    assert_next_eq!(sub, VectorDiff::PushFront { value: 'd' });

    // Another push back.
    // The new item should appear in the middle.
    ob.push_back('c');
    assert_next_eq!(sub, VectorDiff::Insert { index: 1, value: 'c' });

    // Items in the vector have been pushed back and are not sorted.
    assert_eq!(*ob, vector!['b', 'a', 'd', 'c']);

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn insert() {
    let mut ob = ObservableVector::<char>::new();
    let (values, mut sub) = ob.subscribe().sort_by(rev_cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    // Insert on an empty vector.
    ob.insert(0, 'b');
    assert_next_eq!(sub, VectorDiff::PushFront { value: 'b' });

    // Insert on non-empty vector.
    // The new item should appear at the end.
    ob.insert(1, 'a');
    assert_next_eq!(sub, VectorDiff::PushBack { value: 'a' });

    // Another insert.
    // The new item should appear at the beginning.
    ob.insert(1, 'd');
    assert_next_eq!(sub, VectorDiff::PushFront { value: 'd' });

    // Another insert.
    // The new item should appear at the beginning.
    ob.insert(1, 'e');
    assert_next_eq!(sub, VectorDiff::PushFront { value: 'e' });

    // Another insert.
    // The new item should appear in the middle.
    ob.insert(3, 'c');
    assert_next_eq!(sub, VectorDiff::Insert { index: 2, value: 'c' });

    // Items in the vector have been inserted and are not sorted.
    assert_eq!(*ob, vector!['b', 'e', 'd', 'c', 'a']);

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn pop_front() {
    let mut ob = ObservableVector::<char>::new();
    let (values, mut sub) = ob.subscribe().sort_by(rev_cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    // Append a bunch of items.
    ob.append(vector!['e', 'b', 'a', 'd', 'c']);
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['e', 'd', 'c', 'b', 'a'] });

    // Pop front once.
    assert_eq!(ob.pop_front(), Some('e'));
    assert_next_eq!(sub, VectorDiff::PopFront);

    // Pop front again.
    assert_eq!(ob.pop_front(), Some('b'));
    assert_next_eq!(sub, VectorDiff::Remove { index: 2 });

    // Pop front again.
    assert_eq!(ob.pop_front(), Some('a'));
    assert_next_eq!(sub, VectorDiff::PopBack);

    // Pop front again.
    assert_eq!(ob.pop_front(), Some('d'));
    assert_next_eq!(sub, VectorDiff::PopFront);

    // Pop front again.
    assert_eq!(ob.pop_front(), Some('c'));
    assert_next_eq!(sub, VectorDiff::PopFront);

    assert!(ob.is_empty());

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn pop_back() {
    let mut ob = ObservableVector::<char>::new();
    let (values, mut sub) = ob.subscribe().sort_by(rev_cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    // Append a bunch of items.
    ob.append(vector!['e', 'b', 'a', 'd', 'c', 'f']);
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['f', 'e', 'd', 'c', 'b', 'a'] });

    // Pop back once.
    // `f` is at the first sorted position, so it generates a
    // `VectorDiff::PopFront`.
    assert_eq!(ob.pop_back(), Some('f'));
    assert_next_eq!(sub, VectorDiff::PopFront);

    // Pop back again.
    assert_eq!(ob.pop_back(), Some('c'));
    assert_next_eq!(sub, VectorDiff::Remove { index: 2 });

    // Pop back again.
    assert_eq!(ob.pop_back(), Some('d'));
    assert_next_eq!(sub, VectorDiff::Remove { index: 1 });

    // Pop back again.
    assert_eq!(ob.pop_back(), Some('a'));
    assert_next_eq!(sub, VectorDiff::PopBack);

    // Pop back again.
    assert_eq!(ob.pop_back(), Some('b'));
    assert_next_eq!(sub, VectorDiff::PopBack);

    // Pop back again.
    assert_eq!(ob.pop_back(), Some('e'));
    assert_next_eq!(sub, VectorDiff::PopFront);

    assert!(ob.is_empty());

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn remove() {
    let mut ob = ObservableVector::<char>::new();
    let (values, mut sub) = ob.subscribe().sort_by(rev_cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    // Append a bunch of items.
    ob.append(vector!['e', 'b', 'a', 'd', 'c']);
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['e', 'd', 'c', 'b', 'a'] });

    // Remove `a`.
    ob.remove(2);
    assert_next_eq!(sub, VectorDiff::PopBack);

    // Remove `e`.
    ob.remove(0);
    assert_next_eq!(sub, VectorDiff::PopFront);

    // Remove `c`.
    ob.remove(2);
    assert_next_eq!(sub, VectorDiff::Remove { index: 1 });

    // Items in the vector have been removed and are not sorted.
    assert_eq!(*ob, vector!['b', 'd']);

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn set() {
    let mut ob = ObservableVector::<char>::new();
    let (values, mut sub) = ob.subscribe().sort_by(rev_cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    // Append a bunch of items.
    ob.append(vector!['d', 'e', 'b', 'g']);
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['g', 'e', 'd', 'b'] });

    // Same value.
    ob.set(0, 'd');
    assert_next_eq!(sub, VectorDiff::Set { index: 2, value: 'd' });

    // Different value but position stays the same.
    ob.set(0, 'c');
    assert_next_eq!(sub, VectorDiff::Set { index: 2, value: 'c' });

    // This time sorting moves the value.
    // Another value, that is moved to the right.
    ob.set(0, 'f');
    assert_next_eq!(sub, VectorDiff::Remove { index: 2 });
    assert_next_eq!(sub, VectorDiff::Insert { index: 1, value: 'f' });

    // Same operation, at another index, just for fun.
    ob.set(2, 'f');
    assert_next_eq!(sub, VectorDiff::Remove { index: 3 });
    assert_next_eq!(sub, VectorDiff::Insert { index: 1, value: 'f' });

    // Items in the vector have been updated and are not sorted.
    assert_eq!(*ob, vector!['f', 'e', 'f', 'g']);

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn truncate() {
    let mut ob = ObservableVector::<char>::new();
    let (values, mut sub) = ob.subscribe().sort_by(rev_cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    // Append a bunch of items.
    ob.append(vector!['c', 'd', 'a']);
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['d', 'c', 'a'] });

    // Append other items.
    ob.append(vector!['b', 'e', 'f']);
    assert_next_eq!(sub, VectorDiff::PushFront { value: 'f' });
    assert_next_eq!(sub, VectorDiff::Insert { index: 1, value: 'e' });
    assert_next_eq!(sub, VectorDiff::Insert { index: 4, value: 'b' });

    // Truncate.
    ob.truncate(2);
    assert_next_eq!(sub, VectorDiff::Truncate { length: 2 });

    // Items in the vector have been truncated and are not sorted.
    assert_eq!(*ob, vector!['c', 'd']);

    // Append other items.
    ob.append(vector!['b', 'x', 'y']);
    assert_next_eq!(sub, VectorDiff::PushFront { value: 'y' });
    assert_next_eq!(sub, VectorDiff::Insert { index: 1, value: 'x' });
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['b'] });

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn reset() {
    let mut ob = ObservableVector::<char>::with_capacity(1);
    let (values, mut sub) = ob.subscribe().sort_by(rev_cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    // Append a bunch of items.
    ob.append(vector!['c', 'd', 'a']);
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['d', 'c', 'a'] });

    // Push back a bunch of items 3 times, so that it overflows the capacity, and we
    // get a reset!
    ob.push_back('b');
    ob.push_back('f');
    assert_next_eq!(sub, VectorDiff::Reset { values: vector!['f', 'd', 'c', 'b', 'a'] });

    // Items in the vector have been inserted and are not sorted.
    assert_eq!(*ob, vector!['c', 'd', 'a', 'b', 'f']);

    drop(ob);
    assert_closed!(sub);
}
