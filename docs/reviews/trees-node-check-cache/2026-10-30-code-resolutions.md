# Remembered node layout check code review resolutions

Resolutions of [2026-10-30-code.md](2026-10-30-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-10-30

Review record: report `2026-10-30-code.md`, wrapper status `findings`, thread `01a18cd9-9d10-7141-9077-eaeeec8c5414`.
Executed settings from the wrapper telemetry: coordinator `gpt-6.1-sol` / `high`; one `krummi-review-critical`
reviewer `gpt-6-astra` / `xhigh`.

Fix commits on `feat/trees-node-check-cache`: `161c0e4` (R1), `f95c57f` (R2).
Checks after the fixes: `cargo fmt --all --check` passed; `cargo clippy --workspace --all-targets -- -D warnings`
passed; `cargo test --workspace` 421 passed, 0 failed (420 before, plus the new R1 test).

### R1 — major: Cache allocation can abort the process when opening a store

- Disposition: fixed (`161c0e4`).
- Fix: the table is reserved through the pager's fallible `try_reserve` helper, the same one the layers above use
  for their memory (P13), so `Checked::new` returns the out-of-memory class instead of aborting, and both openings
  (on disk and temporary) propagate it. The reservation is also an `Op::Alloc` fault point, like every other
  fallible reservation of the engine. New test
  `check_tests::opening_without_memory_for_the_table_fails_with_out_of_memory`: with a table of `usize::MAX / 8`
  slots, which no system can provide, `Store::open` and `Store::open_temporary` fail with
  `ErrorKind::OutOfMemory`, and the store on disk opens afterwards with its tree intact.
- RED observed before the fix: the new test panicked with `capacity overflow` in `raw_vec`.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — major: AC-C8 can pass without overlapping readers, writes and transfers

- Disposition: fixed (`f95c57f`).
- Fix: `ac_c8_concurrent_reads_with_competing_slots` now coordinates the threads. Each reader counts its rounds,
  each round reading every node (the catalog, the branch and leaves of `good` for every key, and the damaged
  leaf), and keeps reading until it has done 1,000 rounds and the writer has finished. Before the first commit,
  after each commit and after each transfer, the writer waits until every reader has finished two more rounds, so
  one of them started after that step; it fails if a reader stops early. After each transfer it asserts that no
  committed version is left in the scratch area (`scratch_bytes() == 0`) while the readers are still reading.
  At the end each reader must have seen all 21 generations (`g0` to `g20`), and done at least 1,000 rounds.
- RED: not applicable; this is an acceptance coverage gap, not an engine defect, and the old test passed.
- Cost: the test now takes 18.5 s in a debug build on this Windows machine (both stores); its earlier time was
  not measured.
- Status: claimed fixed, unverified until a follow-up review.

## 2026-10-30 (verification)

Review record: follow-up report [2026-10-30-code-02.md](2026-10-30-code-02.md) of `fc57e47..6b095ff`, wrapper
status `findings`, thread `01a18cf1-e3a6-7c73-a359-9fe773b93063`. Executed settings from the wrapper telemetry:
coordinator `gpt-6.1-sol` / `high`; one `krummi-review-critical` reviewer `gpt-6-astra` / `xhigh`.

Verdict quoted: "Requires changes — R3 remains open (one minor finding). R1 and R2 are verified resolved."

- R1: verified resolved.
- R2: verified resolved.
- R3 (new, minor): AC-C8 hangs instead of reporting a reader or writer failure.

## 2026-10-30 (round 2 fixes)

Fix commit on `feat/trees-node-check-cache`: `8cf01cf` (R3).
Checks after the fix: `cargo fmt --all --check` passed; `cargo clippy --workspace --all-targets -- -D warnings`
passed; `cargo test --workspace` 421 passed, 0 failed.

The RDD review of `fc57e47..6b095ff` (lineage `review-e32dc28366105d7c`, lenses resilience and reliability) was
approved before this round and reported the same defect as two non-blocking warnings,
`R3-ac-c8-hangs-on-failure` and `R4-reader-loop-hangs-on-writer-failure`; this fix covers them too.

### R3 — minor: AC-C8 hangs instead of reporting a reader or writer failure

- Disposition: fixed (`8cf01cf`).
- Fix: a guard on the writer's thread sets a `failed` flag when it drops while the thread panics, which happens
  as the scope closure unwinds and before `thread::scope` waits for the readers; the readers stop as soon as they
  see it. A reader's failure reaches the writer through `two_rounds` (a finished reader) or `join`, so either way
  the writer panics and the guard stops the others.
- Reproduction after the fix, with the report's once-only reader panic inserted temporarily before
  `let mut seen = BTreeSet::new();`: `FAILED`, exit 101, "finished in 0.05s". With a temporary
  `assert!(generation < 3, "injected writer failure")` after the writer's commit: `FAILED`, exit 101,
  "finished in 0.08s". Both injections were removed; the report shows that the first one hung before the fix.
- Status: claimed fixed, unverified until a follow-up review.
