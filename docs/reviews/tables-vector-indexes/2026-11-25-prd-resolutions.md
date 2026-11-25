# Vector indexes PRD review resolutions

Resolutions of [2026-11-25-prd.md](2026-11-25-prd.md). Append-only: each update is dated and keeps the earlier ones.
A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-25

Review record: [2026-11-25-prd.md](2026-11-25-prd.md), wrapper status `findings` (R1 to R5 open), thread
`01a21182-4035-7c21-8c8a-c7e73e777416`. Telemetry:
- coordinator `gpt-6.1-sol` / `high`;
- reviewer `krummi-review-critical` (Tesla) `gpt-6-astra` / `xhigh`.

Artifact version after the fixes (uncommitted, SHA-256 of the bytes):
- `docs/prd/krummi/tables/tables-vector-indexes.prd.md`: `f0f8287db1085618261133a0f579dd9e1d076655fefe49f58a63f397eccc7074`

### R1 — blocker: Persistent index writes are classified as unchanged transactions

- Disposition: fixed.
- Fix:
  - VI3: a batch is the worker's own write, a change for T10 and journaled like any other. It changes no table rows,
    and as another transaction it does not end the references of the application's transactions (A10).
  - *Validaciones*: searches and the index information are not changes for T10. Requesting a rebuild is, as analyze
    in a write is (ST7), because it stores the request.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — blocker: The k-result guarantee includes rows whose vectors cannot be returned

- Disposition: fixed.
- Fix:
  - VI6 defines the rows that **pass** as those that satisfy the filter and have a non-null vector (VI1). They count
    for the threshold and for k, and k rows come back whenever at least k pass.
  - AC-VI13 adds the case: k rows satisfy the filter, one of them with a null vector, so k − 1 rows come back and
    that row does not count for the threshold.
- Status: claimed fixed, unverified until a follow-up review.

### R3 — major: Dropping a missing index is both a no-op and an error

- Disposition: fixed.
- Fix: dropping a vector index that does not exist does nothing, as in IX4. "Borrar" was removed from the
  missing-index error row.
- Status: claimed fixed, unverified until a follow-up review.

### R4 — major: Index indicators have no defined output when unavailable or undefined

- Disposition: fixed.
- Fix: VI11 makes an indicator that cannot be computed `ninguno`, like the number of lists:
  - all three indicators, with no ready generation;
  - the fraction of out-of-range codes, in a generation with no entries;
  - the largest list against the mean, when every list is empty.

  A `ninguno` indicator never triggers a rebuild. AC-VI13 covers a ready index whose rows are all deleted one by
  one.
- Status: claimed fixed, unverified until a follow-up review.

### R5 — minor: The starting state overlooks existing background threads

- Disposition: fixed.
- Fix: the starting state now says that, of the tables, the store has only the statistics thread. The other threads
  of the store, the journal's and the transfer thread, do not take the write turn.
- Status: claimed fixed, unverified until a follow-up review.

Open questions of the report: all three are answered by decisions the user already took, so none is pending.
- The no-op drop follows IX4, under the user's "Aceptar todas (Recomendado)" for following 7.2.
- The threshold counts the rows with a non-null vector, which follows from VI1.
- The indicators are `ninguno` when they cannot be computed, which follows from VI11's existing "o ninguno si no hay".

## 2026-11-25 (verification)

Follow-up report: [2026-11-25-prd-02.md](2026-11-25-prd-02.md), wrapper status `approved`, no finding open, thread
`01a2118b-6596-72e3-b5b3-630199393393`. Telemetry:
- coordinator `gpt-6.1-sol` / `high`;
- reviewer `krummi-review-critical` (Tesla) `gpt-6-astra` / `xhigh`.

Verdict, quoted: "Ready for implementation within the requested follow-up scope: R1–R5 are verified resolved, with no
open findings or identified fix-induced regressions."
