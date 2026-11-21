# Fuzz targets

Fuzz targets of the JSON reader and navigation, for AC-JS11 (9) of
[the JSON columns PRD](../docs/prd/krummi/tables/tables-json.prd.en.md). They are a development tool outside the
workspace and the library's build, and run on Linux with nightly Rust and `cargo-fuzz`:

```text
cargo +nightly fuzz run json_normalize -- -max_total_time=3600
cargo +nightly fuzz run json_navigate -- -max_total_time=3600
```

- `json_normalize`: the reader over arbitrary bytes never panics or overflows the stack; an accepted text normalizes
  to itself and is never longer than its input.
- `json_navigate`: every value of a document normalized from arbitrary bytes is visited by iteration, lookup and
  pointer, and read as every type, without a panic.

An input that makes a target fail is fixed and added to the property tests of the reader
(`crates/krummi/tests/json_reader/properties.rs`).
