# Night optimizations code review resolutions: restored T2

Resolutions of [2026-11-18-code-03.md](2026-11-18-code-03.md). Append-only: each update is dated and keeps the
earlier ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-18

Artifact versions: reviewed range `5a00edc..a4a6bef`; fix in `e66de37`.

### R1 — minor: A Krummi-only A/B does not establish the Voron thresholds

- *Disposition:* fixed.
- *Evidence:* `work/night/w12/ab.sh` runs `AB_BUILDS=not2,t2f` through `work/wm/abq.py`, which runs only the
  `krummi` engine; the bench's worker runs only the Rust engines (`crates/krummi-bench/src/worker.rs`, `rust_engine`),
  so Voron cannot be measured in rounds.
- *Fix:* `e66de37`. L19 now limits the A/B's conclusion to the absence of a T2 regression in that A/B (S5) and keeps
  27 of 29 as the complete run's observed result. To answer K7, the complete Windows run was repeated and run
  again with the code without T2 (L20): with T2, 27 of 29 again (W12 0.60, W13 0.75); without T2, 29 of 29 (W12
  1.19, W13 1.15). On that evidence T2 was reverted in the same commit: `crates/` equals `5a00edc`.
- *Status:* claimed, unverified until a follow-up review.

Review record: [2026-11-18-code-03.md](2026-11-18-code-03.md), wrapper status `findings` (R1), thread
`01a1edfa-ec39-7ce1-af83-981037d337ed`. Telemetry: coordinator `gpt-6.1-sol` with effort `high`, reviewer
`krummi-review-critical` `gpt-6-astra` with effort `xhigh`.

Checks before the follow-up, on `e66de37`: `cargo fmt --all --check` clean, `cargo clippy --workspace --all-targets
-- -D warnings` clean, `cargo test --workspace` 869 passed, 0 failed; `git diff --quiet 5a00edc e66de37 -- crates`
exit 0.

## 2026-11-18 (verification)

Follow-up report: [2026-11-18-code-04.md](2026-11-18-code-04.md), wrapper status `approved`, thread
`01a1ee3e-968d-7981-bbb3-b675b8454b05`. Telemetry: coordinator `gpt-6.1-sol` with effort `high`, reviewer
`krummi-review-critical` `gpt-6-astra` with effort `xhigh`. Verdict: «Approved for the requested follow-up scope: R1
verified resolved; no findings remain open.» R1: resolved.
