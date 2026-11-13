# Table row reads PRD review resolutions

Resolutions of [2026-11-11-prd.md](2026-11-11-prd.md). Append-only: each update is dated and keeps the earlier ones. A
claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-11

Artifact versions: the reviewed draft `docs/prd/krummi/tables/tables-row-reads.prd.md` of the commit "docs(prd): draft the
table row reads PRD"; fixes in the next commit on branch `feat/table-row-reads`.

### R1 — major: a thread panic does not guarantee process termination

- Disposition: fixed per the user's decision. Asked after the loop; the user answered «Pánico normal del hilo
  (Recomendado)»: an ordinary Rust panic of the reading thread, which ends that thread, or the process if it is the
  main one, and which the application may catch.
- Fix: L2 states it, and that the guarantee holds in builds that unwind on panic; a build that aborts on panic ends
  the process.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — major: the fate of a write retained across panic capture is unspecified

- Disposition: fixed per the user's decision, «Sigue intacta (Recomendado)»: the panic happens when the column is
  read, without changing the store or the transaction; a captured panic leaves the transaction intact, and a write can
  go on and commit; a panic that unwinds through the transaction drops it as abandoned (T10), freeing a write's turn.
- Fix: L2 and AC-L5 (both capture boundaries, with the commit and the turn outcomes).
- Status: claimed fixed, unverified until a follow-up review.

### R3 — minor: the claimed unchanged error-page attribution is inaccurate

- Disposition: fixed.
- Evidence: the finding holds; a version missing from the history reports the history leaf that should hold it.
- Fix: AC-L3 keeps that: the row's page for an unknown version, a header Krummi does not write and a short fixed part;
  the history page for a version missing from the history; all as today.
- Status: claimed fixed, unverified until a follow-up review.

### R4 — minor: starting state presents future operations as implemented behavior

- Disposition: fixed.
- Fix: the bullet now says that backup, compaction, migration and store verification belong to future modules (10–12
  of the root) and that the module 7 parent asks them to treat table pages as tree pages; this PRD does not change
  that.
- Status: claimed fixed, unverified until a follow-up review.

Review record: [2026-11-11-prd.md](2026-11-11-prd.md), wrapper status `findings`, thread
`01a1cbe4-6bba-7181-8ee3-28cf90acc3fc`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.

## 2026-11-11 (verification)

Follow-up report: [2026-11-11-prd-02.md](2026-11-11-prd-02.md), fix range `HEAD~1..HEAD` at `bf8f6d8`, thread
`01a1cbf3-8e84-78c1-967a-a9a5b8b117c4`, wrapper status `findings`. R1, R2 and R4 verified; R3 reopened as major: L1
still required the row's page for a version missing from the history while AC-L3 required the history page.

### R3 — major, follow-up: L1 and AC-L3 name different pages for the same error

- Disposition: fixed.
- Fix: L1 now splits the failures as AC-L3 does: the row's page for an unknown version, a header Krummi does not write
  and a short fixed part; the page of the history that should hold it for a version missing from the history.
- Status: claimed fixed, unverified until a follow-up review.

Review record: telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer `krummi-review-critical` `gpt-6-astra` /
`xhigh`.

## 2026-11-11 (third round verification)

Follow-up report: [2026-11-11-prd-03.md](2026-11-11-prd-03.md), fix range `bf8f6d8..77325f6`, thread
`01a1cbf8-ed39-7c91-9a2e-e156ba9af7ac`, wrapper status `approved`. No finding and no open question remains in this
chain. Review record: telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer `krummi-review-critical` `gpt-6-astra` /
`xhigh`.
