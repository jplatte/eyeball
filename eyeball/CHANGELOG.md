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
