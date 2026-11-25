# Tables vector indexes code review resolutions

Resolutions of [2026-11-25-code.md](2026-11-25-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-25

Fix commits after the reviewed head `daaacd1`: R1 `732cd88`, R3 `7f7a39c`, R2 `52fe704`, R5 `a21f742`, R4 `abc006f`.
The review report is in `3d170ff`. `08a238f` fixes a warning of the RDD review of these fixes (L25 of the
execution document).

### R1 — Blocker: a concurrent unrelated column drop silently removes valid vector entries

- Disposition: fixed.
- Fix: the worker's target keeps the vector column's identity, not its place. The read side and the write side
  each compute the place from the schema of their own snapshot (`place_of` in
  `crates/krummi/src/table/vector_worker.rs`).
- Test: `table_vector_indexes::codex::r1_dropping_another_column_keeps_the_entries` follows the review's
  reproduction. The worker holds a target listed before `drop_column` while it trains another table. The test
  updates row 42, then checks that `check_vector_index` is empty and that the search through the index finds the row.
  RED before the fix (the row lost its entry), GREEN after.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — Blocker: valid primary keys panic the store's only vector worker

- Disposition: fixed.
- Fix: a primary key of up to 2,039 bytes (`INLINE_KEY`, the tree's maximum less the tag, generation and list)
  keeps its whole key in the posting and locator keys. A longer one, which a table admits up to 2,048 bytes, gets
  a number of its own in the data tree: tag 9 `generation ‖ list ‖ number` → code, 10 `generation ‖ number` →
  list, 11 `number` → primary key. Tag 12 maps the first 2,047 bytes of the key to every whole key that begins
  so, each with its number (an exact map, no hashing), and tag 13 holds the last number given. A number is created
  only when the row first gets an entry; removing an entry of a key that never got one does nothing. The search
  scans tags 5 and 9 of each probed list, the in-index count and `check_vector_index` resolve a key through
  `row_ref`, and the cleanup of retired generations frees tags 9 and 10. The number maps go with the tree when the
  index is dropped or the table emptied (`crates/krummi/src/table/catalog.rs:796-800`). Until then, the number of a
  long key whose row was removed stays, so those maps grow with the distinct long keys ever indexed, about twice
  their bytes each.
- Also: `wait_vector_idle` (a test hook) now waits at most 50 ms between checks. A worker that ended between the
  check and the wait made it wait forever, which turned the regression into a hang instead of a failure.
- Test: `table_vector_indexes::codex::r2_long_primary_keys_are_indexed` uses 10,000 rows with string keys plus
  every admitted key from 2,040 bytes on. It builds the index, moves one long row, removes another, checks that
  the worker is alive and `check_vector_index` is empty, and finds each long row through the index. RED before the
  fix (`thread 'krummi-vectors' panicked at crates/krummi-tree/src/lib.rs:1087:5: a key has 1 to 2048 bytes`, then
  `the worker ended`), GREEN after.
- Status: claimed fixed, unverified until a follow-up review.

### R3 — Major: null-vector rows can leave automatic reconstruction permanently active

- Disposition: fixed.
- Fix: the rows with a vector decide an automatic rebuild and its number of lists (`rows_with_vector`), in
  `start_rebuild_if_due` and in `rebuild_vector_index`. A rebuild whose sample cannot train now ends
  (`cancel_rebuild`) instead of staying active.
- Test: `table_vector_indexes::codex::r3_rows_without_a_vector_start_no_rebuild`: 1,000 vectors among 13,000 rows
  start no rebuild and the index stays ready. RED before the fix, GREEN after.
- Status: claimed fixed, unverified until a follow-up review.

### R4 — Major: the broad-filter comparison does not satisfy matched-recall measurement

- Disposition: fixed.
- Cause: the PostgreSQL figures with a filter (recall 1.0, 354 ms) did not come from IVFFlat. psycopg prepares a
  statement after five runs, and PostgreSQL's generic plan for it scanned the table. With `enable_seqscan = off`,
  that plan was still exact.
- Fix: `scripts/bench/pgvector_compare.py` plans each search with its own vector (`prepare_threshold=None`). It also
  records the `EXPLAIN (ANALYZE)` plan of the first search of each filter and mode, and forces IVFFlat with
  `enable_seqscan = off` and `enable_sort = off`. The rerun (`work/ann/pgvector-filters-r4b.md`, 21:11–21:29) shows
  `Index Scan using near on docs` with `Filter: (cat < 50)` in both modes, and the same results in both.
- The closing report now compares points with recall@10 of at least 0.95:
  - `cat < 50`: Krummi 31.49 ms (50 probes, 0.9600) against pgvector 44.32 ms (30 probes, 0.9515) and 54.50 ms
    (40 probes, 0.9685).
  - `cat = 7`: Krummi exact (1.0), 134 ms without an index on `cat` and 8.3 ms with `by_cat`, against pgvector
    134.59 ms (IVFFlat, 120 probes, 0.9765) and 41 ms with its exact scan.
  - It says that, without `by_cat`, Krummi's exact path for `cat = 7` is slower than PostgreSQL's exact scan.
- Status: claimed fixed, unverified until a follow-up review.

### R5 — Minor: the new motor command silently ignores an explicit dataset selection

- Disposition: fixed.
- Fix: `krummi-ann-lab motor` rejects `--conjunto sift` and `--conjunto glove` with «El comando motor solo mide el
  conjunto cohere.», and the usage text says so. `--conjunto cohere` is still accepted, and so is any dataset with
  the other commands.
- Test: `options_a_command_would_ignore_are_rejected` in `crates/krummi-ann-lab/src/main.rs`. RED before the fix
  (the parser accepted `sift`), GREEN after.
- Status: claimed fixed, unverified until a follow-up review.

### Open product questions

Both stay `pending` for the user. They are asked after the review loop, one at a time.

Review record: `docs/reviews/tables-vector-indexes/2026-11-25-code.md`, wrapper status `findings` (open R1–R5,
two open questions), `threadId` `01a21438-f63b-7201-93ae-e3b85960e880`. Telemetry: coordinator `gpt-6.1-sol` /
`high`; reviewer `krummi-review-critical` `gpt-6-astra` / `xhigh`.

Checks after the fixes, at `08a238f`: `cargo fmt --all --check` ok; `cargo clippy --workspace --all-targets -- -D
warnings` clean; `cargo test --workspace` 1,332 passed, 2 failed, 36 ignored. The failures are the known trybuild
`trees_compile` (also on `main`) and `krummi-tx` `group_commit::ac_e13_failures_of_the_journals_writer`. That test is
intermittent: it failed 1 of 20 runs on the branch and 1 of 40 on a copy of the base `7793a51`. The regression tests
of R1, R2 and R3 (`table_vector_indexes::codex`) and of R5 (`options_a_command_would_ignore_are_rejected`) pass.

## 2026-11-25 (verification)

Follow-up report: [2026-11-25-code-02.md](2026-11-25-code-02.md), fix range `daaacd1..5eaa9ba`. Verdict, quoted:
«Approved — R1–R5 verified resolved; no open findings in the follow-up scope.»

- R1: «Verified resolved: unrelated column drop no longer removes a valid vector entry».
- R2: «Verified resolved: the complete supported primary-key domain is indexable».
- R3: «Verified resolved: automatic reconstruction counts non-null vectors».
- R4: «Verified resolved: the filtered comparison now reports comparable measured quality and plans».
- R5: «Verified resolved: motor rejects a dataset it would ignore».

The two product questions stay open for the user.

Review record: `docs/reviews/tables-vector-indexes/2026-11-25-code-02.md`, wrapper status `approved`, `threadId`
`01a2149e-3efa-7992-bc08-c012efd2e929`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.
