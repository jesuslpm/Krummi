# Benchmark code review resolutions

Resolutions of [2026-11-04-code.md](2026-11-04-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-04

Fix commits on `feat/benchmark`, after the reviewed head `6cef8b1`: `74c8697` (R1), `6afd038` (R2), `6c5ebd1` (R3),
`dcd39f8` (R4).

### R1 — major: W12 counts reads outside the writer's measured interval

- Disposition: fixed.
- Fix: the writer of W12 and W13 has a phase (not started, writing, done). It publishes *writing* after taking its
  start time and *done* before taking its end time, also on failure. A reader checks the phase after every read
  and counts the read only when it is *writing*; at *done* it stops without counting it. So every counted read ends
  inside the time the writer reports, in both workers (`crates/krummi-bench/src/worker.rs`, `read_until` and
  `readers_with_writer`; `crates/krummi-bench/voron/Loads.cs`, `ReadersWithWriter`). The Rust writer now reports its own
  start and end instead of the times `parallel` takes around it. A read that ends between the writer's start time
  and its publication of *writing*, or between *done* and its end time, is not counted: those windows are two
  adjacent instructions, and they only undercount. Design D10 of the execution document says the same.
- Test: `worker::tests::a_w12_reader_counts_only_the_reads_that_end_while_the_writer_writes`, with a store whose
  reads drive the phase: 10 reads before the writer starts, 10 while it writes, then the end; it expects 10 counted
  reads and 21 reads in all. With the previous counting it fails with `left: 20, right: 10` (observed). The Voron
  side has no unit tests; the end-to-end runs exercise it (see the checks below).
- Status: claimed fixed, unverified until a follow-up review.

### R2 — major: initial report validation accepts an unwritable destination folder

- Disposition: fixed.
- Fix: `check_report` now creates a new file with a name of its own (`.krummi-bench-<pid>.tmp`) in the report's
  folder and removes it; when it cannot, it fails with `No se puede usar la carpeta <carpeta>: <causa>.` and code
  `1`, before the RavenDB check, as the validation table orders (`crates/krummi-bench/src/cli.rs`,
  `can_create_in`).
- Test: `cli::tests::the_report_folder_must_accept_a_new_file`: the check leaves the folder empty; on Unix, a
  folder with mode `0o555` fails with that message and code (skipped when the user can write anyway, as a
  superuser). Before the fix it failed in WSL 2 at `cli.rs:312` (observed); after it, it passes in WSL 2 and on
  Windows. Windows has no automated case for a folder without write permission: it would need an ACL change, which
  the tests do not make.
- Status: claimed fixed, unverified until a follow-up review.

### R3 — minor: Ctrl+C during finalization can preserve the report and return success

- Disposition: fixed.
- Fix: once the work subfolder is removed, `settle` checks Ctrl+C again: a successful run interrupted while it was
  finishing removes the report it wrote (created as a new file, so it is its own) and ends as `Banco interrumpido.`
  with code `1`; a failure keeps its message (`crates/krummi-bench/src/orchestrator.rs`).
- Test: `orchestrator::tests::ctrl_c_while_finishing_removes_the_report_and_interrupts`: without Ctrl+C the result
  and the report stay; with it the report is removed and the result is the interruption; a failure keeps its code.
  The test did not compile before the fix (`settle` did not exist), so its RED is the missing behavior, not an
  observed wrong result. A Ctrl+C after this last check, while the summary is printed, still ends as a finished run.
- Status: claimed fixed, unverified until a follow-up review.

### R4 — minor: the integration test rejects the valid infinite-ratio result

- Disposition: fixed.
- Fix: `a_reduced_run_writes_its_report_and_leaves_no_store` accepts a ratio of two decimals with a comma or `∞`,
  and checks the decimal form exactly (digits, a comma, two digits) instead of only looking for a comma
  (`crates/krummi-bench/tests/run.rs`).
- Test: the same test, run with `KRUMMI_BENCH_RAVENDB`: passed (99 s). The `∞` case depends on scheduling and was
  not forced.
- Status: claimed fixed, unverified until a follow-up review.

Review record: `docs/reviews/benchmark/2026-11-04-code.md`, wrapper status `findings` (open R1–R4), `threadId`
`01a1a582-15f8-7c20-8453-25006d622777`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`. The review's seven end-to-end Voron tests stopped on a NuGet
restore error (`NU1301`) in its sandbox; here they run with network access.

Checks after the fixes, at `dcd39f8`: `cargo fmt --all --check` ok; `cargo clippy --workspace --all-targets -- -D
warnings` clean; `cargo test --workspace` 516 passed, 0 failed, 30 ignored; `cargo test --release -p krummi-bench
-- --include-ignored` with `KRUMMI_BENCH_RAVENDB`: 38 + 8 + 8 + 9 passed, the eight end-to-end runs with Voron
(which build the changed Voron program and run W12) in 362 s.
