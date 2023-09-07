use imbl::vector;
use stream_assert::{assert_next_eq, assert_pending};

use eyeball_im::{ObservableVector, VectorDiff};

#[test]
fn lagging_batch_stream() {
    let mut ob = ObservableVector::new();
    let mut st = ob.subscribe().into_batched_stream();

    ob.push_back(0);
    ob.append(vector![1, 2]);
    ob.push_back(3);

    assert_next_eq!(
        st,
        vec![
            VectorDiff::PushBack { value: 0 },
            VectorDiff::Append { values: vector![1, 2] },
            VectorDiff::PushBack { value: 3 },
        ]
    );
}

#[test]
fn transaction() {
    let mut ob = ObservableVector::new();
    let mut st = ob.subscribe().into_batched_stream();
    let mut txn = ob.transaction();

    txn.push_back(0);
    assert_pending!(st);

    txn.push_front(-1);
    assert_pending!(st);

    txn.commit();
    assert_next_eq!(
        st,
        vec![VectorDiff::PushBack { value: 0 }, VectorDiff::PushFront { value: -1 }]
    );

    let mut txn = ob.transaction();
    txn.push_back(3);
    txn.clear();
    txn.push_back(1);
    txn.commit();

    assert_next_eq!(st, vec![VectorDiff::Clear, VectorDiff::PushBack { value: 1 }]);
}
