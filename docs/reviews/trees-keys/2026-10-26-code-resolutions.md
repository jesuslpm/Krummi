# Trees and key operations: follow-up code review resolutions

Resolutions of [2026-10-26-code.md](2026-10-26-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-10-26

### R3 — major: rename reserves a name before rejecting an existing destination

- Disposition: accepted; fixed in `c70bbf1`.
- Fix: `rename_tree` in `crates/krummi/src/lib.rs` looks the destination up and rejects it with the already-exists
  class before reserving anything (A7: public validation errors come before allocations that are not catalog
  reads); the new name for the open-tree table is copied after that check and still before any catalog change.
- Evidence: `crates/krummi/tests/tree_memory.rs`
  `r3_renaming_onto_an_existing_tree_is_rejected_before_any_reservation` reproduces the review's scenario (trees
  `t` and `u`, a write that opened `t` and changed `k`, the next `Op::Alloc` reservation failing at each of the
  first four positions): RED before the fix (`OutOfMemory` instead of `TreeAlreadyExists`), GREEN after: the rename
  fails with the already-exists class, the write commits and a new read sees `changed`. `cargo test --workspace
  --no-fail-fast`: 294 passed, 0 failed, 9 ignored; fmt and clippy pass. RDD review `review-4519b6fb261a6f7c`
  (resilience, reliability): approved.
- Status: claimed fixed, unverified until a follow-up review.
