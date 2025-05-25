#![allow(missing_docs)]

use imbl::{vector, Vector};
use stream_assert::{assert_closed, assert_next_eq, assert_pending};

use eyeball_im::{ObservableVector, ObservableVectorEntry, VectorDiff};

mod apply;
mod batch;
mod entry;
mod panic;
#[cfg(feature = "serde")]
mod serde;

#[test]
fn lag() {
    let mut ob = ObservableVector::with_capacity(1);
    let mut rx1 = ob.subscribe().into_stream();
    let mut rx2 = ob.subscribe().into_stream();

    ob.push_back("hello".to_owned());
    assert_next_eq!(rx1, VectorDiff::PushBack { value: "hello".to_owned() });

    ob.push_back("world".to_owned());
    assert_next_eq!(rx1, VectorDiff::PushBack { value: "world".to_owned() });
    assert_next_eq!(
        rx2,
        VectorDiff::Reset { values: Vector::from_iter(["hello".to_owned(), "world".to_owned()]) }
    );
}

#[test]
fn lag2() {
    let mut ob: ObservableVector<i32> = ObservableVector::with_capacity(2);
    let mut sub = ob.subscribe().into_stream();

    ob.push_back(0);
    ob.append(vector![1, 2]);
    ob.push_back(3);

    // Reset takes us immediately to the latest state, no updates afterwards
    // without modifying the vector again.
    assert_next_eq!(sub, VectorDiff::Reset { values: vector![0, 1, 2, 3] });
    assert_pending!(sub);
}

#[test]
fn truncate() {
    let mut ob: ObservableVector<i32> = ObservableVector::from(vector![1, 2]);
    let mut sub = ob.subscribe().into_stream();

    ob.truncate(3);
    ob.truncate(2);
    assert_pending!(sub);
    assert_eq!(*ob, vector![1, 2]);

    ob.truncate(1);
    assert_next_eq!(sub, VectorDiff::Truncate { length: 1 });
    assert_eq!(*ob, vector![1]);

    ob.truncate(0);
    assert_next_eq!(sub, VectorDiff::Truncate { length: 0 });
    assert!(ob.is_empty());
}

#[test]
fn clear() {
    let mut ob: ObservableVector<i32> = ObservableVector::from(vector![1, 2]);
    let mut sub = ob.subscribe().into_stream();
    assert_pending!(sub);

    ob.clear();
    assert_next_eq!(sub, VectorDiff::Clear);
    assert!(ob.is_empty());

    // Clearing again. The vector is empty now. We don't expect a
    // `VectorDiff::Clear`.
    ob.clear();
    assert_pending!(sub);

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn for_each() {
    let mut ob: ObservableVector<i32> = ObservableVector::from(vector![0, 10, 1, 2, 4, 33, 5]);
    let mut sub = ob.subscribe().into_stream();
    let mut saw_five = false;

    ob.for_each(|mut item| {
        if *item % 2 == 0 {
            let new_value = *item / 2;
            ObservableVectorEntry::set(&mut item, new_value);
            if *item == 0 {
                ObservableVectorEntry::remove(item);
            }
        } else if *item > 10 {
            ObservableVectorEntry::remove(item);
        } else if *item == 5 {
            // only possible because `for_each` accepts FnMut
            saw_five = true;
        }
    });

    assert!(saw_five);
    assert_next_eq!(sub, VectorDiff::Set { index: 0, value: 0 });
    assert_next_eq!(sub, VectorDiff::Remove { index: 0 });
    assert_next_eq!(sub, VectorDiff::Set { index: 0, value: 5 });
    assert_next_eq!(sub, VectorDiff::Set { index: 2, value: 1 });
    assert_next_eq!(sub, VectorDiff::Set { index: 3, value: 2 });
    assert_next_eq!(sub, VectorDiff::Remove { index: 4 });
    assert_pending!(sub);
}

#[test]
fn transaction() {
    let mut ob = ObservableVector::new();
    let mut st = ob.subscribe().into_stream();
    let mut txn = ob.transaction();

    txn.push_back(0);
    assert_pending!(st);

    txn.push_front(-1);
    assert_pending!(st);

    txn.commit();
    assert_next_eq!(st, VectorDiff::PushBack { value: 0 });
    assert_next_eq!(st, VectorDiff::PushFront { value: -1 });
}

#[test]
fn transaction_rollback() {
    let mut ob = ObservableVector::new();
    let mut st = ob.subscribe().into_stream();

    let mut txn = ob.transaction();
    txn.push_back(1);
    drop(txn);

    assert_pending!(st);

    let mut txn = ob.transaction();
    txn.push_back(0);
    txn.rollback();
    txn.insert(0, 123);
    txn.commit();

    assert_next_eq!(st, VectorDiff::Insert { index: 0, value: 123 });
}

#[test]
fn transaction_no_subscribers() {
    let mut ob = ObservableVector::new();
    let mut txn = ob.transaction();
    txn.push_back(0);
    txn.rollback();
    txn.insert(0, 123);
    txn.push_front(45);
    txn.commit();

    assert_eq!(*ob, vector![45, 123]);
}
