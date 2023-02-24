use eyeball::Observable;

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
