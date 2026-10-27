# Trees scan code review resolutions

Resolutions of [2026-10-26-code-02.md](2026-10-26-code-02.md), the original report of this chain. Append-only:
each update is dated and keeps the earlier ones. A claimed fix stays unverified until a follow-up review confirms
it.

The earlier run [2026-10-26-code.md](2026-10-26-code.md) reported the same R1 and R2, but the wrapper flagged it as
a violation: `.atl/.skill-registry.cache.json`, the skill registry's fingerprint cache, changed during the run
after the skills changed on `main`. The guard now leaves that cache out (`261b539` on `main`, merged into the
branch in `f72d826`), and the review was repeated from scratch as this report.

## 2026-10-26

Review record: [2026-10-26-code-02.md](2026-10-26-code-02.md), wrapper status `findings`, `threadId`
`01a17973-effe-7511-836b-b5e3c5c46b0c`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`. Range reviewed: `0e204da..760eb0c`.

### R1 — major: Exhausting one scan end leaves the other able to fail and invalidate a write

- Disposition: fixed in `f2bb44d`.
- Fix: in `Range::guard` (`crates/krummi-tree/src/range.rs`), an end that returns nothing more now stops both ends
  (`stop()`), so the other end never seeks, reads or reserves after the scan ended. An end only stops when no entry
  is left between it and the other end's last one or the bound, so the other end had nothing left either.
- Evidence: `crates/krummi/tests/tree_memory.rs`
  `r1_an_exhausted_scan_ends_at_both_ends_without_reserving` reproduces the report's sequence in both directions
  (one pending entry, one end consumed to `None`, the next reservation failing, then the other end): RED before
  the fix (`front first: true`, the other end returned an error), GREEN after: the other end returns `None` and the
  write commits with its tree. `tree_memory` 14, `tree_scans` 21, `krummi --lib` 3 (2 ignored) and `krummi-tree`
  15 tests pass; clippy clean.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — minor: The short-scan benchmark does not always measure 100 entries per scan

- Disposition: fixed in `52eef05`.
- Fix: `crates/krummi/examples/scan_timing.rs` takes each short scan's start from the sorted keys with at least 100
  entries from them to the end, and asserts that each scan returns 100 (or every entry, for runs of fewer than 100).
- Evidence: rerun on 1,000,000 keys, the workload reads exactly 1,000,000 entries on both systems
  (`10000 scans of 100 from random keys | 1000000`): Windows 0.09 s, Linux (WSL 2) 0.10 s. The full tables are in
  the closing report and in `odd/tasks/trees-scan.md` (L16).
- Status: claimed fixed, unverified until a follow-up review.

## 2026-10-27 (verification)

Review record: [2026-10-27-code.md](2026-10-27-code.md), wrapper status `approved`, `threadId`
`01a17c13-249a-7bc1-84bb-fcdecd02e913`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`. Fix range: `760eb0c..990689b`.

- R1: "verified resolved — Exhausting one scan end leaves the other able to fail and invalidate a write".
- R2: "verified resolved — The short-scan benchmark does not always measure 100 entries per scan".
- Verdict: "Approved: R1 and R2 are verified resolved; no findings remain open in the requested follow-up scope."
