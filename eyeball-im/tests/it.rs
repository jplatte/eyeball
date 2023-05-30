use assert_matches::assert_matches;
use imbl::{vector, Vector};
use tokio_stream::StreamExt as _;

use eyeball_im::{ObservableVector, VectorDiff};

#[tokio::test]
async fn lag() {
    let mut ob = ObservableVector::with_capacity(1);
    let mut rx1 = ob.subscribe();
    let mut rx2 = ob.subscribe();

    ob.push_back("hello".to_owned());
    assert_eq!(rx1.next().await, Some(VectorDiff::PushBack { value: "hello".to_owned() }));

    ob.push_back("world".to_owned());
    assert_eq!(rx1.next().await, Some(VectorDiff::PushBack { value: "world".to_owned() }));
    assert_eq!(
        rx2.next().await,
        Some(VectorDiff::Reset {
            values: Vector::from_iter(["hello".to_owned(), "world".to_owned()])
        })
    );
}

#[tokio::test]
async fn lag2() {
    let mut ob: ObservableVector<i32> = ObservableVector::with_capacity(2);
    let mut sub = ob.subscribe();

    ob.push_back(0);
    ob.append(vector![1, 2]);
    ob.push_back(3);
    assert_matches!(sub.next().await, Some(VectorDiff::Reset { values }) => {
        assert_eq!(values, vector![0, 1, 2]);
    });
    assert_matches!(sub.next().await, Some(VectorDiff::PushBack { value: 3 }));
}
