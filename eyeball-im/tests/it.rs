use imbl::Vector;
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
