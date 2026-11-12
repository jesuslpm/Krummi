# Table row reads code review resolutions (update patch)

Resolutions of [2026-11-12-code.md](2026-11-12-code.md), the review of `d6a88b7..33f7f7b` (T6–T8 of the row reads
run). Append-only: each update is dated and keeps the earlier ones. A claimed fix stays unverified until a follow-up
review confirms it.

## 2026-11-12

Artifact versions: fix in commit `c873418`, on branch `feat/table-row-reads`.

### R1 — major: update patch adds quadratic work for wide compound primary keys

- Disposition: fixed.
- Evidence: the finding holds: `Reader::patch` filtered each given column with `self.key_columns.contains(i)`.
- Fix (`c873418`): `FastColumn` marks whether a column is a primary key column, built in one pass over the key's
  columns, and `Reader::patch` reads that mark. New test `an_update_under_a_wide_compound_key_stores_the_merged_row`
  (300 `bool` key columns, update and upsert).
- Reproduction after the fix (a temporary test like the review's: a nullable string first, `k` `bool not null` key
  columns, a nullable `i64` last; 4,000 updates of the integer, median of three rounds; the control gives the
  string too, so it takes the full merge; Windows, release, not pinned, with the ignored recovery sweep running):
  patch / control 8 keys 1,418 / 1,361 ns (1.04); 256 keys 19,810 / 29,829 ns (0.66); 1,022 keys 93,919 / 124,096 ns
  (0.76). The review's figures were 0.76, 1.03 and 2.32.
- Status: claimed fixed, unverified until a follow-up review.

Checks after the fix: `cargo clippy --workspace --all-targets -- -D warnings` clean; the crate's unit tests pass.

Review record: [2026-11-12-code.md](2026-11-12-code.md), wrapper status `findings`; telemetry as in the report.

## 2026-11-12 (verification)

Follow-up report: [2026-11-12-code-02.md](2026-11-12-code-02.md), fix range `33f7f7b..b5f82bd`, thread
`01a1cf72-dc49-7161-830e-6c4697a199be`, wrapper status `approved`, verdict "Approved for the requested follow-up
scope: R1 verified resolved; no open findings."
