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

## 2026-11-27 (R1 fix)

Fix commits on branch `fix/transfer-pause`: `03f0b2ef` (fix and its test), `b9dd9078` (probe of the independent
verification).

### R1 — minor (pre-existing advisory): the pause helper does not acknowledge an already-dispatched transfer

- Disposition: fixed.
- Fix: `SignalState::passing` (`crates/krummi-pager/src/transfer.rs`) is set by `next_work` under the signal mutex
  when it returns `Work::Pass`, and cleared by `after_work`, which then wakes `answered`. `Signal::set_paused(true)`
  waits on `answered` while a pass is taken and the thread is alive; `Alive::drop` already clears `alive` and wakes
  `answered` when the thread ends by a panic, as in AC-E13.
- Checks: `transfer::tests::pausing_waits_for_a_pass_already_taken` failed before the fix ("the pause waits for the
  pass") and passes after it; `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`
  clean; on Linux (`work/runner/20261127T174157.168Z-03f0b2efc1c5-run`) AC-E13 passed 200 of 200 runs and
  `cargo test --workspace` passed at `03f0b2ef`; on Windows `cargo test --workspace` at `03f0b2ef` failed only
  `worker::ac_vi3_the_index_follows_the_writes` and `worker::ac_vi15_the_lists_of_large_tables` on their 120 s limits
  while another `cargo test` ran on the machine, and the four `worker::ac_vi*` tests then passed alone in 75 s.
  Independent verifier: PASS on "no pass taken before the pause writes after it", "the pause never hangs" and
  "production behavior unchanged". RDD review `review-c3f511c408f7ac2d` (resilience, reliability) approved.
- Status: claimed fixed, unverified until a follow-up review.

## 2026-11-27 (verification)

Review record: [2026-11-27-code-02.md](2026-11-27-code-02.md), follow-up of `508057df..886e4be8`; wrapper status
`approved`; `threadId` `01a21e26-094d-7b30-aac0-bc8a2bf61365`; telemetry: coordinator `gpt-6.1-sol` / `high`,
reviewer `krummi-review-critical` `gpt-6-astra` / `xhigh`. Verdict: "Approved — R1 verified resolved; no open
findings or regressions in the requested follow-up scope."

- R1: "verified resolved at the reviewed head by the two fix commits above."
