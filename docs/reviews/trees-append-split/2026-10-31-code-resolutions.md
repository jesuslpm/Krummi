# Append split code review resolutions

Resolutions of [2026-10-31-code.md](2026-10-31-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-10-31

Review record: report `2026-10-31-code.md`, wrapper status `findings`, thread `01a19149-616f-7911-a047-db8ac3a44511`.
Executed settings from the wrapper telemetry: coordinator `gpt-6.1-sol` / `high`; one `krummi-review-critical`
reviewer `gpt-6-astra` / `xhigh`.

Fix commit on `feat/trees-append-split`: `275ecef` (R1).

### R1 — minor: Exhaustive AC-V8 sweep fails after the workload adjustment

- Disposition: fixed (`275ecef`).
- Reproduction: `cargo test -p krummi --lib --release tests::ac_v8_a_failure_at_every_page_operation_with_values_off_their_leaf_applies_nothing -- --ignored --exact`
  failed at `298ec81` with `131 points` (`crates/krummi/src/tests.rs:588`), as the report says.
- Cause: the sweep's starting tree `t` (12 entries added in increasing order by `create_trees`) now has full
  leaves, so the write reads and frees fewer pages. Changing only the order of the write's 8 new keys did not
  help: in decreasing order it still reached 131. Building the starting trees in both orders made the other
  sweeps fall under their minimums (AC-B18 667 of 700, AC-R12 464 of 500), so it was reverted.
- Fix: the write of `values_off_their_leaf` adds twelve keys (`12..24`) in both orders (`in_both_orders`) instead
  of eight; `values_off_their_leaf_committed` checks 20 entries ("0 to 3, 7, 8, 10 to 23") and key 23. The
  minimum of 140 is unchanged.
- Measured margin (the minimum raised temporarily to read the count, then restored): the exhaustive sweep reaches
  158 points; the base `86614c4`, built from a `git archive` copy in a temporary directory, reaches 156.
- Checks: `cargo test -p krummi --lib --release -- --include-ignored` 24 passed, 0 failed (the exhaustive and
  sampled AC-B18, AC-R12, AC-V8 and AC-M14 included). The full checks of `CLAUDE.md` and the CI's ignored tree
  tests are recorded in the execution document (`odd/tasks/trees-append-split.md`).
- Status: claimed fixed, unverified until a follow-up review.

## 2026-10-31 (verification)

Review record: report [`2026-10-31-code-02.md`](2026-10-31-code-02.md) (follow-up, `-FixRange 298ec81..a5b02d9`),
wrapper status `approved`, thread `01a19199-409a-73c1-a705-27321dabc81d`. Executed settings from the wrapper
telemetry: coordinator `gpt-6.1-sol` / `high`; one `krummi-review` reviewer `gpt-6.1-sol` / `high`.

- R1: "verified resolved". The report notes that "the exact claimed 158-point count and 156-point historical-base
  measurement were not independently remeasured; the unmodified strict guard independently establishes more than
  140 points in both modes, which suffices to resolve R1."
- Verdict, quoted: "Approved for the requested follow-up scope: R1 is verified resolved; no open findings remain."
