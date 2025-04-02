use eyeball::Observable;
use eyeball_im::{ObservableVector, VectorDiff};
use eyeball_im_util::vector::VectorObserverExt;
use imbl::vector;
use stream_assert::{assert_closed, assert_next_eq, assert_pending};

#[test]
fn static_count() {
    let mut ob: ObservableVector<usize> = ObservableVector::from(vector![10, 11, 12, 13]);
    let (initial_values, mut sub) = ob.subscribe().skip(2);
    assert_eq!(initial_values, vector![12, 13]);
    assert_pending!(sub);

    // 10 is removed, it shifts values and 12 is removed.
    ob.pop_front();
    assert_next_eq!(sub, VectorDiff::PopFront);

    // 14 is added at the back.
    ob.push_back(14);
    assert_next_eq!(sub, VectorDiff::PushBack { value: 14 });

    assert_pending!(sub);
    drop(ob);
    assert_closed!(sub);
}

#[test]
fn pending_until_count_emits_a_value() {
    let mut ob: ObservableVector<usize> = ObservableVector::new();
    let mut count = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_skip(Observable::subscribe(&count));

    // Append new values…
    ob.append(vector![10, 11, 12, 13, 14, 15]);

    // … but it's still pending…
    assert_pending!(sub);

    // … because the `count` stream didn't produce any value yet.
    // Let's change that.
    Observable::set(&mut count, 3);

    // Here we are.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![13, 14, 15] });

    assert_pending!(sub);
    drop(ob);
    assert_closed!(sub);
}

