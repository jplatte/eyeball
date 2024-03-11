use eyeball_im::{ObservableVector, VectorDiff};
use eyeball_im_util::vector::VectorObserverExt;
use imbl::vector;
use std::cmp::Ordering;
use stream_assert::{assert_closed, assert_next_eq, assert_pending};

fn cmp<T>(left: &T, right: &T) -> Ordering
where
    T: Ord,
{
    left.cmp(right)
}

#[test]
fn new() {
    let ob = ObservableVector::<char>::from(vector!['c', 'a', 'd', 'b']);
    let (values, mut sub) = ob.subscribe().sort_by(&cmp);

    assert_eq!(values, vector!['a', 'b', 'c', 'd']);
    assert_pending!(sub);

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn append() {
    let mut ob = ObservableVector::<char>::new();
    let (values, mut sub) = ob.subscribe().sort_by(&cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    // Append on an empty vector.
    ob.append(vector!['d', 'a', 'e']);
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['a', 'd', 'e'] });

    // Append on an non-empty vector.
    ob.append(vector!['f', 'g', 'b']);
    assert_next_eq!(sub, VectorDiff::Insert { index: 1, value: 'b' });
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['f', 'g'] });

    // Another append.
    // This time, it contains a duplicated new item + an insert + new items to be
    // appended.
    ob.append(vector!['i', 'h', 'c', 'j', 'a']);
    assert_next_eq!(sub, VectorDiff::PushFront { value: 'a' });
    assert_next_eq!(sub, VectorDiff::Insert { index: 3, value: 'c' });
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['h', 'i', 'j'] });

    // Another append.
    // This time, with two new items that are a duplication of the last item.
    ob.append(vector!['k', 'l', 'j', 'm', 'j']);
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['j', 'j', 'k', 'l', 'm'] });

    // Items in the vector have been appended and are not sorted.
    assert_eq!(
        *ob,
        vector!['d', 'a', 'e', 'f', 'g', 'b', 'i', 'h', 'c', 'j', 'a', 'k', 'l', 'j', 'm', 'j']
    );

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn clear() {
    let mut ob = ObservableVector::<char>::new();
    let (values, mut sub) = ob.subscribe().sort_by(&cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    ob.append(vector!['b', 'a', 'c']);
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['a', 'b', 'c'] });

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
    let (values, mut sub) = ob.subscribe().sort_by(&cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    // Push front on an empty vector.
    ob.push_front('b');
    assert_next_eq!(sub, VectorDiff::PushFront { value: 'b' });

    // Push front on non-empty vector.
    // The new item should appear at position 0.
    ob.push_front('a');
    assert_next_eq!(sub, VectorDiff::PushFront { value: 'a' });

    // Another push front.
    // The new item should appear at last position.
    ob.push_front('d');
    assert_next_eq!(sub, VectorDiff::PushBack { value: 'd' });

    // Another push front.
    // The new item should appear in the middle.
    ob.push_front('c');
    assert_next_eq!(sub, VectorDiff::Insert { index: 2, value: 'c' });

    // Items in the vector have been pushed front and are not sorted.
    assert_eq!(*ob, vector!['c', 'd', 'a', 'b']);

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn push_back() {
    let mut ob = ObservableVector::<char>::new();
    let (values, mut sub) = ob.subscribe().sort_by(&cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    // Push back on an empty vector.
    ob.push_back('b');
    assert_next_eq!(sub, VectorDiff::PushFront { value: 'b' });

    // Push back on non-empty vector.
    // The new item should appear at position 0.
    ob.push_back('a');
    assert_next_eq!(sub, VectorDiff::PushFront { value: 'a' });

    // Another push back.
    // The new item should appear at last position.
    ob.push_back('d');
    assert_next_eq!(sub, VectorDiff::PushBack { value: 'd' });

    // Another push back.
    // The new item should appear in the middle.
    ob.push_back('c');
    assert_next_eq!(sub, VectorDiff::Insert { index: 2, value: 'c' });

    // Items in the vector have been pushed back and are not sorted.
    assert_eq!(*ob, vector!['b', 'a', 'd', 'c']);

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn insert() {
    let mut ob = ObservableVector::<char>::new();
    let (values, mut sub) = ob.subscribe().sort_by(&cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    // Insert on an empty vector.
    ob.insert(0, 'b');
    assert_next_eq!(sub, VectorDiff::PushFront { value: 'b' });

    // Insert on non-empty vector.
    // The new item should appear at position 0.
    ob.insert(1, 'a');
    assert_next_eq!(sub, VectorDiff::PushFront { value: 'a' });

    // Another insert.
    // The new item should appear at last position.
    ob.insert(1, 'd');
    assert_next_eq!(sub, VectorDiff::PushBack { value: 'd' });

    // Another insert.
    // The new item should appear at last position.
    ob.insert(1, 'e');
    assert_next_eq!(sub, VectorDiff::PushBack { value: 'e' });

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
    let (values, mut sub) = ob.subscribe().sort_by(&cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    // Append a bunch of items.
    ob.append(vector!['e', 'b', 'a', 'd', 'c']);
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['a', 'b', 'c', 'd', 'e'] });

    // Pop front once.
    // `e` is at the last sorted position, so it generates a `VectorDiff::PopBack`.
    assert_eq!(ob.pop_front(), Some('e'));
    assert_next_eq!(sub, VectorDiff::PopBack);

    // Pop front again.
    // `b` is at the second sorted position, so it generates a `VectorDiff::Remove`.
    assert_eq!(ob.pop_front(), Some('b'));
    assert_next_eq!(sub, VectorDiff::Remove { index: 1 });

    // Pop front again.
    // `a` is at the first sorted position, so it generates a
    // `VectorDiff::PopFront`.
    assert_eq!(ob.pop_front(), Some('a'));
    assert_next_eq!(sub, VectorDiff::PopFront);

    // Pop front again.
    // `d` is at the last sorted position, so it generates a `VectorDiff::PopBack`.
    assert_eq!(ob.pop_front(), Some('d'));
    assert_next_eq!(sub, VectorDiff::PopBack);

    // Pop front again.
    // `c` is at the first sorted position, so it generates a
    // `VectorDiff::PopFront`.
    assert_eq!(ob.pop_front(), Some('c'));
    assert_next_eq!(sub, VectorDiff::PopFront);

    assert!(ob.is_empty());

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn pop_back() {
    let mut ob = ObservableVector::<char>::new();
    let (values, mut sub) = ob.subscribe().sort_by(&cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    // Append a bunch of items.
    ob.append(vector!['e', 'b', 'a', 'd', 'c', 'f']);
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['a', 'b', 'c', 'd', 'e', 'f'] });

    // Pop back once.
    // `f` is at the last sorted position, so it generates a `VectorDiff::PopBack`.
    assert_eq!(ob.pop_back(), Some('f'));
    assert_next_eq!(sub, VectorDiff::PopBack);

    // Pop back again.
    // `c` is at the third sorted position, so it generates a `VectorDiff::Remove`.
    assert_eq!(ob.pop_back(), Some('c'));
    assert_next_eq!(sub, VectorDiff::Remove { index: 2 });

    // Pop back again.
    // `d` is at the third sorted position, so it generates a `VectorDiff::Remove`.
    assert_eq!(ob.pop_back(), Some('d'));
    assert_next_eq!(sub, VectorDiff::Remove { index: 2 });

    // Pop back again.
    // `a` is at the first sorted position, so it generates a
    // `VectorDiff::PopFront`.
    assert_eq!(ob.pop_back(), Some('a'));
    assert_next_eq!(sub, VectorDiff::PopFront);

    // Pop back again.
    // `b` is at the first sorted position, so it generates a
    // `VectorDiff::PopFront`.
    assert_eq!(ob.pop_back(), Some('b'));
    assert_next_eq!(sub, VectorDiff::PopFront);

    // Pop back again.
    // `e` is at the first sorted position, so it generates a
    // `VectorDiff::PopFront`.
    assert_eq!(ob.pop_back(), Some('e'));
    assert_next_eq!(sub, VectorDiff::PopFront);

    assert!(ob.is_empty());

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn remove() {
    let mut ob = ObservableVector::<char>::new();
    let (values, mut sub) = ob.subscribe().sort_by(&cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    // Append a bunch of items.
    ob.append(vector!['e', 'b', 'a', 'd', 'c']);
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['a', 'b', 'c', 'd', 'e'] });

    // Remove `a`.
    ob.remove(2);
    assert_next_eq!(sub, VectorDiff::PopFront);

    // Remove `e`.
    ob.remove(0);
    assert_next_eq!(sub, VectorDiff::PopBack);

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
    let (values, mut sub) = ob.subscribe().sort_by(&cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    // Append a bunch of items.
    ob.append(vector!['d', 'e', 'b', 'g']);
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['b', 'd', 'e', 'g'] });

    // Same value.
    ob.set(0, 'd');
    assert_next_eq!(sub, VectorDiff::Set { index: 1, value: 'd' });

    // Another value, that is sorted at the same sorted index: `d` is at the sorted
    // index 1, and `c` is at the sorted index 1 too. The `VectorDiff::Remove` +
    // `VectorDiff::Insert` are optimised as a single `VectorDiff::Set`.
    ob.set(0, 'c');
    assert_next_eq!(sub, VectorDiff::Set { index: 1, value: 'c' });

    // Another value, that is sorted at an adjacent sorted index: `c` is at the
    // sorted index 1, but `d` is at the sorted index 2. The `VectorDiff::Remove` +
    // `VectorDiff::Insert` are optimised as a single `VectorDiff::Set`.
    ob.set(0, 'd');
    assert_next_eq!(sub, VectorDiff::Set { index: 1, value: 'd' });

    // Another value, that is moved to the left.
    ob.set(0, 'a');
    assert_next_eq!(sub, VectorDiff::Remove { index: 1 });
    assert_next_eq!(sub, VectorDiff::Insert { index: 0, value: 'a' });

    // Another value, that is moved to the right.
    ob.set(0, 'f');
    assert_next_eq!(sub, VectorDiff::Remove { index: 0 });
    assert_next_eq!(sub, VectorDiff::Insert { index: 2, value: 'f' });

    // Another value, that is moved to the right-most position.
    ob.set(0, 'h');
    assert_next_eq!(sub, VectorDiff::Remove { index: 2 });
    assert_next_eq!(sub, VectorDiff::Insert { index: 3, value: 'h' });

    // Same operation, at another index, just for fun.
    ob.set(2, 'f');
    assert_next_eq!(sub, VectorDiff::Remove { index: 0 });
    assert_next_eq!(sub, VectorDiff::Insert { index: 1, value: 'f' });

    // Items in the vector have been updated and are not sorted.
    assert_eq!(*ob, vector!['h', 'e', 'f', 'g']);

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn truncate() {
    let mut ob = ObservableVector::<char>::new();
    let (values, mut sub) = ob.subscribe().sort_by(&cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    // Append a bunch of items.
    ob.append(vector!['c', 'd', 'a']);
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['a', 'c', 'd'] });

    // Append other items.
    ob.append(vector!['b', 'e', 'f']);
    assert_next_eq!(sub, VectorDiff::Insert { index: 1, value: 'b' });
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['e', 'f'] });

    // Truncate.
    ob.truncate(2);
    assert_next_eq!(sub, VectorDiff::Truncate { length: 2 });

    // Items in the vector have been truncated and are not sorted.
    assert_eq!(*ob, vector!['c', 'd']);

    // Append other items.
    ob.append(vector!['b', 'x', 'y']);
    assert_next_eq!(sub, VectorDiff::PushFront { value: 'b' });
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['x', 'y'] });

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn reset() {
    let mut ob = ObservableVector::<char>::with_capacity(1);
    let (values, mut sub) = ob.subscribe().sort_by(&cmp);

    assert!(values.is_empty());
    assert_pending!(sub);

    // Append a bunch of items.
    ob.append(vector!['c', 'd', 'a']);
    assert_next_eq!(sub, VectorDiff::Append { values: vector!['a', 'c', 'd'] });

    // Push back a bunch of items 3 times, so that it overflows the capacity, and we
    // get a reset!
    ob.push_back('b');
    ob.push_back('f');
    assert_next_eq!(sub, VectorDiff::Reset { values: vector!['a', 'b', 'c', 'd', 'f'] });

    // Items in the vector have been inserted  and are not sorted.
    assert_eq!(*ob, vector!['c', 'd', 'a', 'b', 'f']);

    drop(ob);
    assert_closed!(sub);
}
