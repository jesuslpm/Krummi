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

## 2026-11-06 (verification)

Follow-up report: [2026-11-06-code-02.md](2026-11-06-code-02.md), wrapper status `findings`, threadId
`01a1b1eb-a4d1-7eb2-abdc-0c14ab102002`. The telemetry gives the requested settings: coordinator `gpt-6.1-sol` /
`high`, reviewer `krummi-review` `gpt-6.1-sol` / `high`.

- **R1, missing single-load runs:** "Windows component verified resolved". The WSL pairs were "not fully verified",
  because the reviewer could not read the WSL result files. They are now copied to `work/o4/wsl-before.tsv` and
  `wsl-after.tsv` (ignored by git), whose last rows hold every pair of L11.
- **R1, thresholds:** "Still open". Pending for the user.

The loop stops after two rounds: a third could only repeat that the threshold part waits for the user's decision.

## 2026-11-06 (user decision)

### R1 — blocker: O4 does not satisfy its required performance acceptance

- Disposition: resolved by a user decision. Asked whether to accept O4 as an intermediate step and clarify K13 that
  way, or keep it open until its targets meet their threshold, the user answered "Sí".
- Fix: K13 gets an annotation of 2026-11-06 (`docs/prd/krummi/benchmark.prd.md` and its translation). It lets an
  optimization without a PRD close as an intermediate step, even though its target loads do not meet their
  threshold yet, when single-load runs before and after measure what it improves. The threshold remains the goal of
  the next optimizations, and the condition that no other load gets worse does not change. The plan's
  *Measurement method* says the same.
- Evidence: O4's single-load runs before and after are L7 and L11 of `odd/tasks/commit-copies.md`. The threshold
  work continues as O5–O8 of `docs/performance/plan.md`, from the diagnosis of 2026-11-06.
- Status: claimed, unverified until a follow-up review.
