use eyeball_im::{ObservableVector, VectorDiff};
use eyeball_im_util::vector::FilterMap;
use imbl::vector;
use stream_assert::{assert_next_eq, assert_pending};

#[test]
fn filter_map_batch() {
    let mut ob: ObservableVector<i32> = ObservableVector::new();
    let (_, mut sub) =
        FilterMap::new(ob.clone(), ob.subscribe().into_batched_stream(), |i| u8::try_from(i).ok());

    ob.append(vector![1024, -1]);
    assert_pending!(sub);

    let mut txn = ob.transaction();
    txn.push_back(-1);
    txn.push_front(-2);
    txn.commit();

    assert_pending!(sub);

    let mut txn = ob.transaction();
    txn.push_back(1);
    assert_pending!(sub);
    txn.push_back(9999);
    txn.set(1, 2);
    assert_pending!(sub);
    txn.commit();

    assert_next_eq!(
        sub,
        vec![VectorDiff::PushBack { value: 1 }, VectorDiff::Insert { index: 0, value: 2 }]
    );
}
