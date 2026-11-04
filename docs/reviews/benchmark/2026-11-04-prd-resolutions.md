# Benchmark PRD review resolutions

Resolutions of [2026-11-04-prd.md](2026-11-04-prd.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-04

Artifact versions after the fixes (uncommitted working tree, SHA-256 of the bytes):

- `docs/prd/krummi/benchmark.prd.md`: `e375cffbd2a96756d2134a8658a3f9c561d34fc104f8c08f17947bf00d9a04a6`
- `docs/prd/krummi/benchmark.prd.en.md`: `ecdf815ab4c5d9c7373437ae8df2520601a898571c3a3b15275ff117c646ed24`

### R1 — major: W12 has no defined result when Voron completes zero reads

- Disposition: pending.
- Evidence: it is a product decision (the ratio text, the threshold verdict and the exit code of a load whose Voron
  metric is zero), so the requirement is not edited without the user. The implementation shows `∞` when Krummi's
  metric is positive and Voron's is zero, and `0,00` when both are zero (`crates/krummi-bench/src/measure.rs:45-52`),
  and its threshold is the inequality `5 · krummi ≥ 4 · voron` (`measure.rs:38-40`), which a zero Voron metric
  always meets; the PRD does not fix either choice.
- Status: open, waiting for the user's decision.

### R2 — minor: AC-K1 refers to size-table cells that do not exist

- Disposition: fixed.
- Fix: AC-K1 now asks for `no aplica` in the canopydb and fjall cells of W19 to W22 in the operations table and of
  W19 and W21 in the sizes table, which has no rows for W20 or W22 (K9); same in the translation.
- Status: claimed fixed, unverified until a follow-up review.

### R3 — minor: reduced W17 needs an explicit final partial-batch exception

- Disposition: fixed.
- Fix: K11 now says that, as in a full run, the last write of a workload holds the remaining entries when they do
  not fill a whole write (in reduced W17, a single write of 64 values); same in the translation. Both workers
  already do so (`crates/krummi-bench/src/worker.rs:529`, `Math.Min(count, from + batch)` in
  `crates/krummi-bench/voron/Loads.cs`).
- Status: claimed fixed, unverified until a follow-up review.

Review record: `docs/reviews/benchmark/2026-11-04-prd.md`, wrapper status `findings` (open R1, R2, R3; open question
on R1), `threadId` `01a1a551-888c-7843-b49e-903fce8b549d`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review` `gpt-6.1-sol` / `high`. An earlier attempt the same day (`threadId`
`01a1a53b-8933-7a61-9e12-f7d589e20bcb`) failed on the Codex usage limit and saved no report.
