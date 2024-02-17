# unreleased

- Remove lifetime parameter from `SortBy`
- Allow `SortBy::new` and `VectorObserverExt::sort_by` to accept a callable
  directly, instead of through a reference (references still work since `&F`
  also implements `Fn(X) -> Y` if `F` does)
- Add `Sort`, `SortByKey` adapters and corresponding `VectorObserverExt` methods

# 0.5.3

- Add the `SortBy` adapter

# 0.5.2

- Optimize the implementation of the `Limit` adapter
- Fix broken links in the documentation

# 0.5.1

- Fix a bug where the `Limit` adapter would fail to register interest in new
  items from its limit stream

# 0.5.0

- Move bounds to simplify documentation
  - Technically a breaking change, which is why this version is 0.5.0
- Add `VectorSubscriberExt` and `VectorObserverExt` for fluent adapter
  construction

# 0.4.0

- Upgrade `eyeball-im` dependency to 0.4.0
- Add `Limit` adapter for presenting a limited view of an underlying observable
  vector

# 0.3.1

- Fix a bug with `Filter` and `FilterMap` that was corrupting their internal
  state, leading to invalid output or panics, when the underlying stream
  produced a `VectorDiff::Insert`

# 0.3.0

- Upgrade `eyeball-im` dependency to 0.3.0
  - Make adapter types compatible with new batched streams
  - Remove `VectorExt` as it didn't fit in very well with the new stream types
- Rename / move adapter types
  - `FilterVectorSubscriber` is now `vector::Filter`
  - `FilterMapVectorSubscriber` is now `vector::FilterMap`

# 0.2.2

This release only updates metadata for crates.io.

# 0.2.1

- Export `FilterMapVectorSubscriber` from the crate root

# 0.2.0

- Rename `subscribe_filtered` to `subscribe_filter`
- Rename `FilteredVectorSubscriber` to `FilterVectorSubscriber`
- Add `subscribe_filter_map` and `FilterMapVectorSubscriber`
