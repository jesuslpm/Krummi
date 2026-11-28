# Single-table queries PRD review resolutions

Resolutions of [2026-11-28-prd.md](2026-11-28-prd.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-28

Review record: [2026-11-28-prd.md](2026-11-28-prd.md), wrapper status `violation`, `threadId`
`01a22217-b8f1-7421-a0b4-d60ad9a4e3e3`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` (Mill) `gpt-6-astra` / `xhigh`. The violation listed `odd/tasks/three-way-split-phases.tsv`
and `refs`: another session of the user, running a tree experiment in the same working copy, updated its phase log
and committed `b7aef2d8`, `520baece` and `0df204ab` (12:21 UTC) during the review. Codex changed nothing but its
report, which its own baseline section records. Nothing was reverted. The user decided to handle the report's
findings as round 1 and to continue with follow-ups ("Sigue").

Artifact versions after the fixes (uncommitted working tree, SHA-256 of the bytes):

- `docs/prd/krummi/queries/queries-single-table.prd.md`: `890335927f319fc0977114f0d15039cdf12ad2f4f4402ac494eadc483c3f51e9`

### R1 — blocker: AC-CU13 requires committing after a fatal store-read error

- Disposition: fixed.
- Fix: AC-CU13 now separates the query-local failures (a budget, the deadline, a cancellation, a spill failure),
  after which the write commits its change, from a damage or an I/O failure while reading the store, after which the
  write can no longer commit (T11), as CU18 says.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — major: Normalization does not define a total order for compound expressions

- Disposition: fixed.
- Fix: CU7 now compares two nested `and` (or two `or`) by their already sorted arguments one by one, with the same
  rules, up to the first that differs; when one is a prefix of the other, the shorter goes first. With the leaf order
  (column name bytes, operation, `not` right after its operation, parameter number) and `and` before `or`, the order
  is total. The example `OR(AND(eq(a,p0), eq(b,p1)), AND(eq(a,p0), eq(c,p2)))` sorts by the second arguments, `b`
  before `c`.
- Status: claimed fixed, unverified until a follow-up review.

### R3 — major: The subnormal rule conflicts with nearest-even rounding

- Disposition: fixed.
- Fix: the float row of the value forms now rounds every JSON number to the nearest value of the type with ties to
  even, also between the subnormals and zero: only a value below half the smallest subnormal, or exactly at that
  half, gives zero, and `1e-45` in an `f32` gives the smallest subnormal, as the report's probe observed.
- Status: claimed fixed, unverified until a follow-up review.

### R4 — major: Floating-point aggregate admissibility lacks a usable accuracy contract

- Disposition: pending.
- Reason: which accuracy the product promises for floating-point `sum` and `avg` across plans (any IEEE order, or a
  result independent of the plan) changes behavior and cost. It is a product decision, kept for the user after the
  loop.
- Status: pending.

### R5 — major: Canonical output leaves the order of bind.columns unspecified

- Disposition: fixed.
- Fix: CU5 now says Krummi writes `bind.columns` in the order of the bytes of the column names, which does not depend
  on the schema order or on the order of use; the reference documents of AC-CU1 and AC-CU14 follow it.
- Status: claimed fixed, unverified until a follow-up review.

### R6 — major: store/1 column descriptors leave existing type attributes unresolved

- Disposition: fixed.
- Fix: CU20 now gives `store/1` column descriptors of its own that carry the attributes the schema already stores:
  `{"type": "uuid", "order": "rfc9562" | "sql_server"}`, `{"type": "string", "collation": "binary" | "ci" |
  "ci_ai"}` and `{"type": "vector", "element", "dim", "cosine": true | false}`, plus `{"type": "json"}` and CU5's
  descriptors for the types without further attributes. Parameter descriptors stay CU5's, where a `uuid` or a
  `string` matches any order or collation.
- Status: claimed fixed, unverified until a follow-up review.

### R7 — minor: The starting-state description implies a nonexistent temporary-sort cap

- Disposition: fixed.
- Fix: the starting-state row now says index creation sorts with 64 MiB before spilling to `krummi.sort`, and that in
  temporary mode it does not spill and keeps sorting in memory without that cap until the system cannot give more
  and it fails with out of memory (IX2). The new query budget in temporary mode stays CU17's.
- Status: claimed fixed, unverified until a follow-up review.

## 2026-11-28 (verification)

Review record: [2026-11-28-prd-02.md](2026-11-28-prd-02.md), wrapper status `findings` (R4 open), `threadId`
`01a22220-1fec-7261-b9a9-a6c93eb1d09e`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` (McClintock) `gpt-6-astra` / `xhigh`.

- R1, R2, R3, R5, R6, R7: "verified resolved".
- R4: open, pending the product decision.

Verdict, quoted: "Not ready for implementation. Six findings are verified resolved; R4 remains open pending the
product's floating-point aggregate contract."

## 2026-11-28 (R4 decided)

Artifact versions after the fix (uncommitted working tree, SHA-256 of the bytes):

- `docs/prd/krummi/queries/queries-single-table.prd.md`: `8f231055c20d73d17675bce8fb5021e397d6cc63fb6637529ae7df064ed5d372`

### R4 — major: Floating-point aggregate admissibility lacks a usable accuracy contract

- Disposition: fixed, after the user's decision. User: "Exacta y sin depender del plan (Recomendado)".
- Fix: CU12 now defines floating-point `sum` as the exact sum of all values rounded once to the nearest `f64`
  with ties to even, independent of the plan and of the summation order: it gives ±∞ only when that exact result
  does not fit in an `f64`, a NaN among the values gives NaN, and +∞ with −∞ gives NaN. `avg` of integers and
  floats is the exact sum divided by the count, rounded once to the nearest `f64`, with the same NaN and infinity
  rules. The exception to the admissible-result rule (CS7), its annotation to the parent and the 10⁻⁹ tolerance of
  CU23 are removed, so AC-CS1 compares these aggregates exactly. AC-CU7 now covers cancellation, intermediate
  overflow, NaN and infinities, equal under any plan. How the exact sum is computed is left to the implementation.
- Status: claimed fixed, unverified until a follow-up review.

## 2026-11-28 (verification 2)

Review record: [2026-11-28-prd-03.md](2026-11-28-prd-03.md), wrapper status `findings` (R4 open), `threadId`
`01a222b5-b6b8-7a20-ac6c-7a1c9afa4921`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer `krummi-review`
(Chandrasekhar) `gpt-6.1-sol` / `high`.

- R4: open. Verdict, quoted: "Not ready for implementation. R4 remains open because avg does not specify its rounding
  tie breaker."

### R4 — major: Floating-point aggregate admissibility lacks a usable accuracy contract (follow-up)

- Artifact version after the fix: `docs/prd/krummi/queries/queries-single-table.prd.md`: `baf677eabc80d943b0c982ca93143feea1d2807593ae1d29f5481d636c17ba92`
- Disposition: fixed.
- Fix: CU12 now says the floating-point and integer `avg` is rounded once to the nearest `f64` with ties to even,
  as `sum`, which is the rule the decision already fixed for `sum` (IEEE 754 default rounding). A wording fix, no
  new product decision.
- Status: claimed fixed, unverified until a follow-up review.

## 2026-11-28 (verification 3)

Review record: [2026-11-28-prd-04.md](2026-11-28-prd-04.md), wrapper status `approved`, `threadId`
`01a222b9-445c-7883-91a4-2c508e1d9e4f`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer `krummi-review`
(Bohr) `gpt-6.1-sol` / `high`.

- R4: verified resolved.

Verdict, quoted: "Ready for implementation within the requested follow-up scope. R4 is verified resolved; no findings
remain open."
