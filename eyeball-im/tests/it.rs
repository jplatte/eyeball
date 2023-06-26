use imbl::{vector, Vector};
use stream_assert::{assert_next_eq, assert_pending};

use eyeball_im::{ObservableVector, VectorDiff};

#[test]
fn lag() {
    let mut ob = ObservableVector::with_capacity(1);
    let mut rx1 = ob.subscribe();
    let mut rx2 = ob.subscribe();

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
    let mut sub = ob.subscribe();

    ob.push_back(0);
    ob.append(vector![1, 2]);
    ob.push_back(3);

    // Reset takes us immediately to the latest state, no updates afterwards
    // without modifying the vector again.
    assert_next_eq!(sub, VectorDiff::Reset { values: vector![0, 1, 2, 3] });
    assert_pending!(sub);
}
