# Resolutions: code review of tables-statistics-followups

Report: [2026-11-20-code.md](2026-11-20-code.md). Range reviewed: `134c6f2..7bcc9db`.

## 2026-11-20

Artifact versions: fix commit `e2a12bd`.

### R1 — Major: the new damage test does not exercise the write sampling path required by S1d

- *Disposition:* fixed.
- *Evidence of the defect:* with only `krummi_tx::WriteTransaction::run_unchecked` changed to check the run
  (`crates/krummi-tx/src/transaction.rs`, `run_checked_or_not(first, count, false)`), the old
  `sampling_in_a_write_skips_the_runs_checksum` still passed: the write had not changed the table, so
  `WriteTransaction::sample_built` sampled the committed table through a read.
- *Fix (`e2a12bd`):* the test adds a column to the table in the write before estimating, so the committed table no longer has every
  column of the write's schema and the estimate samples the write's own view (`build_own`, ST3). Under the same
  write-only mutation the test now fails; with the code as it is, it passes. `cargo fmt --all --check` and `cargo
  clippy --workspace --all-targets -- -D warnings` pass; `cargo test --workspace`: 59 binaries, 1,030 passed, 0 failed, 35 ignored.
- *Status:* claimed, unverified until a follow-up review.

Review record: report `docs/reviews/tables-statistics-followups/2026-11-20-code.md`, wrapper status `findings`,
`threadId` `01a1fa3f-e474-7ac2-a21a-bac572c332e8`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.
