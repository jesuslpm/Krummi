# Cheaper write path (O5, O6, O7) code review resolutions

Resolutions of [2026-11-07-code.md](2026-11-07-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-07

Artifact versions: reviewed range `b9514cf..b5739cd`; the documentation fixes and the W22 measurements of L18 of
[write-path.md](../../../odd/tasks/write-path.md), committed after `b5739cd`. No code change.

### R1 — blocker: AC-S8 is not met on WSL 2

Disposition: pending, a product decision. The figures are right: W2 −2.5 % / +6.9 % and W21 −2.9 % / −3.2 % in
`work/o5/o7-wsl.out`, as L12 records. The diagnostic repeat (`work/o5/repeat-wsl.out`: W2 +2.0 % / +2.4 %, W21
+0.9 % / +4.7 %) does not replace the rule's rounds, and L12 already says so. Whether to accept the result as it
stands, measure again or amend AC-S8 is the user's call. The requirement was not edited.

Status: pending.

### R2 — major: O5 is marked complete while two explicit S1 copy removals remain

Disposition: pending for the scope (a product decision), and fixed for the records. The retained copies are the ones
L6 lists with their reasons: the raw payload, because LZ4 compresses one contiguous block, and the group copy when
several records share a write, because laying them out in place across threads is a concurrency redesign. T1 of
`write-path.md` now reads "done, partial", and the plan's history row for O5 now says which copies stay. Whether O5
closes as a partial step, with those removals tracked, is the user's call.

Status: claimed, unverified until a follow-up review (records); pending (scope).

### R3 — major: the range has a confirmed Windows W22 performance regression

Disposition: pending, a product decision, with the cause narrowed. A binary built from `f7b19d8` (O6 alone) was
measured against O5 and O7 on one store, the W21 store of O5, in six rounds with rotating order
(`work/o5/cross.py`, output in `work/o5/cross-w22.out`). Medians: O5 3,840,880, O6 3,891,757, O7 3,728,840 op/s. O6
is +1.3 % over O5, and O7 is −4.2 % under O6, lower in all six rounds. O7 does not change what a read transaction
does: `Snapshot::integrity()` already returned `UseRemembered` for readers, and O7 makes that a constant
(`crates/krummi-pager/src/scratch.rs`). So the drop is in how that code compiles, and finding its mechanism needs a
profile. The earlier cross test of L13 had only shown that the store does not explain it. Fixing or deferring it is
the user's call; the plan's history row says so.

Status: pending.

### R4 — minor: the performance history states the wrong lifetime for writer integrity checks

Disposition: fixed. The plan's history row now reads "a writer checks a changed node's layout once per change and
uses the page integrity remembered while the store is open".

Status: claimed, unverified until a follow-up review.

### Check failure: AC-J9 recovery test

Advisory, not a finding. Codex's full workspace run failed once in
`recovery::ac_j9_a_failed_transfer_refuses_writes_and_recovery_brings_everything_back` and passed on both reruns.
Codex traces it to a race in the test, which the range does not change. It is recorded in L18 and in the closing
report.

Review record: [2026-11-07-code.md](2026-11-07-code.md), wrapper status `findings`, threadId
`01a1b4a6-57be-7f30-8216-a0680146f7b9`. The telemetry gives the requested settings: coordinator `gpt-6.1-sol` /
`high`, reviewer `krummi-review-critical` `gpt-6-astra` / `xhigh`. The report says the effective settings were not
independently exposed.

Checks before the follow-up, at `06f933b`: `cargo fmt --all --check` ok; `cargo clippy --workspace --all-targets -- -D
warnings` clean; `cargo test --workspace` 41 suites, 555 passed, 0 failed, 30 ignored (AC-J9 passed in this run).

## 2026-11-07 (verification)

Follow-up report: [2026-11-07-code-02.md](2026-11-07-code-02.md), wrapper status `findings`, threadId
`01a1b4b5-3bdd-7c83-a988-902908ae7db1`. The telemetry gives the requested settings: coordinator `gpt-6.1-sol` /
`high`, reviewer `krummi-review` `gpt-6.1-sol` / `high`.

- **R1:** "Still open: AC-S8 remains unmet on WSL 2." Pending for the user.
- **R2:** "Records verified corrected; implementation/scope gap remains open." Scope pending for the user.
- **R3:** "Still open: the Windows W22 regression remains. Additional diagnostic arithmetic is confirmed; its
  mechanism is unproven." Pending for the user.
- **R4:** "Verified resolved by `06f933b`; no longer an open finding."

The loop stops after two rounds: a third could only repeat that R1, R2 and R3 wait for the user's decisions.
