# Tables statistics code review resolutions

Resolutions of [2026-11-19-code.md](2026-11-19-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-19

Fix commits: `b93f241` (R1, R3–R12, R14, R15) and `32af503` (R13), on `feat/tables-statistics`. Checks after each:
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`
pass. Regression tests: `crates/krummi/tests/table_statistics/codex.rs` and
`crates/krummi/src/table/stats_fault_tests.rs`.

### R1 — blocker: Sampling damage does not invalidate the requesting write

- Disposition: fixed.
- Fix: in `WriteTransaction::sample_built`, a failure of the auxiliary read's sample and of opening its table goes
  through `self.inner.fail`, so the write cannot commit (T11).
- Test: `r1_damage_while_sampling_in_a_write_fails_the_write`.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — blocker: Sampling reads and verifies entire runs containing large values

- Disposition: pending (not fixed in this round).
- Evidence of the gap: it is task T7b of the execution document (L4, L7); a sampled row stored in a run is still read
  whole. It needs a run read by pages without the run checksum in `krummi-tx`, a leaf visit that gives the run
  without resolving it in `krummi-tree`, and a row decoder over the pages read; and the ST14 page-count hook.
- Status: open.

### R3 — major: Private statistics survive emptying and override newer analyzed statistics

- Disposition: fixed.
- Fix: `Shared::seen` picks the newest of saved, memory and a write's private statistics, leaving out private ones
  built before an emptying the write sees; `WriteTransaction::column_statistics` uses the same rule.
- Test: `r3_a_writes_own_statistics_end_when_it_empties_the_table`.
- Status: claimed fixed, unverified until a follow-up review.

### R4 — major: Shortened histogram bounds can turn a 2,500-row interval into zero

- Disposition: fixed.
- Fix: a shortened bucket bound is never taken as inside the range and may always meet it, so the bucket counts half.
- Test: `r4_shortened_bounds_keep_the_interval_estimate` (Codex's distribution, within 500 rows).
- Status: claimed fixed, unverified until a follow-up review.

### R5 — major: Joint estimates do not retry saving statistics held only in memory

- Disposition: fixed.
- Fix: every lookup of an estimate, of a column or an index prefix, read or write, goes through `Shared::seen`, which
  asks to save statistics that are only in memory (`one_target` builds the request for a prefix too).
- Test: none of its own; the save retry is tested for columns in
  `a_failed_background_save_keeps_memory_and_is_asked_again`, through the same function.
- Status: claimed fixed, unverified until a follow-up review.

### R6 — major: Save polling loses its FIFO position every 20 milliseconds

- Disposition: fixed.
- Fix: `Engine::write_unless` waits in the queue with one ticket, asking a stop check every 20 ms, and leaves only
  when the store closes; the background save uses it (`Shared::open_write_unless`).
- Test: none of its own; the existing close and turn tests (`st10_…`, `ac_st7_…`) pass through it.
- Status: claimed fixed, unverified until a follow-up review.

### R7 — major: Store analyze silently loses its save after a table rename

- Disposition: fixed.
- Fix: analyze on the store and the background save find the sampled table by identity
  (`worker::find_write_table`), also renamed.
- Test: `r7_analyze_saves_for_a_table_renamed_while_it_sampled`.
- Status: claimed fixed, unverified until a follow-up review.

### R8 — major: Builds from a write's own view scan the entire large table

- Disposition: fixed.
- Fix: `WriteTransaction::build_own` reads whole up to 30,000 rows and samples above, with `sampled_rows`, which reads
  and writes share; analyze in a write and the private sample use it.
- Test: `r8_analyze_in_a_write_samples_a_large_table`.
- Status: claimed fixed, unverified until a follow-up review.

### R9 — major: Private index-prefix statistics are discarded between estimates

- Disposition: fixed.
- Fix: a write's private statistics are keyed by target, columns and index prefixes.
- Test: `r9_a_writes_own_prefix_statistics_serve_it_again`.
- Status: claimed fixed, unverified until a follow-up review.

### R10 — major: Estimating a table that replaces another object fails with its old type

- Disposition: fixed.
- Fix: a committed object of another type is absence of the table; the write samples its own view.
- Test: `r10_a_table_that_replaced_a_tree_estimates`.
- Status: claimed fixed, unverified until a follow-up review.

### R11 — major: Shared statistics are not retired or checked against committed lifecycle changes

- Disposition: fixed.
- Fix: a commit retires from memory the statistics of the tables it emptied or deleted (`finish_tables` returns
  them, `StatsMemory::retire`).
- Test: `r11_statistics_end_with_a_committed_emptying` (Codex's zero-row case).
- Status: claimed fixed, unverified until a follow-up review.

### R12 — major: Closing cannot cancel an active background sample

- Disposition: fixed.
- Fix: a renewal reads its sample with `build_statistics_until`, which stops the row scan and the leaf sampling
  (`sample_leaves` now lets its visitor stop it) as soon as the store closes.
- Test: none of its own; `st10_close_drops_a_sample_and_waits_for_a_save_with_the_turn` passes through it.
- Status: claimed fixed, unverified until a follow-up review.

### R13 — major: Required failure-injection acceptance coverage is missing

- Disposition: partly fixed.
- Fix: `stats_fault_tests.rs` injects failures through the engine's page faults: a background save that fails keeps
  the statistics in memory and a later estimate saves them; analyze on the store reports disk full and input/output
  with their classes, keeps what it published, and the store still accepts writes. R1's test covers damage in a
  write's sample. Still missing: a commit input/output failure that makes the store refuse writes (T12) in the
  background save, and damage during a renewal.
- Status: claimed partly fixed, unverified until a follow-up review.

### R14 — minor: Frequent-combination ties ignore the index's direction and null order

- Disposition: fixed.
- Fix: prefix combinations are encoded, ordered and decoded with each key column's direction and null order.
- Test: `r14_ties_follow_the_index_order`.
- Status: claimed fixed, unverified until a follow-up review.

### R15 — minor: Validation is interleaved instead of following ST12 precedence

- Disposition: fixed.
- Fix: an equality in several columns checks every name's size before the columns; analyze on the store checks the
  column name's size before the catalog.
- Test: `r15_name_sizes_come_before_the_columns`.
- Status: claimed fixed, unverified until a follow-up review.

Review record: [2026-11-19-code.md](2026-11-19-code.md), wrapper status `findings` (R1–R15 open), thread
`01a1f446-5ae1-70e1-b8d0-f8a49d9e3b1c`. Telemetry: coordinator `gpt-6.1-sol`/`high`; reviewer
`krummi-review-critical` `gpt-6-astra`/`xhigh`.

## 2026-11-19 (verification)

Follow-up report: [2026-11-19-code-02.md](2026-11-19-code-02.md). Verdict, quoted: "Changes required. R1, R2, R4,
R9, R11, R12, R13 and R15 remain open: two blockers, five major findings and one minor finding. R3, R5, R6, R7, R8,
R10 and R14 are verified resolved; approval is withheld."

Review record: [2026-11-19-code-02.md](2026-11-19-code-02.md), wrapper status `findings`. Telemetry as round 1.

## 2026-11-19 (round 2 fixes)

Fix commits: `05d3d7f` and `f1fc1a0` (`05d3d7f` was committed before its clippy check finished; the next commit adds
an `expect` with its reason to `build`). Checks after them: fmt, clippy and `cargo test --workspace` pass.

- R1 — fixed: opening the auxiliary snapshot also fails the requesting write (`self.inner.fail`). Test:
  `r1_a_failed_auxiliary_snapshot_fails_the_write` (Codex's `Op::Snapshot` out-of-memory injection).
- R2 — pending: the partial read of runs (T7b) is not done.
- R4 — fixed: a shortened bound is the shortest prefix, of at least 64 bytes, greater than the previous bucket's
  bound, so bounds keep their order and the estimate treats them as exact. Test: `r4_…` now uses Codex's
  distribution (100 suffixes) and its four intervals `050..099`, `090..099`, `000..099`, `100..199`.
- R9 — fixed: `WriteTransaction::index_statistics` sees the write's own prefixes, with the newest-wins and emptying
  rules. Test: `r9_a_write_reads_its_own_prefix_statistics`.
- R11 — fixed: each committed emptying, deletion, schema change or index change bumps the table's memory epoch and
  retires its entries; a build carries the epoch of its start and is neither published nor saved with an older one.
  Test: `r11_a_build_from_before_an_emptying_is_dropped` (Codex's zero-row case). A save that gets the turn between the
  emptying's commit and its retirement can still save a zero-row build, which then still describes the table.
- R12 — fixed: a cancelled build stops gathering and returns nothing.
- R13 — partly fixed, as in round 1; still missing: a background commit input/output failure that makes the store
  refuse writes (T12) and damage during a renewal.
- R15 — fixed: write estimate, statistics reads and analyze check the handle's transaction, then the names, then a
  deleted handle. Test: `r15_a_deleted_handle_comes_after_the_names`.
- Status: claimed, unverified until a follow-up review.
