use eyeball::SharedObservable;
use tokio::task::JoinSet;

#[tokio::test]
async fn lag() {
    let ob = SharedObservable::new("hello, world!".to_owned());
    let mut rx1 = ob.subscribe();
    let mut rx2 = ob.subscribe();

    ob.set("A".to_owned());
    assert_eq!(rx1.next().await, Some("A".to_owned()));

    ob.set("B".to_owned());
    assert_eq!(rx1.next().await, Some("B".to_owned()));
    assert_eq!(rx2.next().await, Some("B".to_owned()));
}

#[tokio::test]
#[cfg_attr(miri, ignore)]
async fn separate_tasks() {
    let ob = SharedObservable::new(Box::new([0; 256]));

    let mut subscriber = ob.subscribe();
    let handle = tokio::spawn(async move {
        let mut value = subscriber.next().await.unwrap();
        while let Some(update) = subscriber.next().await {
            value = update;
        }
        assert_eq!(value, Box::new([32; 256]));
        assert_eq!(subscriber.next().await, None);
    });

    let mut join_set = JoinSet::new();
    for i in 1..=32 {
        let ob = ob.clone();
        join_set.spawn(async move {
            ob.set(Box::new([i; 256]));
        });
        tokio::task::yield_now().await;
    }
    drop(ob);

    handle.await.unwrap();
}

#[tokio::test]
async fn lag_no_clone() {
    // no Clone impl
    struct Foo(String);

    let ob = SharedObservable::new(Foo("hello, world!".to_owned()));
    let mut rx1 = ob.subscribe();
    let mut rx2 = ob.subscribe();

    ob.set(Foo("A".to_owned()));
    assert_eq!(rx1.next_ref().await.as_ref().map(|f| f.0.as_str()), Some("A"));

    ob.set(Foo("B".to_owned()));
    assert_eq!(rx1.next_ref().await.as_ref().map(|f| f.0.as_str()), Some("B"));
    assert_eq!(rx2.next_ref().await.as_ref().map(|f| f.0.as_str()), Some("B"));
}
