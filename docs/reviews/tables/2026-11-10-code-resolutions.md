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
