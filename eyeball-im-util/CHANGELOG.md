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
