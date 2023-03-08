use eyeball::unique::Observable;

#[tokio::test]
async fn lag() {
    let mut ob = Observable::new("hello, world!".to_owned());
    let mut rx1 = Observable::subscribe(&ob);
    let mut rx2 = Observable::subscribe(&ob);

    Observable::set(&mut ob, "A".to_owned());
    assert_eq!(rx1.next().await, Some("A".to_owned()));

    Observable::set(&mut ob, "B".to_owned());
    assert_eq!(rx1.next().await, Some("B".to_owned()));
    assert_eq!(rx2.next().await, Some("B".to_owned()));
}

#[tokio::test]
async fn separate_tasks() {
    let mut ob = Observable::new(Box::new([0; 256]));

    let mut subscriber = Observable::subscribe(&ob);
    let handle = tokio::spawn(async move {
        let mut value = subscriber.next().await.unwrap();
        while let Some(update) = subscriber.next().await {
            value = update;
        }
        assert_eq!(value, Box::new([32; 256]));
        assert_eq!(subscriber.next().await, None);
    });

    for i in 1..=32 {
        Observable::set(&mut ob, Box::new([i; 256]));
        tokio::task::yield_now().await;
    }
    drop(ob);

    handle.await.unwrap();
}

#[tokio::test]
async fn lag_no_clone() {
    // no Clone impl
    struct Foo(String);

    let mut ob = Observable::new(Foo("hello, world!".to_owned()));
    let mut rx1 = Observable::subscribe(&ob);
    let mut rx2 = Observable::subscribe(&ob);

    Observable::set(&mut ob, Foo("A".to_owned()));
    assert_eq!(rx1.next_ref().await.as_ref().map(|f| f.0.as_str()), Some("A"));

    Observable::set(&mut ob, Foo("B".to_owned()));
    assert_eq!(rx1.next_ref().await.as_ref().map(|f| f.0.as_str()), Some("B"));
    assert_eq!(rx2.next_ref().await.as_ref().map(|f| f.0.as_str()), Some("B"));
}
