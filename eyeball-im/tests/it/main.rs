use imbl::{vector, Vector};
use stream_assert::{assert_next_eq, assert_pending};

use eyeball_im::{ObservableVector, ObservableVectorEntry, VectorDiff};

mod entry;

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
