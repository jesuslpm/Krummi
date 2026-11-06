# Fewer zeroed buffers in the commit (O4) code review resolutions

Resolutions of [2026-11-06-code.md](2026-11-06-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-06

Artifact versions: reviewed range `13489d1..47ce11c`; the missing single-load runs recorded in L11 of
[commit-copies.md](../../../odd/tasks/commit-copies.md), committed after `47ce11c`. No code change.

### R1 — blocker: O4 does not satisfy its required performance acceptance

The finding has two parts.

- **The single-load runs omitted W1, W3, W13, W15 and W19.** Disposition: fixed. Evidence: L11 of
  `odd/tasks/commit-copies.md`: two rounds of each, Windows and WSL 2, `before` = `13489d1` and `after` = `cf61713`
  (the binaries of L7), with the order swapped in the second round. W13 runs in W12's process (`worker.rs:206`, `651`),
  so it was measured as W12, and W12 is reported too. Ratios after / before range from 0.958 to 1.109, except WSL
  W15's first round, 0.843, whose reps scatter by 25 %; its second round is 1.043.
- **None of the nine target loads meets its threshold, which K13's annotation of 2026-11-04 sets as the
  acceptance.** Disposition: pending, a product decision. The figures in the report are right
  (`docs/benchmark/2026-11-06-*.md`), and the annotation reads as quoted. Whether O4 closes as a step toward the
  thresholds, as O1, O2 and O3a did (plan History: "no threshold met yet"), or stays open, is the user's call. The
  requirement was not edited.

Status: claimed, unverified until a follow-up review.

Review record: [2026-11-06-code.md](2026-11-06-code.md), wrapper status `findings`, threadId
`01a1b19d-f226-7b80-87d0-e91b622b9a35`. The telemetry gives the requested settings: coordinator `gpt-6.1-sol` /
`high`, reviewer `krummi-review-critical` `gpt-6-astra` / `xhigh`. The report says the effective settings were not
independently exposed. An earlier attempt at 11:36 UTC ended in `error` (Codex's usage limit) with no report.
