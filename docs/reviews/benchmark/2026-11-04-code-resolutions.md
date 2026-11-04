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

## 2026-11-04 (verification, round 2)

Follow-up review [2026-11-04-code-02.md](2026-11-04-code-02.md) over `6cef8b1..8972ddf`. The wrapper returned
`error`, because Codex hit its usage limit while writing its final message (`threadId`
`01a1a5a4-656a-75c1-b968-cd9638e3bea9`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`); the report itself is complete and has its verdict, so it is
taken as round 2. Per-ID outcome, quoted:

- R1: "**Still open.** Phase gating removes ordinary pre-start counting, but its boundaries still differ from the
  measured interval."
- R2: "**Verified resolved for the original defect.**"
- R3: "**Partially corrected, still open.** Late interruption now changes success to interruption, but report
  removal can silently fail."
- R4: "**Verified resolved.**"
- New R5 (minor): "The report-folder probe can leave a file after initial validation fails".

Verdict, quoted: "`findings` — changes requested: R1 (major), R3 (minor) and new regression R5 (minor) remain open;
original R2 and R4 are verified resolved."

## 2026-11-04 (round 2 fixes)

Fix commits: `9cb6a49` (R1), `6d35cd5` (R3, R5).

### R1 — major: W12 counting still differs from the writer's measured interval

- Disposition: fixed.
- Fix: the writer has four phases. It takes its start and then publishes `WRITING`; after its last commit it
  publishes `FINISHING`, takes its end and then publishes `ENDED` (also on failure). A reader checks the phase after
  every read: a read it sees end in `WRITING` counts, since it ended after the start was taken and before the end
  was; a read it sees end in `NOT_STARTED` or `FINISHING` gets its end time, and counts when that time is inside the
  writer's start and end; at `ENDED` the reader stops and that read, which ended after the end, does not count. So a
  pause of the writer at either boundary no longer drops reads inside its time, and readers keep reading until the
  end is taken, so the time no longer grows while they are stopped. A read ends when its reader records it, right
  after its value's checksum; the scenario of a read that ends before the start and is recorded during `WRITING` is
  a read recorded inside, by that definition, written in D10. Same in both workers (`worker.rs`, `read_until`,
  `ReaderTally`, `readers_with_writer`; `voron/Loads.cs`, `ReadersWithWriter`).
- Test: `worker::tests::a_w12_reader_counts_exactly_the_reads_that_end_in_the_writers_time`, whose store plays the
  writer's steps with pauses: the start taken at read 3, `WRITING` at read 6, `FINISHING` at read 16, the end taken
  at read 19, `ENDED` at read 22. It expects 16 reads (3–5, 6–15, 16–18) of 23. With the round-1 rule it fails with
  `left: 10, right: 16` (observed); with the fix it passes. The Voron side runs in the end-to-end tests.
- Status: claimed fixed, unverified until a follow-up review.

### R3 — minor: finalization interruption can still leave the report when removal fails

- Disposition: fixed.
- Fix: the report removal of an interrupted run is retried for up to five seconds while it fails, with the same
  helper that already removed the work subfolder (`cli::remove_retrying`, now shared); if it still fails, the
  command says `No se pudo borrar el informe <informe>: <causa>` before `Banco interrumpido.`, as it does for the
  subfolder.
- Test: `cli::tests::a_removal_is_retried_until_it_succeeds` (retries, the last failure returned, a missing path is
  removed) and, on Windows, `cli::tests::a_file_held_for_a_moment_is_still_removed`: a handle opened without
  `FILE_SHARE_DELETE` blocks the removal for 300 ms, and the retried removal then succeeds.
- Status: claimed fixed, unverified until a follow-up review.

### R5 — minor: the report-folder probe can leave a file after initial validation fails

- Disposition: fixed.
- Fix: the probe's removal uses the same retried removal; if it still fails, the check fails with
  `No se puede usar la carpeta <carpeta>: <causa>.` instead of succeeding with the probe left behind.
- Test: the two tests of R3, which cover the shared helper.
- Status: claimed fixed, unverified until a follow-up review.

Checks after the round 2 fixes, at `6d35cd5`: `cargo fmt --all --check` ok; clippy clean; `cargo test --workspace`
518 passed, 0 failed, 30 ignored; `cargo test --release -p krummi-bench -- --include-ignored` with
`KRUMMI_BENCH_RAVENDB`: 40 + 8 + 8 + 9 passed, the eight end-to-end runs with Voron (the changed Voron W12) in
304 s. In WSL 2, `cargo test -p krummi-bench --lib`: 39 passed (the Windows-only test does not run there).

## 2026-11-04 (verification, round 3)

Follow-up review [2026-11-04-code-03.md](2026-11-04-code-03.md) over `8972ddf..c8e52eb`. Wrapper status `findings`
(open R1), `threadId` `01a1a665-8c73-7591-9a9b-2aa40d824334`. Telemetry: coordinator `gpt-6.1-sol` / `high`;
reviewer `krummi-review-critical` `gpt-6-astra` / `xhigh`. Per-ID outcome, quoted:

- R1: "**Partially corrected, still open as minor.** Boundary timestamps fix the demonstrated writer scheduling
  gaps. Cross-processor publication can still permit post-end counting".
- R3: "**Verified resolved for the demonstrated defect.**"
- R5: "**Verified resolved for the demonstrated defect.**"

Verdict, quoted: "`findings` — changes requested; R1 (minor) remains open. R3 and R5 are verified resolved."

## 2026-11-04 (round 3 fixes)

Fix commits: `384e13e` (R1), `c148cdf` (its doc comments, from the RDD review of `384e13e`).

### R1 — minor: W12 can still count reads after the writer's measured end

- Disposition: fixed.
- Fix: the writer publishes `FINISHING` with a sequentially consistent store followed by a full fence, and only then
  takes its end (`worker.rs`, `finish`); the reader's check is `record`. In Voron, `Interlocked.Exchange` (a full
  fence) replaces `Volatile.Write` for `Finishing` (`voron/Loads.cs`, `ReadersWithWriter`). D10 states the fence.
- Test: `worker::tests::no_reader_sees_writing_after_the_writers_end`, a stress test of the clock/phase protocol
  through `finish` and `record`, built like the review's harness: 20,000 rounds; the reader takes the time behind a
  full fence before each check, and a round fails when a `WRITING` was seen with a time after the writer's end. With
  the release store it failed with `left: 74, right: 0` (observed, debug build, Windows x64); with the fix it passed
  four runs of four (three debug, one release). A pass does not prove the order; the test says so. The Voron side
  runs only in the end-to-end tests, as before.
- Status: claimed fixed, unverified until a follow-up review. Round 3 was the last automatic round.

Checks after the fixes, at `384e13e`: `cargo fmt --all --check` ok; `cargo clippy --workspace --all-targets -- -D
warnings` clean; `cargo test --workspace` 519 passed, 0 failed, 30 ignored; `cargo test --release -p krummi-bench
-- --include-ignored` with `KRUMMI_BENCH_RAVENDB`: 41 + 8 + 8 + 9 passed, the eight end-to-end runs with Voron (the
changed Voron W12) in 473 s. At `c148cdf` (comments only): fmt ok, clippy of `krummi-bench` clean, its 41 unit
tests passed.
