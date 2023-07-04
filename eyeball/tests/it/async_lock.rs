use eyeball::Observable;
use futures_util::FutureExt;
use stream_assert::{assert_next_eq, assert_pending};

#[tokio::test]
async fn smoke_test() {
    let mut ob = Observable::new_async("hello, world!");
    let mut rx1 = Observable::subscribe_async(&ob);

    assert_eq!(rx1.get().await, "hello, world!");
    assert_pending!(rx1);

    let join_hdl = tokio::spawn(async move {
        // Make sure this doesn't happen before the parent task starts waiting
        tokio::task::yield_now().await;
        Observable::set_async(&mut ob, "this is a test").await;
        ob
    });

    assert_eq!(rx1.next().await, Some("this is a test"));
    let ob = join_hdl.now_or_never().unwrap().unwrap();

    let ob = Observable::into_shared(ob);
    let mut rx2 = ob.subscribe().await;

    ob.set("A").await;
    assert_next_eq!(rx1, "A");

    ob.set("B").await;
    assert_next_eq!(rx1, "B");
    assert_next_eq!(rx2, "B");
}
