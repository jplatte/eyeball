# eyeball 👁️

> eyeball (verb) - synonym for observe

Add observability to your Rust types!

There are two Rust crates to be found in this repository, both containing types
that broadcast changes made to them to subscribers (which can currently only be
polled using `async` / `.await`):

- [eyeball](./eyeball/) – Contains the basic `Observable` type and things related to that
- [eyeball-im](./eyeball-im/) – Contains observable collections (currently only `ObservableVector`)

Click on one of those two links to find out more.

## License

Both crates are distributed under the terms of the Mozilla Public License 2.0.
You can find the license text in the [LICENSE](./LICENSE) file.
