# eyeball 👁️

> eyeball (verb) - synonym for observe

Add observability to your Rust types!
Currently, polling observables is only possibly in async Rust,
but this constraint might be lifted in the future.

This repository hosts the following crates:

- [eyeball](./eyeball/) – Contains the basic `Observable` type and things related to that
- [eyeball-im](./eyeball-im/) – Contains observable collections (currently only `ObservableVector`)
- [eyeball-im-util](./eyeball-im-util/) – Contains additional utilities on top of `eyeball-im`

Click on one of the links to find out more.

## License

Both crates are distributed under the terms of the Mozilla Public License 2.0.
You can find the license text in the [LICENSE](./LICENSE) file.
