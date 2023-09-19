use eyeball::Observable;
use eyeball_im::{ObservableVector, VectorDiff};
use eyeball_im_util::vector::VectorObserverExt;
use imbl::vector;
use stream_assert::{assert_closed, assert_next_eq, assert_pending};

#[test]
fn static_limit() {
    let mut ob: ObservableVector<usize> = ObservableVector::from(vector![1, 20, 300]);
    let (limited, mut sub) = ob.subscribe().limit(2);
    assert_eq!(limited, vector![1, 20]);
    assert_pending!(sub);

    ob.pop_back();
    assert_pending!(sub);

    ob.pop_back();
    assert_next_eq!(sub, VectorDiff::PopBack);
}

#[test]
fn pending_until_limit_emits_a_value() {
    let mut ob: ObservableVector<usize> = ObservableVector::new();
    let mut limit = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_limit(Observable::subscribe(&limit));

    // Append new values…
    ob.append(vector![10, 11, 12]);

    // … but it's still pending…
    assert_pending!(sub);

    // … because the `limit` stream didn't produce any value yet.
    // Let's change that.
    Observable::set(&mut limit, 7);

    // Here we are.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![10, 11, 12] });

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn increase_and_decrease_the_limit_on_an_empty_stream() {
    let ob = ObservableVector::<usize>::new();
    let mut limit = Observable::new(1);
    let (limited, mut sub) =
        ob.subscribe().dynamic_limit_with_initial_value(1, Observable::subscribe(&limit));

    assert!(limited.is_empty());

    // `ob` is empty!

    // Set limit to 2.
    Observable::set(&mut limit, 2);

    // Observe nothing.
    assert_pending!(sub);

    // Set limit to 1.
    Observable::set(&mut limit, 1);

    // Observe nothing.
    assert_pending!(sub);

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn increase_and_decrease_the_limit_only() {
    let mut ob = ObservableVector::<usize>::new();
    let mut limit = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_limit(Observable::subscribe(&limit));

    // Append 4 values.
    ob.append(vector![10, 11, 12, 13]);

    // Set limit to 2.
    Observable::set(&mut limit, 2);

    // Observe 2 values.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![10, 11] });

    // Set limit to 4.
    Observable::set(&mut limit, 4);

    // Observe 2 more values.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![12, 13] });

    // Set limit to 6.
    Observable::set(&mut limit, 6);

    // Observe nothing.
    assert_pending!(sub);

    // Set limit to 5.
    Observable::set(&mut limit, 5);

    // Observe nothing.
    assert_pending!(sub);

    // Set limit to 1.
    Observable::set(&mut limit, 1);

    // Observe a truncation.
    assert_next_eq!(sub, VectorDiff::Truncate { length: 1 });

    // Set limit to 0.
    Observable::set(&mut limit, 0);

    // Observe another truncation.
    assert_next_eq!(sub, VectorDiff::Truncate { length: 0 });

    // Set limit to 5.
    Observable::set(&mut limit, 5);

    // Observe all available values.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![10, 11, 12, 13] });

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn limit_is_zero() {
    let mut ob = ObservableVector::<usize>::new();
    let mut limit = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_limit(Observable::subscribe(&limit));

    // Append 4 values.
    ob.append(vector![10, 11, 12, 13]);

    // Set limit to 0.
    Observable::set(&mut limit, 0);

    // Observe nothing.
    assert_pending!(sub);

    // Add 1 value.
    ob.push_back(14);

    // Observe nothing.
    assert_pending!(sub);

    // Set limit to 5.
    Observable::set(&mut limit, 5);

    // Observe 5 values.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![10, 11, 12, 13, 14] });

    // Set limit to 0 again.
    Observable::set(&mut limit, 0);

    // Observe a truncation.
    assert_next_eq!(sub, VectorDiff::Truncate { length: 0 });

    // Add 1 value.
    ob.push_back(15);

    // Observe nothing.
    assert_pending!(sub);

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn limit_is_polled_first() {
    let mut ob = ObservableVector::<usize>::new();
    let mut limit = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_limit(Observable::subscribe(&limit));

    // Append 4 values.
    ob.append(vector![10, 11, 12, 13]);

    // Set limit to 3.
    Observable::set(&mut limit, 3);

    // Observe 3 values.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![10, 11, 12] });

    // Add 1 value on the front…
    ob.push_front(14);

    // … and set limit to 2 _after_.
    Observable::set(&mut limit, 2);

    // However, let's observe the limit's change _first_…
    assert_next_eq!(sub, VectorDiff::Truncate { length: 2 });

    // … and let's observe the other updates _after_.
    assert_next_eq!(sub, VectorDiff::PopBack);
    assert_next_eq!(sub, VectorDiff::PushFront { value: 14 });
    assert_pending!(sub);

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn append() {
    let mut ob = ObservableVector::<usize>::new();
    let mut limit = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_limit(Observable::subscribe(&limit));

    // Set limit to 4.
    Observable::set(&mut limit, 4);

    // Append 3 values.
    ob.append(vector![10, 11, 12]);

    // Observe 3 more values.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![10, 11, 12] });

    // Append 3 more values, whom 2 are outside the limit.
    ob.append(vector![13, 14, 15]);

    // Observe only 1 value.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![13] });

    // Set limit to 8.
    Observable::set(&mut limit, 8);

    // Observe the other 2 values.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![14, 15] });

    // Append 2 more values to reach the limit.
    ob.append(vector![16, 17]);

    // Observe 2 values.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![16, 17] });

    // Append 2 more values, that are outside the limit.
    ob.append(vector![18, 19]);

    // Observe nothing.
    assert_pending!(sub);

    // Check the content of the vector.
    {
        let expected = vector![10, 11, 12, 13, 14, 15, 16, 17, 18, 19];
        assert_eq!(*ob, expected);

        Observable::set(&mut limit, 0);
        assert_next_eq!(sub, VectorDiff::Truncate { length: 0 });

        Observable::set(&mut limit, 42);
        assert_next_eq!(sub, VectorDiff::Append { values: expected });
    }

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn clear() {
    let mut ob = ObservableVector::<usize>::new();
    let mut limit = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_limit(Observable::subscribe(&limit));

    // Set limit to 4.
    Observable::set(&mut limit, 4);

    // Add 1 value.
    ob.push_back(10);

    // Clear.
    ob.clear();

    // Observe the updates.
    assert_next_eq!(sub, VectorDiff::PushBack { value: 10 });
    assert_next_eq!(sub, VectorDiff::Clear);

    // Check the content of the vector.
    {
        let expected = vector![];
        assert_eq!(*ob, expected);
    }

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn push_front() {
    let mut ob = ObservableVector::<usize>::new();
    let mut limit = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_limit(Observable::subscribe(&limit));

    // Set limit to 2.
    Observable::set(&mut limit, 2);

    // Add 1 value.
    ob.push_front(10);

    // Observe 1 value.
    assert_next_eq!(sub, VectorDiff::PushFront { value: 10 });

    // Add 1 value.
    ob.push_front(11);

    // Observe 1 value.
    assert_next_eq!(sub, VectorDiff::PushFront { value: 11 });

    // Add 1 value.
    ob.push_front(12);

    // Observe 1 value being removed, and 1 new value.
    assert_next_eq!(sub, VectorDiff::PopBack);
    assert_next_eq!(sub, VectorDiff::PushFront { value: 12 });

    // Add 1 value.
    ob.push_front(13);

    // Observe 1 value being removed, and 1 new value.
    assert_next_eq!(sub, VectorDiff::PopBack);
    assert_next_eq!(sub, VectorDiff::PushFront { value: 13 });
    assert_pending!(sub);

    // Check the content of the vector.
    {
        let expected = vector![13, 12, 11, 10];
        assert_eq!(*ob, expected);

        Observable::set(&mut limit, 0);
        assert_next_eq!(sub, VectorDiff::Truncate { length: 0 });

        Observable::set(&mut limit, 42);
        assert_next_eq!(sub, VectorDiff::Append { values: expected });
    }

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn push_back() {
    let mut ob = ObservableVector::<usize>::new();
    let mut limit = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_limit(Observable::subscribe(&limit));

    // Set limit to 2.
    Observable::set(&mut limit, 2);

    // Add 1 value.
    ob.push_back(10);

    // Observe 1 value.
    assert_next_eq!(sub, VectorDiff::PushBack { value: 10 });

    // Add 1 value.
    ob.push_back(11);

    // Observe 1 value.
    assert_next_eq!(sub, VectorDiff::PushBack { value: 11 });

    // Add 1 value.
    ob.push_back(12);

    // Observe nothing.
    assert_pending!(sub);

    // Add 1 value.
    ob.push_back(13);

    // Observe nothing.
    assert_pending!(sub);

    // Check the content of the vector.
    {
        let expected = vector![10, 11, 12, 13];
        assert_eq!(*ob, expected);

        Observable::set(&mut limit, 0);
        assert_next_eq!(sub, VectorDiff::Truncate { length: 0 });

        Observable::set(&mut limit, 42);
        assert_next_eq!(sub, VectorDiff::Append { values: expected });
    }

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn pop_front() {
    let mut ob = ObservableVector::<usize>::new();
    let mut limit = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_limit(Observable::subscribe(&limit));

    // Add 4 values.
    ob.append(vector![10, 11, 12, 13]);

    // Set limit to 2.
    Observable::set(&mut limit, 2);

    // Observe 2 values.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![10, 11] });

    // Remove 1 value.
    ob.pop_front();

    // Observe 1 value being removed, and 1 new value being inserted at the back.
    assert_next_eq!(sub, VectorDiff::PopFront);
    assert_next_eq!(sub, VectorDiff::PushBack { value: 12 });

    // Remove 1 value.
    ob.pop_front();

    // Observe 1 value being removed, and 1 new value being inserted at the back.
    assert_next_eq!(sub, VectorDiff::PopFront);
    assert_next_eq!(sub, VectorDiff::PushBack { value: 13 });

    // Remove 1 value.
    ob.pop_front();

    // Observe only 1 value being removed.
    assert_next_eq!(sub, VectorDiff::PopFront);
    assert_pending!(sub);

    // Check the content of the vector.
    {
        let expected = vector![13];
        assert_eq!(*ob, expected);

        Observable::set(&mut limit, 0);
        assert_next_eq!(sub, VectorDiff::Truncate { length: 0 });

        Observable::set(&mut limit, 42);
        assert_next_eq!(sub, VectorDiff::Append { values: expected });
    }

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn pop_back() {
    let mut ob = ObservableVector::<usize>::new();
    let mut limit = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_limit(Observable::subscribe(&limit));

    // Add 4 values.
    ob.append(vector![10, 11, 12, 13]);

    // Set limit to 3.
    Observable::set(&mut limit, 3);

    // Observe 3 values.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![10, 11, 12] });

    // Remove 1 value.
    ob.pop_back();

    // Observe nothing.
    assert_pending!(sub);

    // Remove 1 value.
    ob.pop_back();

    // Observe nothing.
    assert_next_eq!(sub, VectorDiff::PopBack);

    // Remove 1 value.
    ob.pop_back();

    // Observe nothing.
    assert_next_eq!(sub, VectorDiff::PopBack);

    // Check the content of the vector.
    {
        let expected = vector![10];
        assert_eq!(*ob, expected);

        Observable::set(&mut limit, 0);
        assert_next_eq!(sub, VectorDiff::Truncate { length: 0 });

        Observable::set(&mut limit, 42);
        assert_next_eq!(sub, VectorDiff::Append { values: expected });
    }

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn insert() {
    let mut ob = ObservableVector::<usize>::new();
    let mut limit = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_limit(Observable::subscribe(&limit));

    // Add 2 values.
    ob.append(vector![10, 11]);

    // Set limit to 3.
    Observable::set(&mut limit, 3);

    // Observe 2 values.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![10, 11] });

    // Insert 1 value.
    ob.insert(1, 12);

    // Observe 1 value.
    assert_next_eq!(sub, VectorDiff::Insert { index: 1, value: 12 });

    // Insert 1 value.
    ob.insert(1, 13);

    // Observe 1 value being removed, and 1 value being added.
    assert_next_eq!(sub, VectorDiff::PopBack);
    assert_next_eq!(sub, VectorDiff::Insert { index: 1, value: 13 });

    // Insert 1 value at the limit.
    ob.insert(2, 14);

    // Observe 1 value being removed, and 1 value being added.
    assert_next_eq!(sub, VectorDiff::PopBack);
    assert_next_eq!(sub, VectorDiff::Insert { index: 2, value: 14 });

    // Insert 1 value after the limit.
    ob.insert(4, 15);

    // Observe nothing.
    assert_pending!(sub);

    // Check the content of the vector.
    {
        let expected = vector![10, 13, 14, 12, 15, 11];
        assert_eq!(*ob, expected);

        Observable::set(&mut limit, 0);
        assert_next_eq!(sub, VectorDiff::Truncate { length: 0 });

        Observable::set(&mut limit, 42);
        assert_next_eq!(sub, VectorDiff::Append { values: expected });
    }

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn set() {
    let mut ob = ObservableVector::<usize>::new();
    let mut limit = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_limit(Observable::subscribe(&limit));

    // Add 3 values.
    ob.append(vector![10, 11, 12]);

    // Set limit to 2.
    Observable::set(&mut limit, 2);

    // Observe 2 values.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![10, 11] });

    // Set 1 value.
    ob.set(0, 20);

    // Observe 1 update.
    assert_next_eq!(sub, VectorDiff::Set { index: 0, value: 20 });

    // Set 1 value at the limit.
    ob.set(1, 21);

    // Observe 1 update.
    assert_next_eq!(sub, VectorDiff::Set { index: 1, value: 21 });

    // Set 1 value after the limit.
    ob.set(2, 22);

    // Observe nothing.
    assert_pending!(sub);

    // Check the content of the vector.
    {
        let expected = vector![20, 21, 22];
        assert_eq!(*ob, expected);

        Observable::set(&mut limit, 0);
        assert_next_eq!(sub, VectorDiff::Truncate { length: 0 });

        Observable::set(&mut limit, 42);
        assert_next_eq!(sub, VectorDiff::Append { values: expected });
    }

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn remove() {
    let mut ob = ObservableVector::<usize>::new();
    let mut limit = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_limit(Observable::subscribe(&limit));

    // Add 5 values.
    ob.append(vector![10, 11, 12, 13, 14]);

    // Set limit to 2.
    Observable::set(&mut limit, 2);

    // Observe 2 values.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![10, 11] });

    // Remove 1 value after the limit.
    ob.remove(3);

    // Observe nothing.
    assert_pending!(sub);

    // Remove 1 value at the limit.
    ob.remove(1);

    // Observe 1 value being removed, and 1 new value being pushed back.
    assert_next_eq!(sub, VectorDiff::Remove { index: 1 });
    assert_next_eq!(sub, VectorDiff::PushBack { value: 12 });

    // Remove 1 value.
    ob.remove(0);

    // Observe 1 value being removed, and 1 new value being pushed back.
    assert_next_eq!(sub, VectorDiff::Remove { index: 0 });
    assert_next_eq!(sub, VectorDiff::PushBack { value: 14 });
    assert_pending!(sub);

    // Check the content of the vector.
    {
        let expected = vector![12, 14];
        assert_eq!(*ob, expected);

        Observable::set(&mut limit, 0);
        assert_next_eq!(sub, VectorDiff::Truncate { length: 0 });

        Observable::set(&mut limit, 42);
        assert_next_eq!(sub, VectorDiff::Append { values: expected });
    }

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn truncate() {
    let mut ob = ObservableVector::<usize>::new();
    let mut limit = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_limit(Observable::subscribe(&limit));

    // Add 5 values.
    ob.append(vector![10, 11, 12, 13, 14]);

    // Set limit to 3.
    Observable::set(&mut limit, 3);

    // Observe 3 values.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![10, 11, 12] });

    // Truncate after the limit.
    ob.truncate(4);

    // Observe nothing.
    assert_pending!(sub);

    // Truncate at the limit.
    ob.truncate(3);

    // Observe nothing.
    assert_pending!(sub);

    // Truncate.
    ob.truncate(1);

    // Observe truncation.
    assert_next_eq!(sub, VectorDiff::Truncate { length: 1 });

    // Check the content of the vector.
    {
        let expected = vector![10];
        assert_eq!(*ob, expected);

        Observable::set(&mut limit, 0);
        assert_next_eq!(sub, VectorDiff::Truncate { length: 0 });

        Observable::set(&mut limit, 42);
        assert_next_eq!(sub, VectorDiff::Append { values: expected });
    }

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn reset() {
    let mut ob = ObservableVector::<usize>::with_capacity(2);
    let mut limit = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_limit(Observable::subscribe(&limit));

    // Add 1 value.
    ob.append(vector![10]);

    // Set limit to 4.
    Observable::set(&mut limit, 4);

    // Observe 1 value.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![10] });

    // Modify many values to saturate the observable capacity, in order to trigger a
    // reset.
    ob.push_back(11);
    ob.append(vector![12, 13]);
    ob.insert(0, 14);

    // Observe a reset, capped to the limit.
    assert_next_eq!(sub, VectorDiff::Reset { values: vector![14, 10, 11, 12] });
    assert_pending!(sub);

    // Check the content of the vector.
    {
        let expected = vector![14, 10, 11, 12, 13];
        assert_eq!(*ob, expected);

        Observable::set(&mut limit, 0);
        assert_next_eq!(sub, VectorDiff::Truncate { length: 0 });

        Observable::set(&mut limit, 42);
        assert_next_eq!(sub, VectorDiff::Append { values: expected });
    }

    drop(ob);
    assert_closed!(sub);
}

#[tokio::test]
async fn limit_stream_wake_bug() {
    use futures_util::{FutureExt, StreamExt};
    use tokio::task::yield_now;

    let ob = ObservableVector::<u32>::from(vector![1, 2]);
    let mut limit = Observable::new(1);
    let (values, mut sub) =
        ob.subscribe().dynamic_limit_with_initial_value(1, Observable::subscribe(&limit));

    assert_eq!(values, vector![1]);

    let task_hdl = tokio::spawn(async move {
        let update = sub.next().await.unwrap();
        assert_eq!(update, VectorDiff::Append { values: vector![2] });
    });

    // Set the limit to the same value that `Limit` has already seen.
    Observable::set(&mut limit, 1);

    // Make sure the task spawned above sees the "updated" limit.
    yield_now().await;

    // Now update the limit again.
    Observable::set(&mut limit, 2);

    // Allow the task to make progress.
    yield_now().await;

    // It should be finished now.
    task_hdl.now_or_never().unwrap().unwrap();
}
