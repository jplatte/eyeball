# eyeball

This crate implements a basic form of the [Observer pattern][] for Rust.
It provides `Observable<T>` as a type that uniquely owns an inner value `T` and
broadcasts changes to any associated `Subscriber<T>`s.
`Subscriber`s can currently only be polled for updates using `async` / `.await`,
but this may change in the future.

Here is a quick walk-through:

```rust
use eyeball::Observable;

let mut observable = Observable::new("A".to_owned());
// Observable has no methods of its own, as those could conflict
// with methods of the inner type, which it `Deref`erences to.
let mut subscriber1 = Observable::subscribe(&observable);
let mut subscriber2 = Observable::subscribe(&observable);

// You can get the current value from a subscriber without waiting
// for updates.
assert_eq!(subscriber1.get(), "A");

Observable::set(&mut observable, "B".to_owned());
// `.next().await` will wait for the next update, then return the
// new value.
assert_eq!(subscriber1.next().await, Some("B".to_owned()));

// If multiple updates have happened without the subscriber being
// polled, the next poll will skip all but the latest.
Observable::set(&mut observable, "C".to_owned());
assert_eq!(subscriber1.next().await, Some("C".to_owned()));
assert_eq!(subscriber2.next().await, Some("C".to_owned()));

// You can even obtain the value without cloning the value, by
// using `.read()` (no waiting) or `.next_ref().await` (waits for
// the next update).
// If you restrict yourself to these methods, you can even use
// `Observable` with inner types that don't implement the `Clone`
// trait.
// However, note that while a read guard returned by `.read()` or
// `.next_ref().await` is alive, updating the observable is
// blocked.
Observable::set(&mut observable, "D".to_owned());
{
    let guard = subscriber1.next_ref().await.unwrap();
    assert_eq!(*guard, "D");
}

// The latest value is kept alive by subscribers when the
// `Observable` is dropped.
drop(observable);
assert_eq!(subscriber1.get(), "D");
assert_eq!(*subscriber2.read(), "D");
```

There is also the type `SharedObservable<T>` as a wrapper around 
`Arc<RwLock<Observable<T>>>` that is a little more convenient to use than that
type.

For more details, see the documentation [on docs.rs][docs.rs].

[Observer pattern]: https://en.wikipedia.org/wiki/Observer_pattern
[docs.rs]: https://docs.rs/eyeball/latest/eyeball/
