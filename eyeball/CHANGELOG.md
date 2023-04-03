# 0.5.1

- `Add shared::Observable::{observable_count, subscriber_count}`
- Make `unique::Observable::subscriber_count` a regular associated function like
  all the others, not a method

# 0.5.0

- Remove `T: Clone` bound from `set_eq`
- Merge `replace`s functionality of returning the previous value into `set`
- Remove `update_eq`, `update_hash`
- Rename `set_eq` to `set_if_not_eq`
- Rename `set_hash` to `set_if_hash_not_eq`
- Return the previous inner value if `set_if_not_eq` or `set_if_hash_not_eq`
  replaces it
- Add `update_if`

# 0.4.2

- Add `unique::Observable::subscriber_count`
- Add `shared::Observable::ref_count`

# 0.4.1

- Implement `Clone` for `Subscriber`
- Add `Subscriber::reset` and `Subscriber::clone_reset`
- Add `Observable::subscribe_reset` for both observable types

# 0.4.0

- Make `unique::Subscriber` and `shared::Subscriber` the same type
  - Same for `ObservableReadGuard` and other auxiliary types
- The `unique` Cargo feature was removed, `readlock` is no longer an optional
  dependency

# 0.3.2

- Add `shared::Observable::get`

# 0.3.1

- Relax `&mut` methods to `&` in `shared::Observable` (copy-paste error)

# 0.3.0

- Move the existing `Observable` into a module called `unique`, to contrast it
  with the shared observable type
- Remove `SharedObservable` (wrapper around an `Observable` with extra locking)
- Add `shared::Observable`, which provides a similar API to the previous
  `SharedObservable`, in a more efficient and more obvious way
- Add `#[clippy::has_significant_drop]` attribute to `SubscriberReadLock` so the
  [`clippy::significant_drop_in_scrutinee`] lint works with it
- Rewrite the waking implementation to not rely on `tokio`'s broadcast channel.
  This improves compile time if you're not using tokio otherwise, and improves
  performance when there's a small number of subscribers. Expect performance for
  more than 4 subscribers to potentially regress, especially if it's many more.
  This case might be optimized in the future.

[`clippy::significant_drop_in_scrutinee`]: https://rust-lang.github.io/rust-clippy/master/index.html#significant_drop_in_scrutinee

# 0.2.0

- Add more documentation
- Move `SharedObservableBase` and `ObservableLock` out of the crate root
  - They are now accessible in `eyeball::shared`

# 0.1.5

- Add `Subscriber::{next, next_ref, get, read}`

# 0.1.4

- Add `SharedObservable` convenience API

# 0.1.3

- Allow non-`Send` and / or non-`'static` values

# 0.1.2

- Implement `Default` for `Observable`
