# 0.2.5

- Emit more tracing events when the `tracing` Cargo feature is enabled

# 0.2.4

- Add `ObservableVector::entries`

# 0.2.3

- Add `entry` and `for_each` methods to `ObservableVector`

# 0.2.2

- Update the lag handling in `VectorSubscriber` to yield a `VectorDiff::Reset`
  with the latest state, instead of the state N - 1 changes earlier (where N is
  the capacity of the internal buffer)

# 0.2.1

- Add `VectorDiff::map`

# 0.2.0

- Switch from the unmaintained `im` crate to the maintained fork `imbl`
- Re-export the non-observable `Vector` type from the crate root
