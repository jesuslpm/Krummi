# Tables PRD review resolutions

Resolutions of [2026-11-10-prd.md](2026-11-10-prd.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-10

Artifact versions: the reviewed PRD at `eb1801c`; the fix in commit `44bf49f` on branch `docs/tables`.

### R1 — major: AC-N8 presupposes whole-load atomicity, still delegated to 7.1

- Disposition: fixed.
- Evidence: the finding holds. AC-N8 required that no row of a failed load be counted, while 7.1 decides whether a
  load fails whole (AC-N3 is already conditional). The clause came from the fourth round of an external design
  review the same day (H18), which assumed the whole-load rule.
- Fix: AC-N8 now says the stored statistics count only the rows of the load that stayed committed as 7.1 fixes
  (none if the load fails whole). Design document 3's test list says the same (commit `1e276f9`).
- Status: claimed fixed, unverified until a follow-up review.

### R2 — minor: the announced annotations to other PRDs are missing

- Disposition: fixed in the PRD's wording; the annotations themselves are applied when the PRD is closed.
- Evidence: the PRD said the affected documents "llevan las anotaciones correspondientes", which was false. The plan
  of this PRD's run applies them after its review, together with the translation and the elaboration time (CLAUDE.md
  puts the cross review before those steps), and the user has not yet approved closing it. Some of them also depend
  on product decisions still open in design document 11 (the D12 rule for dependencies, modules 17 and 18).
- Fix: the context and the *Anotaciones a otros PRD* section now say the annotations are pending and are applied, each
  dated, when the PRD is closed. The list gains the scalar-read exception to D11 and AC20 that the review found
  missing.
- Status: claimed fixed, unverified until a follow-up review. If the reviewer holds that the umbrella cannot be
  approved before the annotations exist, that is a sequencing question for the user, not a wording defect.

Review record: [2026-11-10-prd.md](2026-11-10-prd.md), wrapper status `findings`, thread
`01a1c4ad-2da5-7f80-a875-539af1296113`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.

## 2026-11-10 (verification)

Follow-up report: [2026-11-10-prd-02.md](2026-11-10-prd-02.md), thread `01a1c4c5-f9e2-7122-b0dd-a381d4d6750f`. Wrapper
status `violation`: the pre-existing untracked `odd/tasks/tasks.zip`, outside the review's scope and not created by
Claude, disappeared during the run; the report states that no review command deleted it and that it was not restored.
Nothing was reverted. Per-ID outcome, quoted from the report:

- R1: "verified resolved".
- R2: "still open". The reviewer holds that a pending checklist does not satisfy the requirement to annotate the
  affected documents, and that the table/catalog, scalar-read and benchmark annotations do not depend on document 11's
  open decisions. Applying them is the closing of this PRD, which waits for the user's approval.

## 2026-11-10 (annotations applied)

The user approved closing R2 by applying the annotations and authorized a fourth round.

### R2 — minor: the announced annotations to other PRDs are missing

- Disposition: fixed.
- Fix (commit `b471cc4`): each affected PRD and its English translation carries a block "Anotaciones del 2026-11-10
  desde las tablas" before its starting state: `krummi.prd.md` (objective 2 and D4, D2, D11 and AC20, D12 and AC21
  with the dependency rule, AC19, out of scope; and, before *Validaciones y mensajes*, modules 7, 10–12 and the new
  17–19), `benchmark.prd.md` (K4, K6, K7, K11, K13, Output), `trees.prd.md` (B3, B4, B7, B8, B11, out of scope),
  `trees-keys.prd.md` (A1, A3, A6, A7, A9, A10) and `transactions.prd.md` (T4, T12). `tables.prd.md` now says the
  documents carry them.
- Status: claimed fixed, unverified until a follow-up review.
