# Secondary indexes code review resolutions

Resolutions of [2026-11-19-code.md](2026-11-19-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-19

Review record: [2026-11-19-code.md](2026-11-19-code.md), range `0158fd4..767e251`, wrapper status `findings`,
verdict «Changes requested: three major findings remain open.» `threadId` `01a1f1bd-16a9-7373-be18-50c26576303e`;
telemetry: coordinator `gpt-6.1-sol`/`high`, reviewer `krummi-review-critical` (Pasteur) `gpt-6-astra`/`xhigh`.

Fix commits: `4bf9b3c` (R3), `2d2912b` (R2), `8f1343a` (R1), each test-first. After each fix: `cargo fmt --all
--check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` passed (58 test
binaries ok); the parent re-ran `cargo test -p krummi --test table_index_sort` after the last fix: 12 passed.

### R3 — major: a failed store opening deletes krummi.sort

- Disposition: fixed (`4bf9b3c`).
- Fix: recovery no longer removes `krummi.sort`. `Scratch` records whether the open succeeded; the new
  `Scratch::finish_open`, called last in `krummi_tx::Engine::new` after the metadata page check, removes a leftover
  sort folder (an access failure fails the open, as before) and marks the open as done. Close and drop remove
  `krummi.sort` only after that, so a partly opened engine's drop leaves it.
- Reproduction (RED before the fix: `run.probe` was gone): `ac_ix14_an_open_rejected_after_recovery_leaves_the_files_of_the_sort`
  in `crates/krummi/tests/table_index_sort.rs`, the review's scenario (page 2 damaged, `Damaged { page: Some(2) }`,
  `krummi.sort/run.probe` kept), then the repaired store opens and removes the folder. Making close and drop ignore
  the flag fails the test again.
- Not changed: `krummi.temp` has the same order (recovery removes it before the metadata page check); that behavior
  predates this range.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — major: index identities and states accept values already known by the catalog

- Disposition: fixed (`2d2912b`).
- Fix: `WriteTransaction::store_knows` checks every table of the catalog as the transaction sees it: the table's
  identity, every identity of its current schema (columns, indexes, schema state, index state) and its history tree
  (`known_identity`), through the existing `catalog::list` and `find_table`. `create_index` (identity and state) and
  `drop_index` (state) redraw a value that the table's history or `store_knows` knows, besides the existing check
  against the identity drawn in the same operation.
- Reproduction (RED before the fix: `(10, 100)`): `ac_ix11_identities_known_to_the_store_outside_the_table_are_drawn_again`
  in `crates/krummi/tests/table_indexes.rs`, the review's draws on a disk and a temporary store: creation gives
  identity 50 and state 60, the drop state 70, and Describe after commit agrees.
- Not changed: `create_table`'s column, schema-state and index-state draws and `change_schema`'s draws check only
  local values; that predates this range.
- Status: claimed fixed, unverified until a follow-up review.

### R1 — major: index construction aborts the process on an entry allocation failure

- Disposition: fixed (`8f1343a`).
- Fix: `EntryLayout::entry` reserves the entry key and the included-column payload through `krummi_tree::reserve`
  (the `Op::Alloc` fault point, `OutOfMemory`, T11) after its size checks, and writes them with the new
  `index_key::write_entry` and `RowLayout::encode_into`; `entry_key` (deletes) is fallible too. Index creation and the
  row writes that maintain indexes (insert, update, delete) propagate the error. `RowLayout::encode`, shared with row
  writes, now calls `encode_into` with an empty buffer: same bytes, still infallible.
- Reproductions: `ac_ix14_no_memory_for_an_entry_fails_the_creation_with_out_of_memory`
  (`table_index_sort.rs`; RED before the fix: no reservation of the key, 1,004 bytes, or the payload, 2,097,156
  bytes, was reached) and `no_memory_for_an_entry_fails_the_row_write_with_out_of_memory` (`table_indexes.rs`; RED
  observed by temporarily restoring infallible reservations). Each counts the `Op::Alloc` reservations of a
  successful operation and fails each in turn, on disk and temporary stores: `OutOfMemory`, T11 at commit, store
  and index entries unchanged.
- Not changed: small bounded allocations on these paths (value lists, the per-write step list, the uniqueness prefix
  of at most 2,048 bytes) and the pre-existing row encoding of a row write.
- Status: claimed fixed, unverified until a follow-up review.

RDD of the fixes, `767e251..8f1343a` (risk, resilience, reliability): approved, lineage `review-4d2dc3647d685d63`;
advisories: `store_knows` reads the whole catalog on each draw (`catalog.rs:800-816`; index creation and dropping
are rare, as `create_table`'s `catalog::has_identity`), and its history branch has no test of its own
(`catalog.rs:808-810`).
