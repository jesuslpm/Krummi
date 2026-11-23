# Collation follow-up code review resolutions

Resolutions of [2026-11-23-code.md](2026-11-23-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-23

Artifact versions: the review covered `9974d0a..154dfec`; the fix is `9dc01a7` on `feat/tables-collation`.

### R1 — minor: Restore document links after reopening the execution

- Disposition: fixed.
- Fix: `9dc01a7` commits the execution documents as `move_docs.py reopen` rewrote them: the product-source
  link of `odd/tasks/tables-collation-report.md` and the review and resolutions links of the log of
  `odd/tasks/tables-collation.md` use `../../docs/...`. The cause: `3373bc7` took the renames that `reopen` had
  staged, without the rewritten links. Check: every `../../docs/` link of both documents resolves from `odd/tasks`.
- Status: claimed fixed, unverified until a follow-up review.

Review record: [2026-11-23-code.md](2026-11-23-code.md), wrapper status `findings` (R1), thread
`01a20887-159b-74a3-98ec-45860c07fad0`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.

## 2026-11-23 (verification)

Follow-up report: [2026-11-23-code-02.md](2026-11-23-code-02.md), wrapper status `approved`, thread
`01a20896-4eb7-7940-a394-4542f1fe714b`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer `krummi-review`
`gpt-6.1-sol` / `high`. Verdict: "Approved: R1 verified resolved; no open findings in the requested scope."
