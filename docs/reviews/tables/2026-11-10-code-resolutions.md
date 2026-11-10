# Tables commit-range review resolutions

Resolutions of [2026-11-10-code.md](2026-11-10-code.md), the review of `ddb6716..eb1801c` (the tables PRD and design
series). Append-only: each update is dated and keeps the earlier ones. A claimed fix stays unverified until a
follow-up review confirms it.

## 2026-11-10

Fix commits on branch `docs/tables`, range `eb1801c..7235f8f`: `44bf49f` (PRD), `07fdcfa` (doc 1), `1e276f9`
(doc 3), `70b6038` (doc 5), `5fd8554` (doc 8), `7235f8f` (doc 11). The range changes documents only; no checks
apply beyond reading.

### R1 — major: failed-load statistics criterion assumes whole-load atomicity

- Disposition: fixed. Same finding as R1 of [2026-11-10-prd.md](2026-11-10-prd.md); see its resolution.
- Fix: `44bf49f` (AC-N8) and `1e276f9` (doc 3's test list): only the rows of the load that stayed committed as 7.1
  fixes are counted, none if the load fails whole.
- Status: claimed fixed, unverified until a follow-up review.

### R3 — major: proposed vector arithmetic produces NaN for valid finite vectors

- Disposition: fixed.
- Evidence: the finding holds; the review compiled and ran the cases.
- Fix (`70b6038`, and the norm's size in doc 1 by `07fdcfa`): every floating kernel converts each element to `f64`
  and accumulates in `f64`. The product of two `f32` values is exact in `f64`; `f32::MAX`² times 4,096 terms is below
  `f64::MAX`; the smallest nonzero `f32` product (about 2e-90) does not underflow in `f64`; `i8` kernels keep the
  exact `i32` sum and scale in `f64`. So admitted vectors give finite distances and a positive finite norm. The norm
  is stored as `f64` (8 bytes) and distances are `f64`. The golden test adds the review's three cases and asserts no
  NaN or infinity. The cost (up to about twice the arithmetic time) is an estimate to measure.
- Status: claimed fixed, unverified until a follow-up review.

### R4 — major: primary-key-only storage cannot provide the promised zero-copy bytes reads

- Disposition: fixed.
- Evidence: the finding holds: a `bytes` key `41 00 42` is `01 41 00 FF 42 00` in the key.
- Fix (`07fdcfa`): `string` and `bytes` primary key columns are also stored in the row, so they read without a
  copy; fixed-width primary key columns stay only in the key, since the PRD returns scalars by value. Doc 1's open
  question 9 is settled accordingly.
- Status: claimed fixed, unverified until a follow-up review.

### R5 — major: maximum-size primary keys generate bounds rejected by the reused trees

- Disposition: fixed.
- Fix (`1e276f9`): when a bound's base `B` is already 2,048 bytes, no stored key extends it, so `B ‖ FF` as a start
  becomes `B` excluded and as an excluded end becomes `B` included. Every bound passed to the tree is at most 2,048
  bytes and selects the same keys.
- Status: claimed fixed, unverified until a follow-up review.

### R6 — major: spill sort keys exceed the reused trees' key limit

- Disposition: fixed.
- Fix (`5fd8554`): a run is sorted in memory before it spills and appended to a fixed-size tree keyed by its position
  in the run (`u64`), with the encoded sort key and the row as the value; the merge compares decoded sort keys in
  memory. No sort key is a tree key any more. The reserved index spool notes the same constraint for later.
- Status: claimed fixed, unverified until a follow-up review.

### R7 — major: decided row design contradicts the accepted schema and vector rules

- Disposition: fixed.
- Fix (`07fdcfa`): dropping a column is allowed only outside the primary key and every index, and fails without
  effect otherwise (N3, AC-N6, AC-N7); allowing nulls in a `NOT NULL` column outside the primary key is a catalog-only
  change (N3, AC-N6); a `bit` column's dimension is a multiple of 8 (N4), in the layout and in question 5.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — minor: required cross-document annotations are declared complete but missing

- Disposition: fixed in the PRD's wording; see R2 of [2026-11-10-prd-resolutions.md](2026-11-10-prd-resolutions.md).
- Status: claimed fixed, unverified until a follow-up review.

### R8 — minor: differential oracle requires equality for valid tied windows

- Disposition: fixed.
- Fix (`7235f8f`): doc 11's differential tests use document 10's admissibility rule against the reference plan; a
  generated query that wants equality adds the primary key to its sort keys.
- Status: claimed fixed, unverified until a follow-up review.

Review record: [2026-11-10-code.md](2026-11-10-code.md), wrapper status `findings`, thread
`01a1c4b8-243c-7da2-8e90-437cb62ac8e0`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.

## 2026-11-10 (verification)

Follow-up report: [2026-11-10-code-02.md](2026-11-10-code-02.md), thread `01a1c4cb-efe8-7de2-9b00-a128226cd5fb`,
wrapper status `findings`, verdict "Changes requested: R4 and R6 remain major; R2 remains minor." R1, R3, R5, R7 and
R8 were verified resolved; R4 and R6 "still open, partially corrected"; R2 "still open".

## 2026-11-10 (second fixes)

### R4 — major, follow-up: index-only reads still cannot return variable primary keys without copying

- Disposition: fixed. The finding holds and is wider than stated: a `string` or `bytes` column of the index key is
  escaped in the key too, not only the primary key.
- Fix (`docs/design/tables/03-keys-and-indexes.md`, `04-query-tree-and-planner.md`, `06-c-interface-and-csharp.md`):
  an index-only read returns only fixed-width key columns from the key; `string` and `bytes` columns of the index key
  or the primary key come from an included column or from the row (doc 1 stores `string` and `bytes` primary key
  columns raw). Doc 4's covering rule no longer counts them; the C and C# accessors read them from the row.
- Status: claimed fixed, unverified until a follow-up review.

### R6 — major, follow-up: spill runs still violate the reused tree's value constraints

- Disposition: fixed. The finding holds: fixed-size trees have one constant value length of at most 2,048 bytes and
  no values off their leaf (X1); the first fix chose the wrong tree kind.
- Fix (`docs/design/tables/08-execution-plans.md`): a run is a normal tree keyed by its position, a big-endian `u64`
  (8 bytes, appended in order), with the encoded sort key and the row as a value of any length, a long one going off
  its leaf by the normal tree's large-value rule up to B6's maximum.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — minor

- Disposition: pending the user's approval to close the tables PRD, which applies the annotations (see the PRD
  resolutions).

## 2026-11-10 (third round verification and fixes)

Follow-up report: [2026-11-10-code-03.md](2026-11-10-code-03.md), thread `01a1c4d4-f724-7251-832e-44f63b900804`,
wrapper status `findings`, verdict "Changes requested: R6 remains major; R4 and R2 remain minor." This was the third
automatic round, the limit of the codex-review loop; a further round needs the user's authorization.

### R6 — major, third round: combined spill records can exceed the normal tree's value limit

- Disposition: fixed. The finding holds (a 100 MiB `bytes` row and its 200 MiB escaped sort key exceed B6 together).
- Fix (`docs/design/tables/08-execution-plans.md`): a run record is the row alone, as the table holds it, so it fits
  wherever the row fits; the merge decodes the sort columns from each row in memory and compares typed values, as the
  in-memory sort does; a computed sort key or projected value goes in a second small tree keyed by the same position.
- Status: claimed fixed, unverified until a follow-up review.

### R4 — minor, third round: corrected zero-copy rules still conflict with dependent descriptions

- Disposition: fixed: doc 3's primary key and covering paragraphs, doc 2's `rows` table entry and doc 4's batch
  description now follow the fixed-width / `string` and `bytes` distinction.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — minor

- Disposition: unchanged, pending the user's approval to apply the annotations.

## 2026-11-10 (annotations applied)

R2: fixed by commit `b471cc4`; see [2026-11-10-prd-resolutions.md](2026-11-10-prd-resolutions.md). The user authorized
a fourth round to verify R2, R4 and R6.

## 2026-11-10 (fourth round verification and fixes)

Follow-up report: [2026-11-10-code-04.md](2026-11-10-code-04.md), status `findings`, verdict "Changes requested: one
major and two minor findings remain open." R2 verified resolved; R6 and R4 still open; new R9 (same as R3 of the PRD
chain). Fixes in commit `27ec654`:

- R6, major: the run's second tree, keyed by the same position, holds the row's encoded primary key (at most 2,048
  bytes) followed by any computed value, since the row omits fixed-width primary key columns; each record of either
  tree fits B6. Status: claimed fixed, unverified.
- R4, minor: doc 2's primary key paragraph and off-leaf scan sentence follow the fixed-width / `string` and `bytes`
  distinction. Status: claimed fixed, unverified.
- R9, minor: the benchmark translation keeps the Spanish CLI literal. Status: claimed fixed, unverified.

The user authorized the fourth round only; a fifth needs a new authorization.

## 2026-11-10 (fifth round verification)

The user authorized a fifth round. Follow-up report: [2026-11-10-code-05.md](2026-11-10-code-05.md), range
`abee7c6..b89bd74`, thread `01a1c52a-694b-7cc3-9750-d852158e8056`, wrapper status `approved`, verdict "Approved: R4,
R6 and R9 are verified resolved; no findings remain open in the requested scope." Per-ID outcome, quoted from the
report:

- R6: "Verified resolved" by `27ec654`.
- R4: "Verified resolved" by the same commit.
- R9: "Verified resolved" by the same commit.

No finding remains open in this chain. Review record: telemetry coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.