#[test]
fn update_count_before_polling_sub() {
    let mut ob = ObservableVector::<usize>::new();
    let mut count = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_skip(Observable::subscribe(&count));

    // Append 4 values.
    ob.append(vector![10, 11, 12, 13]);

    // Observe nothing because the count is still undefined.
    assert_pending!(sub);

    // Set count to 2.
    Observable::set(&mut count, 2);

    // Observe 2 values.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![12, 13] });

    // Observe nothing.
    assert_pending!(sub);

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn update_count_after_polling_sub() {
    let mut ob = ObservableVector::<usize>::new();
    let mut count = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_skip(Observable::subscribe(&count));

    // Append 4 values.
    ob.append(vector![10, 11, 12, 13]);

    // Set count to 2.
    Observable::set(&mut count, 2);

    // Observe 2 values.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![12, 13] });

    // Observe nothing.
    assert_pending!(sub);

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn increase_and_decrease_the_count_on_an_empty_stream() {
    let ob = ObservableVector::<usize>::new();
    let mut count = Observable::new(1);
    let (initial_values, mut sub) =
        ob.subscribe().dynamic_skip_with_initial_count(1, Observable::subscribe(&count));

    assert!(initial_values.is_empty());

    // `ob` is empty!

    // Set count to 2.
    Observable::set(&mut count, 2);

    // Observe nothing.
    assert_pending!(sub);

    // Set count to 1.
    Observable::set(&mut count, 1);

    // Observe nothing.
    assert_pending!(sub);

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn increase_and_decrease_the_count_only() {
    let mut ob = ObservableVector::<usize>::new();
    let mut count = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_skip(Observable::subscribe(&count));

    // Append 4 values.
    ob.append(vector![10, 11, 12, 13, 14]);

    // Observe nothing because the count is still undefined.
    assert_pending!(sub);

    // Set `count` to its first value.
    Observable::set(&mut count, 2);

    assert_next_eq!(sub, VectorDiff::Append { values: vector![12, 13, 14] });

    // Set `count` closer to the length of the vector.
    Observable::set(&mut count, 4);

    assert_next_eq!(sub, VectorDiff::PopFront);
    assert_next_eq!(sub, VectorDiff::PopFront);

    // Set `count` to the length of the vector.
    Observable::set(&mut count, 5);

    // Optimisation: A `Clear` is emitted instead of many `PopFront`.
    assert_next_eq!(sub, VectorDiff::Clear);

    // Set `count` out of bounds.
    Observable::set(&mut count, 7);

    assert_pending!(sub);

    // Set `count` back to the length of the vector.
    Observable::set(&mut count, 5);

    assert_pending!(sub);

    // Set `count` to 2.
    Observable::set(&mut count, 2);

    assert_next_eq!(sub, VectorDiff::PushFront { value: 14 });
    assert_next_eq!(sub, VectorDiff::PushFront { value: 13 });
    assert_next_eq!(sub, VectorDiff::PushFront { value: 12 });

    // Set `count` to 0.
    Observable::set(&mut count, 0);

    assert_next_eq!(sub, VectorDiff::PushFront { value: 11 });
    assert_next_eq!(sub, VectorDiff::PushFront { value: 10 });

    // Set `count` to 3.
    Observable::set(&mut count, 3);

    assert_next_eq!(sub, VectorDiff::PopFront);
    assert_next_eq!(sub, VectorDiff::PopFront);
    assert_next_eq!(sub, VectorDiff::PopFront);

    // Enough fun.

    assert_pending!(sub);
    drop(ob);
    assert_closed!(sub);
}

#[test]
fn count_at_limits() {
    let mut ob = ObservableVector::<usize>::new();
    let mut count = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_skip(Observable::subscribe(&count));

    // Append 4 values.
    ob.append(vector![10, 11, 12, 13]);

    // Set `count` to 10.
    Observable::set(&mut count, 10);

    assert_pending!(sub);

    // Add 1 value.
    ob.push_back(14);

    assert_pending!(sub);

    // Set `count` to 0.
    Observable::set(&mut count, 0);

    // Optimisation: a single `Append` is generated instead of 5 `PushFront`.
    assert_next_eq!(sub, VectorDiff::Append { values: vector![10, 11, 12, 13, 14] });

    // Set `count` to 10 again.
    Observable::set(&mut count, 10);

    assert_next_eq!(sub, VectorDiff::Clear);

    // Add 1 value.
    ob.push_back(15);

    assert_pending!(sub);

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn count_is_polled_first() {
    let mut ob = ObservableVector::<usize>::new();
    let mut count = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_skip(Observable::subscribe(&count));

    // Append 4 values.
    ob.append(vector![10, 11, 12, 13]);

    // Set `count` to 3.
    Observable::set(&mut count, 3);

    assert_next_eq!(sub, VectorDiff::Append { values: vector![13] });

    // Add 1 value on the back…
    ob.push_back(14);

    // … and set `count` to 2 _after_.
    Observable::set(&mut count, 2);

    // However, let's observe the `count`'s change _first_…
    assert_next_eq!(sub, VectorDiff::PushFront { value: 12 });

    // … and let's observe the other updates _after_.
    assert_next_eq!(sub, VectorDiff::PushBack { value: 14 });
    assert_pending!(sub);

    drop(ob);
    assert_closed!(sub);
}

#[test]
fn append() {
    let mut ob = ObservableVector::<usize>::new();
    let mut count = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_skip(Observable::subscribe(&count));

    // We need to test appending values:
    //
    // - before `count` where not a single value reaches `count` (test case #1),
    // - before `count` where some values are outreaching `count` (test case #2),
    // - after `count` (test case #3).

    Observable::set(&mut count, 6);

    // Test case #1.
    //
    // Count is not reached; append values; no value reaches `count`.
    {
        ob.append(vector![10, 11, 12, 13]);

        assert_pending!(sub);

        // State of:
        //
        // - the vector: [ 10, 11, 12, 13 ]
        // - the “view”: [  X   X   X   X ]
    }

    // Test case #2.
    //
    // Count is not reached; append values; some values are outreaching `count`.
    {
        ob.append(vector![14, 15, 16, 17]);

        assert_next_eq!(sub, VectorDiff::Append { values: vector![16, 17] });

        // State of:
        //
        // - the vector: [ 10, 11, 12, 13, 14, 15, 16, 17 ]
        // - the “view”: [  X   X   X   X   X   X, 16, 17 ]
    }

    // Test case #3.
    //
    // Count is reached; append values.
    {
        ob.append(vector![18, 19, 20]);

        assert_next_eq!(sub, VectorDiff::Append { values: vector![18, 19, 20] });

        // State of:
        //
        // - the vector: [ 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20 ]
        // - the “view”: [  X   X   X   X   X   X, 16, 17, 18, 19, 20 ]
    }

    assert_pending!(sub);

    // Check the content of the vector.
    {
        let expected = vector![10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20];
        assert_eq!(*ob, expected);

        Observable::set(&mut count, 128);
        assert_next_eq!(sub, VectorDiff::Clear);

        Observable::set(&mut count, 0);
        assert_next_eq!(sub, VectorDiff::Append { values: expected });
    }

    assert_pending!(sub);
    drop(ob);
    assert_closed!(sub);
}

#[test]
fn clear() {
    let mut ob = ObservableVector::<usize>::new();
    let mut count = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_skip(Observable::subscribe(&count));

    // Set `count` to 4.
    Observable::set(&mut count, 4);

    // Add 1 value.
    ob.push_back(10);

    // Clear.
    ob.clear();

    // Observe a single update (the push back is skipped).
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
    let mut count = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_skip(Observable::subscribe(&count));

    // Set `count` to 2.
    Observable::set(&mut count, 2);

    // Vector is empty and `count` is polled for the first time.
    {
        ob.push_front(10);

        assert_pending!(sub);

        // State of:
        //
        // - the vector: [ 10 ]
        // - the “view”: [  X ]
    }

    // Push front, the `count` is not reached yet.
    {
        ob.push_front(11);

        assert_pending!(sub);

        // State of:
        //
        // - the vector: [ 11, 10 ]
        // - the “view”: [  X,  X ]
    }

    // Push front, the `count` is reached.
    {
        ob.push_front(12);

        assert_next_eq!(sub, VectorDiff::PushFront { value: 10 });

        // State of:
        //
        // - the vector: [ 12, 11, 10 ]
        // - the “view”: [  X,  X, 10 ]
    }

    // Push front, the `count` is outreached.
    {
        ob.push_front(13);

        assert_next_eq!(sub, VectorDiff::PushFront { value: 11 });

        // State of:
        //
        // - the vector: [ 13, 12, 11, 10 ]
        // - the “view”: [  X,  X, 11, 10 ]
    }

    // Check the content of the vector.
    {
        let expected = vector![13, 12, 11, 10];
        assert_eq!(*ob, expected);

        Observable::set(&mut count, 128);
        assert_next_eq!(sub, VectorDiff::Clear);

        Observable::set(&mut count, 0);
        assert_next_eq!(sub, VectorDiff::Append { values: expected });
    }

    assert_pending!(sub);
    drop(ob);
    assert_closed!(sub);
}

#[test]
fn push_back() {
    let mut ob = ObservableVector::<usize>::new();
    let mut count = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_skip(Observable::subscribe(&count));

    // Set `count` to 2.
    Observable::set(&mut count, 2);

    // Vector is empty and `count` is polled for the first time.
    {
        ob.push_back(10);

        assert_pending!(sub);

        // State of:
        //
        // - the vector: [ 10 ]
        // - the “view”: [  X ]
    }

    // Push back, the `count` is not reached yet.
    {
        ob.push_back(11);

        assert_pending!(sub);

        // State of:
        //
        // - the vector: [ 10, 11 ]
        // - the “view”: [  X,  X ]
    }

    // Push back, the `count` is reached.
    {
        ob.push_back(12);

        assert_next_eq!(sub, VectorDiff::PushBack { value: 12 });

        // State of:
        //
        // - the vector: [ 10, 11, 12 ]
        // - the “view”: [  X,  X, 12 ]
    }

    // Push back, the `count` is outreached.
    {
        ob.push_back(13);

        assert_next_eq!(sub, VectorDiff::PushBack { value: 13 });

        // State of:
        //
        // - the vector: [ 10, 11, 12, 13 ]
        // - the “view”: [  X,  X, 12, 13 ]
    }

    // Check the content of the vector.
    {
        let expected = vector![10, 11, 12, 13];
        assert_eq!(*ob, expected);

        Observable::set(&mut count, 128);
        assert_next_eq!(sub, VectorDiff::Clear);

        Observable::set(&mut count, 0);
        assert_next_eq!(sub, VectorDiff::Append { values: expected });
    }

    assert_pending!(sub);
    drop(ob);
    assert_closed!(sub);
}

#[test]
fn pop_front() {
    let mut ob = ObservableVector::<usize>::new();
    let mut count = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_skip(Observable::subscribe(&count));

    // Init state.
    {
        ob.append(vector![10, 11, 12, 13]);
        Observable::set(&mut count, 2);

        assert_next_eq!(sub, VectorDiff::Append { values: vector![12, 13] });

        // State of:
        //
        // - the vector: [ 10, 11, 12, 13 ]
        // - the “view”: [  X,  X, 12, 13 ]
    }

    // Pop front, it shifts values before, and thus after `count`.
    {
        ob.pop_front(); // 10

        assert_next_eq!(sub, VectorDiff::PopFront); // 12

        // State of:
        //
        // - the vector: [ 11, 12, 13 ]
        // - the “view”: [  X,  X, 13 ]
    }

    // Pop front, it shifts values before, and thus after `count`.
    {
        ob.pop_front(); // 11

        assert_next_eq!(sub, VectorDiff::PopFront); // 13

        // State of:
        //
        // - the vector: [ 12, 13 ]
        // - the “view”: [  X,  X ]
    }

    // Pop front, it shifts values before `count`.
    {
        ob.pop_front(); // 12

        assert_pending!(sub);

        // State of:
        //
        // - the vector: [ 13 ]
        // - the “view”: [  X ]
    }

    // Check the content of the vector.
    {
        let expected = vector![13];
        assert_eq!(*ob, expected);

        Observable::set(&mut count, 128);
        assert_pending!(sub);

        Observable::set(&mut count, 0);
        assert_next_eq!(sub, VectorDiff::Append { values: expected });
    }

    assert_pending!(sub);
    drop(ob);
    assert_closed!(sub);
}

#[test]
fn pop_back() {
    let mut ob = ObservableVector::<usize>::new();
    let mut count = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_skip(Observable::subscribe(&count));

    // Init state.
    {
        ob.append(vector![10, 11, 12, 13]);
        Observable::set(&mut count, 2);

        assert_next_eq!(sub, VectorDiff::Append { values: vector![12, 13] });

        // State of:
        //
        // - the vector: [ 10, 11, 12, 13 ]
        // - the “view”: [  X,  X, 12, 13 ]
    }

    // Pop back, it happens after `count`.
    {
        ob.pop_back(); // 13

        assert_next_eq!(sub, VectorDiff::PopBack);

        // State of:
        //
        // - the vector: [ 10, 11, 12 ]
        // - the “view”: [  X,  X, 12 ]
    }

    // Pop back, it happens after `count`.
    {
        ob.pop_back(); // 12

        assert_next_eq!(sub, VectorDiff::PopBack);

        // State of:
        //
        // - the vector: [ 10, 11 ]
        // - the “view”: [  X,  X ]
    }

    // Pop back, it happens before `count`.
    {
        ob.pop_back(); // 11

        assert_pending!(sub);

        // State of:
        //
        // - the vector: [ 10 ]
        // - the “view”: [  X ]
    }

    // Check the content of the vector.
    {
        let expected = vector![10];
        assert_eq!(*ob, expected);

        Observable::set(&mut count, 128);
        assert_pending!(sub);

        Observable::set(&mut count, 0);
        assert_next_eq!(sub, VectorDiff::Append { values: expected });
    }

    assert_pending!(sub);
    drop(ob);
    assert_closed!(sub);
}

#[test]
fn insert() {
    let mut ob = ObservableVector::<usize>::new();
    let mut count = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_skip(Observable::subscribe(&count));

    Observable::set(&mut count, 2);

    // Insert at 0, on an empty vector.
    {
        ob.insert(0, 10);

        assert_pending!(sub);

        // State of:
        //
        // - the vector: [ 10 ]
        // - the “view”: [  X ]
    }

    // Insert at 0, on a non-empty vector, still inside the view.
    {
        ob.insert(0, 11);

        assert_pending!(sub);

        // State of:
        //
        // - the vector: [ 11, 10 ]
        // - the “view”: [  X,  X ]
    }

    // Insert at 0, on a non-empty vector, but this time, `count` is reached.
    {
        ob.insert(0, 12);

        assert_next_eq!(sub, VectorDiff::PushFront { value: 10 });

        // State of:
        //
        // - the vector: [ 12, 11, 10 ]
        // - the “view”: [  X,  X, 10 ]
    }

    // Insert just before `count`.
    {
        ob.insert(1, 13);

        assert_next_eq!(sub, VectorDiff::PushFront { value: 11 });

        // State of:
        //
        // - the vector: [ 12, 13, 11, 10 ]
        // - the “view”: [  X,  X, 11, 10 ]
    }

    // Insert at `count`.
    {
        ob.insert(2, 14);

        assert_next_eq!(sub, VectorDiff::Insert { index: 0, value: 14 });

        // State of:
        //
        // - the vector: [ 12, 13, 14, 11, 10 ]
        // - the “view”: [  X,  X, 14, 11, 10 ]
    }

    // Insert after `count`.
    {
        ob.insert(3, 15);

        assert_next_eq!(sub, VectorDiff::Insert { index: 1, value: 15 });

        // State of:
        //
        // - the vector: [ 12, 13, 14, 15, 11, 10 ]
        // - the “view”: [  X,  X, 14, 15, 11, 10 ]
    }

    // Insert at the end of the vector, after `count`.
    {
        ob.insert(6, 16);

        assert_next_eq!(sub, VectorDiff::Insert { index: 4, value: 16 });

        // State of:
        //
        // - the vector: [ 12, 13, 14, 15, 11, 10, 16 ]
        // - the “view”: [  X,  X, 14, 15, 11, 10, 16 ]
    }

    // Check the content of the vector.
    {
        let expected = vector![12, 13, 14, 15, 11, 10, 16];
        assert_eq!(*ob, expected);

        Observable::set(&mut count, 128);
        assert_next_eq!(sub, VectorDiff::Clear);

        Observable::set(&mut count, 0);
        assert_next_eq!(sub, VectorDiff::Append { values: expected });
    }

    assert_pending!(sub);
    drop(ob);
    assert_closed!(sub);
}

#[test]
fn set() {
    let mut ob = ObservableVector::<usize>::new();
    let mut count = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_skip(Observable::subscribe(&count));

    // Init state.
    {
        ob.append(vector![10, 11, 12, 13]);
        Observable::set(&mut count, 2);

        assert_next_eq!(sub, VectorDiff::Append { values: vector![12, 13] });

        // State of:
        //
        // - the vector: [ 10, 11, 12, 13 ]
        // - the “view”: [  X,  X, 12, 13 ]
    }

    // Set before `count`.
    {
        ob.set(0, 20);

        assert_pending!(sub);

        // State of:
        //
        // - the vector: [ 20, 11, 12, 13 ]
        // - the “view”: [  X,  X, 12, 13 ]
    }

    // Set at `count`.
    {
        ob.set(2, 22);

        assert_next_eq!(sub, VectorDiff::Set { index: 0, value: 22 });

        // State of:
        //
        // - the vector: [ 20, 11, 22, 13 ]
        // - the “view”: [  X,  X, 22, 13 ]
    }

    // Set after `count`.
    {
        ob.set(3, 23);

        assert_next_eq!(sub, VectorDiff::Set { index: 1, value: 23 });

        // State of:
        //
        // - the vector: [ 20, 11, 22, 23 ]
        // - the “view”: [  X,  X, 22, 23 ]
    }

    // Check the content of the vector.
    {
        let expected = vector![20, 11, 22, 23];
        assert_eq!(*ob, expected);

        Observable::set(&mut count, 128);
        assert_next_eq!(sub, VectorDiff::Clear);

        Observable::set(&mut count, 0);
        assert_next_eq!(sub, VectorDiff::Append { values: expected });
    }

    assert_pending!(sub);
    drop(ob);
    assert_closed!(sub);
}

#[test]
fn remove() {
    let mut ob = ObservableVector::<usize>::new();
    let mut count = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_skip(Observable::subscribe(&count));

    // Init state.
    {
        ob.append(vector![10, 11, 12, 13, 14, 15]);
        Observable::set(&mut count, 3);

        assert_next_eq!(sub, VectorDiff::Append { values: vector![13, 14, 15] });

        // State of:
        //
        // - the vector: [ 10, 11, 12, 13, 14, 15 ]
        // - the “view”: [  X,  X,  X, 13, 14, 15 ]
    }

    // There are 4 test cases:
    //
    // - remove before `count`, with no value after `count` (test case #1),
    // - remove before `count`, with values after `count` (test case #2),
    // - remove at `count` (test case #3),
    // - remove after `count` (test case #4).

    // Test case #2.
    //
    // Remove before `count`, values after `count`.
    {
        ob.remove(1);

        assert_next_eq!(sub, VectorDiff::PopFront);

        // State of:
        //
        // - the vector: [ 10, 12, 13, 14, 15 ]
        // - the “view”: [  X,  X,  X, 14, 15 ]
    }

    // Test case #4.
    //
    // Remove after `count`.
    {
        ob.remove(4);

        assert_next_eq!(sub, VectorDiff::Remove { index: 1 });

        // State of:
        //
        // - the vector: [ 10, 12, 13, 14 ]
        // - the “view”: [  X,  X,  X, 14 ]
    }

    // Test case #3.
    //
    // Remove at `count`.
    {
        ob.remove(3);

        assert_next_eq!(sub, VectorDiff::Remove { index: 0 });

        // State of:
        //
        // - the vector: [ 10, 12, 13 ]
        // - the “view”: [  X,  X,  X ]
    }

    // Test case #1.
    //
    // Remove before `count`, no value after `count`.
    {
        ob.remove(1);

        assert_pending!(sub);

        // State of:
        //
        // - the vector: [ 10, 13 ]
        // - the “view”: [  X,  X ]
    }

    // Check the content of the vector.
    {
        let expected = vector![10, 13];
        assert_eq!(*ob, expected);

        Observable::set(&mut count, 128);
        assert_pending!(sub);

        Observable::set(&mut count, 0);
        assert_next_eq!(sub, VectorDiff::Append { values: expected });
    }

    assert_pending!(sub);
    drop(ob);
    assert_closed!(sub);
}

#[test]
fn truncate() {
    let mut ob = ObservableVector::<usize>::new();
    let mut count = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_skip(Observable::subscribe(&count));

    // Init state.
    {
        ob.append(vector![10, 11, 12, 13, 14, 15, 16, 17]);
        Observable::set(&mut count, 5);

        assert_next_eq!(sub, VectorDiff::Append { values: vector![15, 16, 17] });

        // State of:
        //
        // - the vector: [ 10, 11, 12, 13, 14, 15, 16, 17 ]
        // - the “view”: [  X,  X,  X,  X,  X, 15, 16, 17 ]
    }

    // Truncate to a size larger than `count`.
    {
        ob.truncate(7);

        assert_next_eq!(sub, VectorDiff::Truncate { length: 2 });

        // State of:
        //
        // - the vector: [ 10, 11, 12, 13, 14, 15, 16 ]
        // - the “view”: [  X,  X,  X,  X,  X, 15, 16 ]
    }

    // Truncate to a size equal to `count`.
    {
        ob.truncate(5);

        assert_next_eq!(sub, VectorDiff::Clear);

        // State of:
        //
        // - the vector: [ 10, 11, 12, 13, 14 ]
        // - the “view”: [  X,  X,  X,  X,  X ]
    }

    // Truncate to a size smaller than `count` with values after `count`.
    {
        Observable::set(&mut count, 4);

        assert_next_eq!(sub, VectorDiff::PushFront { value: 14 });

        // State of:
        //
        // - the vector: [ 10, 11, 12, 13, 14 ]
        // - the “view”: [  X,  X,  X,  X, 14 ]

        ob.truncate(3);

        assert_next_eq!(sub, VectorDiff::Clear);

        // State of:
        //
        // - the vector: [ 10, 11, 12 ]
        // - the “view”: [  X,  X,  X ]
    }

    // Truncate to a size smaller than `count` with no values after `count`.
    {
        ob.truncate(2);

        assert_pending!(sub);

        // State of:
        //
        // - the vector: [ 10, 11 ]
        // - the “view”: [  X,  X ]
    }

    // Check the content of the vector.
    {
        let expected = vector![10, 11];
        assert_eq!(*ob, expected);

        Observable::set(&mut count, 128);
        assert_pending!(sub);

        Observable::set(&mut count, 0);
        assert_next_eq!(sub, VectorDiff::Append { values: expected });
    }

    assert_pending!(sub);
    drop(ob);
    assert_closed!(sub);
}

#[test]
fn reset() {
    let mut ob = ObservableVector::<usize>::with_capacity(2);
    let mut count = Observable::new(0);
    let mut sub = ob.subscribe().dynamic_skip(Observable::subscribe(&count));

    // Init state.
    {
        ob.append(vector![10, 11, 12, 13, 14, 15]);
        Observable::set(&mut count, 3);

        assert_next_eq!(sub, VectorDiff::Append { values: vector![13, 14, 15] });

        // State of:
        //
        // - the vector: [ 10, 11, 12, 13, 14, 15 ]
        // - the “view”: [  X,  X,  X, 13, 14, 15 ]
    }

    // Do multiple operations to saturate the observable capacity, in order to
    // trigger a reset.
    {
        ob.push_back(16);
        ob.push_back(17);
        ob.push_back(18);
    }

    // Observe a reset, with skipped values.
    {
        assert_next_eq!(sub, VectorDiff::Reset { values: vector![13, 14, 15, 16, 17, 18] });

        // State of:
        //
        // - the vector: [ 10, 11, 12, 13, 14, 15, 16, 17, 18 ]
        // - the “view”: [  X,  X,  X, 13, 14, 15, 16, 17, 18 ]
    }

    // Check the content of the vector.
    {
        let expected = vector![10, 11, 12, 13, 14, 15, 16, 17, 18];
        assert_eq!(*ob, expected);

        Observable::set(&mut count, 128);
        assert_next_eq!(sub, VectorDiff::Clear);

        Observable::set(&mut count, 0);
        assert_next_eq!(sub, VectorDiff::Append { values: expected });
    }

    assert_pending!(sub);
    drop(ob);
    assert_closed!(sub);
}

// This test is copied (and modified to match the behaviour of `Skip`) from
// `Head`'s test suite. The bug is not present in `Skip`, but since their
// behaviours are quite similar, let's ensure the bug cannot happen
// preemptively.
#[tokio::test]
async fn count_stream_wake() {
    use futures_util::{FutureExt, StreamExt};
    use tokio::task::yield_now;

    let ob = ObservableVector::<u32>::from(vector![1, 2, 3]);
    let mut count = Observable::new(2);
    let (values, mut sub) =
        ob.subscribe().dynamic_skip_with_initial_count(2, Observable::subscribe(&count));

    assert_eq!(values, vector![3]);

    let task_hdl = tokio::spawn(async move {
        let update = sub.next().await.unwrap();
        assert_eq!(update, VectorDiff::PushFront { value: 2 });
    });

    // Set the `count` to the same value that `Skip` has already seen.
    Observable::set(&mut count, 1);

    // Make sure the task spawned above sees the "updated" count.
    yield_now().await;

    // Now update the `count` again.
    Observable::set(&mut count, 2);

    // Allow the task to make progress.
    yield_now().await;

    // It should be finished now.
    task_hdl.now_or_never().unwrap().unwrap();
}
