# Linux runner — code review resolutions

Report: [2026-11-26-code.md](2026-11-26-code.md).

## 2026-11-26

Artifact version: fix commit `562378e`.

### R1 — major: Checkout cleanup retains untracked nested Git repositories

- *Disposition:* fixed.
- *Fix:* `git clean -qffdx` in `scripts/runner/runner.sh`; test "nested repositories do not survive to the next run"
  in `scripts/runner/tests/test_runner.sh`, seen failing before the change.
- *Status:* claimed, unverified until a follow-up review.

### R2 — minor: Concurrent first calls race during shared repository initialization

- *Disposition:* fixed.
- *Fix:* the initialization takes its own waiting lock, `init.lock`, apart from the run lock so that a call during a
  run does not fail; test "simultaneous first calls both succeed", seen failing before the change.
- *Status:* claimed, unverified until a follow-up review.

### Recorded checks

- `bash scripts/runner/tests/test_runner.sh`: 0 failures.
- `bash scripts/runner/tests/Test-InvokeLinux.sh`: 0 failures.
- Cargo checks not rerun: no Rust or Cargo file changes in the range.

## Review record

- [2026-11-26-code.md](2026-11-26-code.md): wrapper status `violation`, `threadId` `01a2174b-6b12-74e0-8325-fdb1a589b281`;
  coordinator `gpt-6.1-sol` high, `krummi-review` `gpt-6.1-sol` high, `krummi-review-critical` `gpt-6-astra` xhigh.
  The unexpected change was `.atl/skill-registry.md`, an ignored index that the user's Codex start hook
  (`gentle-ai skill-registry refresh`) writes, new in this fresh clone. Nothing was reverted; the user accepted
  that cause and authorized continuing.

## 2026-11-26 (verification)

Report: [2026-11-26-code-02.md](2026-11-26-code-02.md). Wrapper status `approved`, `threadId`
`01a2175a-0bde-7f12-be3f-fcffd0ea7295`; coordinator `gpt-6.1-sol` high, `krummi-review-critical` `gpt-6-astra` xhigh.

- R1: "verified resolved, not merely accepted from Claude's claimed checks."
- R2: "verified resolved."
- Verdict: no finding open in scope.
