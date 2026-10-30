# Multi-value trees code review resolutions (2026-10-30)

Resolutions of [2026-10-30-code.md](2026-10-30-code.md), the review of `d5dc52a..c6097f8`. Append-only: each update
is dated and keeps the earlier ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-10-30

Fix commit: the commit that adds this file (R1). Reviewed head of the original report: `c6097f8`.

### R1 — minor: Oscillating measurement panics when the new sizes leave no initial values

- Disposition: fixed.
- Evidence: the report's scenario reproduced before the fix: `multi_timing --loads oscillating --pairs 512:448
  --values 10 --key-size 64 --value-size 1024` panicked at `multi_timing.rs:692` with `open the tree: TreeNotFound`.
- Fix: `oscillate` creates the tree in a write of its own before adding the values at the threshold, and the loop
  that adds them only opens it, so a load with no values before the threshold still has its tree.
- Regression: the two commands of the report (`--pairs 512:448 --values 10 --key-size 64 --value-size 1024`, and
  `--pairs 512:256 --values 10 --key-size 512 --value-size 8`) now print their row, with 0 values at the threshold,
  and exit 0; `--pairs 2048:1024 --values 1000` still reports 112 values at the threshold. The measurement is run by
  hand and has no automated test (the user's rule: not in CI nor the normal tests).
- Checks: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
  on Windows: 408 passed, 0 failed.
- Status: claimed fixed, unverified until a follow-up review.

Review record: [2026-10-30-code.md](2026-10-30-code.md), wrapper status `findings` (R1 open), thread
`01a18bd2-1446-7d91-9cb6-e6864a10d90a`, coordinator `gpt-6.1-sol`/`high`, reviewer `krummi-review-critical`
`gpt-6-astra`/`xhigh` (telemetry). An earlier attempt the same day failed before reviewing (`Selected model is at
capacity`) and saved no report.

## 2026-10-30 (verification)

Follow-up report: [2026-10-30-code-02.md](2026-10-30-code-02.md), fix range `c6097f8..0f5c8dd`. Verdict, quoted:
"Approved — R1 verified resolved; no open findings in the requested follow-up scope."

- R1: verified resolved.

Review record: wrapper status `approved`, thread `01a18bdd-652a-7400-a4bf-8b40a33cf41c`, coordinator
`gpt-6.1-sol`/`high`, reviewer `krummi-review` `gpt-6.1-sol`/`high` (telemetry).
