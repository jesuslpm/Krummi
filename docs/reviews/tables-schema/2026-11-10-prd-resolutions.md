# Tables with schema PRD review resolutions

Resolutions of [2026-11-10-prd.md](2026-11-10-prd.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-10

Artifact versions: the reviewed PRD at `dab30ed`; fixes in commits `af09c4d` (PRD 7.1), `c84ba23` (annotations to the
affected PRDs and their translations) and `8ef8ecd` (design documents), on branch `docs/tables`.

### R1 — blocker: bulk-load rollback promises extend beyond the commit point of no return

- Disposition: fixed.
- Evidence: the finding holds; E2 of `journal-group-commit.prd.md` fixes a point of no return, and E4 says the
  durability wait cannot be cancelled.
- Fix (`af09c4d`): E13 splits the load in two phases. While loading rows, before committing, a failing row or source,
  a cancellation or a panic aborts it whole, and a crash discards it. Its commit follows the rules of any write: past
  the point of no return it stays committed, and recovery keeps it when it was durable (G1, G2, G5; E2 and E4 of the
  group commit). AC10 tests the first phase.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — blocker: AC10 requires secondary indexes before they can be created

- Disposition: fixed.
- Fix (`af09c4d`): AC10 uses a table with rows and no indexes and says 7.2 adds the indexed case; E13 admits indexes
  "desde 7.2".
- Status: claimed fixed, unverified until a follow-up review.

### R3 — major: adding a nonnullable column to an empty table has two readings

- Disposition: fixed as a consistency defect, not a product question.
- Evidence: parent N3 lists the admitted additions as "añadir una columna que admite nulos" and "añadir una columna
  con un valor por defecto"; a nonnullable column without a default is in neither, whatever the table holds.
- Fix (`af09c4d`): E4 says adding a nonnullable column needs a default "tenga filas la tabla o no", citing N3; E5 and
  AC4 already applied the rule unconditionally.
- Status: claimed fixed, unverified until a follow-up review.

### R4 — major: error precedence does not break ties within one column

- Disposition: fixed, extending the rule the user approved for several violations ("se informa la primera regla en
  el orden de la lista").
- Fix (`af09c4d`): E17's rule list is ordered (repeated column, missing key column, null not admitted, type, malformed
  text, domain, ...), and a row reports its first column in schema order (a repeated column in the order given) and,
  for it, the first rule of the list. AC14 adds the review's example.
- Status: claimed fixed, unverified until a follow-up review.

### R5 — minor: promised annotations have not been added to the affected documents

- Disposition: fixed.
- Fix: `c84ba23` adds a dated block "Anotaciones del 2026-11-10 desde las tablas con esquema" before the starting
  state of `tables.prd.md` (N1, AC-N5, N4, N6, N10, AC-N3, AC-N4, AC-N7, out of scope), `trees-keys.prd.md` (A1, A3,
  A6, A7, A9, A10), `trees.prd.md` (B3, B7) and `benchmark.prd.md` (load table, K4, K7 and output, K9), and to their
  English translations. `8ef8ecd` adds dated notes to design documents 1 (domains), 2 (sequences instead of the
  autonumeric counter, the point of no return), 3 (`SqlGuid` vectors), 11 (P7) and the README (sequences, emptying).
- Status: claimed fixed, unverified until a follow-up review.

### R6 — minor: Reuse lists an operating-system random function that does not yet exist

- Disposition: fixed.
- Fix (`af09c4d`): Reuse says no such function exists and 7.1 adds it to the pager (P20), shared by the generators
  and the catalog identities.
- Status: claimed fixed, unverified until a follow-up review.

Open questions of the report: both are answered by the fixes above (R3 by parent N3; R4 by the ordered rule list,
which follows the user's approved principle). No product question remains for the user.

Review record: [2026-11-10-prd.md](2026-11-10-prd.md), wrapper status `findings`, thread
`01a1c5c8-9d7d-7d61-87cc-607d0d590214`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.

## 2026-11-10 (verification)

Follow-up report: [2026-11-10-prd-02.md](2026-11-10-prd-02.md), range `dab30ed..f765311`, thread
`01a1c5d5-34df-7ec3-9bb1-d3cdf7b64269`, wrapper status `findings`, verdict "Not ready for implementation: R4 remains
open." R1, R2, R3, R5 and R6 verified resolved; R4 open as a regression of its fix: the column selector mixed schema
order and input order, shown with schema `id, a, b` and the row `[(b,2),(b,3),(id,1),(a,NULL)]`.

### R4 — major, follow-up: error selection mixes schema order and input order

- Disposition: fixed. The finding holds.
- Fix (`baf036e`): E17 reports one violation in three steps: (1) a repeated column, the first repeated in input
  order; (2) else a missing key column, the first in key order; (3) else the first column in schema order that breaks
  another rule, with its first rule in the list's order. AC14 adds the review's mixed row (repeated `b`), the original
  example and a missing-key case.
- Status: claimed fixed, unverified until a follow-up review.

Review record: telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer `krummi-review-critical` `gpt-6-astra` /
`xhigh`.
