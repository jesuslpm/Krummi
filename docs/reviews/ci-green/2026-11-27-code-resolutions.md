# ci-green code review resolutions

Resolutions of [2026-11-27-code.md](2026-11-27-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-27

Reviewed range: `0b7c4992..508057df` (branch `fix/ci-green`).

Review record: [2026-11-27-code.md](2026-11-27-code.md); wrapper status `violation`, with `unexpectedChanges`
`.atl/skill-registry.md`; `threadId` `01a21d94-9b9c-7512-8ce2-f726dd6c1a1f`; telemetry: coordinator `gpt-6.1-sol` /
`high`, reviewer `krummi-review-critical` `gpt-6-astra` / `xhigh`. The report's verdict: "Approved — no open
findings introduced by the reviewed range."

The `violation` did not come from Codex. The `UserPromptSubmit` hook of the user's `~/.claude/settings.json` runs
`gentle-ai skill-registry refresh` on every prompt, and a prompt sent at 15:19 UTC, during the review (15:16–15:30
UTC), rewrote `.atl/skill-registry.md` (modified 15:19:03 UTC). The Codex run log (`events.jsonl` in
`%TEMP%\codex-review\20261127-151646-ci-green-code`) never names the file. Nothing was reverted.

### R1 — minor (pre-existing advisory): the pause helper does not acknowledge an already-dispatched transfer

- Disposition: pending, as separate later work outside this change.
- Evidence: confirmed by reading the code. `Signal::next_work` returns `Work::Pass` after releasing the signal
  mutex (`crates/krummi-pager/src/transfer.rs:239`), and `Shared::pass` takes `flushing` only afterwards
  (`crates/krummi-pager/src/scratch.rs:763`), so `set_transfer_paused(true)` (`crates/krummi-pager/src/scratch.rs:593`)
  can return between the two. Commit `3816274c` narrows the window AC-E13 failed in on CI (job 140398583970, where
  the `krummi-transfer` thread took the injected panic), but does not close it. Closing it needs the transfer thread
  to acknowledge the pause of a dispatched pass, a change to a helper shared by every test that pauses the transfer.
  AC-E13 passed 20 of 20 local runs after `3816274c`.
- Status: open.
