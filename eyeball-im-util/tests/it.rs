use eyeball_im::{ObservableVector, VectorDiff};
use eyeball_im_util::VectorExt;
use imbl::vector;
use stream_assert::{assert_closed, assert_next_eq, assert_pending};

#[test]
fn append() {
    let mut ob: ObservableVector<&str> = ObservableVector::new();
    let (_, mut sub) = ob.subscribe_filter(|s| s.len() < 8);

    ob.append(vector!["hello", "world"]);
    assert_next_eq!(sub, VectorDiff::Append { values: vector!["hello", "world"] });

    ob.append(vector!["hello, world!"]);
    assert_pending!(sub);

    ob.append(vector!["goodbye"]);
    assert_next_eq!(sub, VectorDiff::Append { values: vector!["goodbye"] });

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn append_clear() {
    let mut ob: ObservableVector<i32> = ObservableVector::new();
    let (_, mut sub) = ob.subscribe_filter(|&i| i < 256);

    ob.append(vector![1024]);
    assert_pending!(sub);

    ob.append(vector![1, 2, 3]);
    assert_next_eq!(sub, VectorDiff::Append { values: vector![1, 2, 3] });

    ob.clear();
    assert_next_eq!(sub, VectorDiff::Clear);

    ob.append(vector![999, 256, 1234]);
    assert_pending!(sub);

    ob.append(vector![255, 127]);
    assert_next_eq!(sub, VectorDiff::Append { values: vector![255, 127] });
    assert_pending!(sub);
}

#[test]
fn buffering() {
    let mut ob: ObservableVector<i32> = ObservableVector::new();
    let (_, mut sub) = ob.subscribe_filter(|&i| i < 256);

    ob.append(vector![1024]);
    ob.append(vector![1, 2, 3]);
    ob.clear();
    ob.append(vector![255, 127]);
    ob.append(vector![999, 256, 1234]);

    assert_next_eq!(sub, VectorDiff::Append { values: vector![1, 2, 3] });
    assert_next_eq!(sub, VectorDiff::Clear);
    assert_next_eq!(sub, VectorDiff::Append { values: vector![255, 127] });
    assert_pending!(sub);
}

#[test]
fn push_front() {
    let mut ob: ObservableVector<i32> = ObservableVector::new();
    let (_, mut sub) = ob.subscribe_filter(|&i| i < 256);

    ob.push_front(1);
    assert_next_eq!(sub, VectorDiff::PushFront { value: 1 });
    ob.push_front(1024);
    assert_pending!(sub);
    ob.push_front(2);
    assert_next_eq!(sub, VectorDiff::PushFront { value: 2 });

    ob.clear();
    ob.push_front(256);
    assert_next_eq!(sub, VectorDiff::Clear);
    assert_pending!(sub);
    ob.push_front(55);
    assert_next_eq!(sub, VectorDiff::PushFront { value: 55 });
}

#[test]
fn push_back() {
    let mut ob: ObservableVector<i32> = ObservableVector::new();
    let (_, mut sub) = ob.subscribe_filter(|&i| i < 256);

    ob.push_back(1);
    assert_next_eq!(sub, VectorDiff::PushBack { value: 1 });
    ob.push_back(1024);
    assert_pending!(sub);
    ob.push_back(2);
    assert_next_eq!(sub, VectorDiff::PushBack { value: 2 });

    ob.clear();
    ob.push_back(256);
    ob.push_back(55);
    assert_next_eq!(sub, VectorDiff::Clear);
    assert_next_eq!(sub, VectorDiff::PushBack { value: 55 });
}

#[test]
fn push_both() {
    let mut ob: ObservableVector<i32> = ObservableVector::new();
    let (_, mut sub) = ob.subscribe_filter(|&i| i < 256);

    ob.push_back(1);
    ob.push_front(2);
    assert_next_eq!(sub, VectorDiff::PushBack { value: 1 });
    assert_next_eq!(sub, VectorDiff::PushFront { value: 2 });

    ob.push_front(777);
    ob.push_front(511);
    assert_pending!(sub);

    ob.push_front(4);
    ob.push_front(511);
    ob.push_back(123);
    assert_next_eq!(sub, VectorDiff::PushFront { value: 4 });
    assert_next_eq!(sub, VectorDiff::PushBack { value: 123 });
}

#[test]
fn pop_front() {
    let mut ob: ObservableVector<i32> = ObservableVector::from(vector![1, 2, 3]);
    let (items, mut sub) = ob.subscribe_filter(|&i| i < 256);
    assert_eq!(items, vector![1, 2, 3]);

    ob.pop_front();
    assert_next_eq!(sub, VectorDiff::PopFront);
    ob.pop_front();
    ob.pop_front();
    assert_next_eq!(sub, VectorDiff::PopFront);
    assert_next_eq!(sub, VectorDiff::PopFront);
    assert_pending!(sub);

    let mut ob: ObservableVector<i32> = ObservableVector::from(vector![1000, 2, 3000, 4]);
    let (items, mut sub) = ob.subscribe_filter(|&i| i < 256);
    assert_eq!(items, vector![2, 4]);

    ob.pop_front();
    assert_pending!(sub);
    ob.pop_front();
    assert_next_eq!(sub, VectorDiff::PopFront);
    ob.pop_front();
    ob.pop_front();
    assert_next_eq!(sub, VectorDiff::PopFront);
    assert_pending!(sub);
}

#[test]
fn pop_back() {
    let mut ob: ObservableVector<i32> = ObservableVector::from(vector![1, 2, 3]);
    let (items, mut sub) = ob.subscribe_filter(|&i| i < 256);
    assert_eq!(items, vector![1, 2, 3]);

    ob.pop_back();
    assert_next_eq!(sub, VectorDiff::PopBack);
    ob.pop_back();
    ob.pop_back();
    assert_next_eq!(sub, VectorDiff::PopBack);
    assert_next_eq!(sub, VectorDiff::PopBack);
    assert_pending!(sub);

    let mut ob: ObservableVector<i32> = ObservableVector::from(vector![1000, 2, 3000, 4]);
    let (items, mut sub) = ob.subscribe_filter(|&i| i < 256);
    assert_eq!(items, vector![2, 4]);

    ob.pop_back();
    assert_next_eq!(sub, VectorDiff::PopBack);
    ob.pop_back();
    assert_pending!(sub);
    ob.pop_back();
    ob.pop_back();
    assert_next_eq!(sub, VectorDiff::PopBack);
    assert_pending!(sub);
}

#[test]
fn insert() {
    let mut ob: ObservableVector<i32> = ObservableVector::new();
    let (_, mut sub) = ob.subscribe_filter(|&i| i < 256);

    ob.insert(0, 256);
    assert_pending!(sub);
    ob.insert(1, 123);
    assert_next_eq!(sub, VectorDiff::Insert { index: 0, value: 123 });
    ob.insert(0, 234);
    assert_next_eq!(sub, VectorDiff::Insert { index: 0, value: 234 });
    ob.insert(1, 1000);
    ob.insert(2, 255);
    assert_eq!(*ob, vector![234, 1000, 255, 256, 123]);
    assert_next_eq!(sub, VectorDiff::Insert { index: 1, value: 255 });
    assert_pending!(sub);

    let mut ob: ObservableVector<i32> =
        ObservableVector::from(vector![1, 2000, 3000, 4000, 5000, 6]);
    let (_, mut sub) = ob.subscribe_filter(|&i| i < 256);
    ob.insert(3, 30);
    assert_next_eq!(sub, VectorDiff::Insert { index: 1, value: 30 });
    assert_pending!(sub);
    ob.insert(4, 400);
    assert_pending!(sub);
    assert_eq!(*ob, vector![1, 2000, 3000, 30, 400, 4000, 5000, 6]);
    ob.insert(8, 80);
    assert_next_eq!(sub, VectorDiff::Insert { index: 3, value: 80 });
    assert_pending!(sub);
}

#[test]
fn set() {
    let mut ob: ObservableVector<i32> =
        ObservableVector::from(vector![0, 1000, 2000, 3000, 4, 5000]);
    let (_, mut sub) = ob.subscribe_filter(|&i| i < 256);

    ob.set(2, 200);
    assert_next_eq!(sub, VectorDiff::Insert { index: 1, value: 200 });
    ob.set(2, 20);
    assert_next_eq!(sub, VectorDiff::Set { index: 1, value: 20 });
    ob.set(0, 255);
    assert_next_eq!(sub, VectorDiff::Set { index: 0, value: 255 });
    ob.set(4, 4000);
    assert_next_eq!(sub, VectorDiff::Remove { index: 2 });
    ob.set(5, 50);
    assert_next_eq!(sub, VectorDiff::Insert { index: 2, value: 50 });
}

#[test]
fn remove() {
    let mut ob: ObservableVector<i32> =
        ObservableVector::from(vector![0, 1, 2000, 3000, 4000, 5, 6000]);
    let (_, mut sub) = ob.subscribe_filter(|&i| i < 256);

    ob.remove(3); // 3000
    assert_pending!(sub);
    ob.remove(3); // 4000
    assert_pending!(sub);
    ob.remove(3); // 5
    assert_next_eq!(sub, VectorDiff::Remove { index: 2 });
    ob.remove(0); // 0
    assert_next_eq!(sub, VectorDiff::Remove { index: 0 });
    ob.remove(2); // 6000
    assert_pending!(sub);
    ob.remove(1); // 2000
    assert_pending!(sub);
    ob.remove(0); // 2000
    assert_next_eq!(sub, VectorDiff::Remove { index: 0 });
}

#[test]
fn reset() {
    let mut ob: ObservableVector<i32> = ObservableVector::with_capacity(1);
    let (_, mut sub) = ob.subscribe_filter(|&i| i < 256);

    ob.push_front(0);
    ob.append(vector![1000, 2, 3000, 4]);
    assert_next_eq!(sub, VectorDiff::Reset { values: vector![0, 2, 4] });
    ob.remove(2);
    assert_next_eq!(sub, VectorDiff::Remove { index: 1 });
    ob.remove(1);
    assert_pending!(sub);

    ob.pop_front();
    ob.insert(2, 5);
    assert_next_eq!(sub, VectorDiff::Reset { values: vector![4, 5] });
    ob.remove(2);
    assert_next_eq!(sub, VectorDiff::Remove { index: 1 });
    ob.remove(1);
    assert_next_eq!(sub, VectorDiff::Remove { index: 0 });
    ob.remove(0);
    assert_pending!(sub);
}
