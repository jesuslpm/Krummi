# Collation of text columns code review resolutions

Resolutions of [2026-11-23-code.md](2026-11-23-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-23

Artifact versions: the review covered `6b583a2..22da401`; the fixes are the commits `61ee7bf`, `e5d9b8f`, `aca3dd3`
and `1faea20` on `feat/tables-collation`. Each regression test was seen failing without its fix (the three of
`tests/table_collation/review.rs` failed with the source changes stashed; the R3 test needs the new slot list).

### R1 — major: Equivalent predicates produce different equality estimates

- Disposition: fixed.
- Fix: `e5d9b8f`. An estimate's equality value is large only when its folded form passes 2,048 bytes
  (`stats::is_large_folded`, IC7); stored values keep the rule of IC8 (the original or the folded form). Regression:
  `table_collation::review::r1_a_long_spelling_of_a_short_folded_form_estimates_as_it`, the report's reproduction
  (`ana` and `a` + 1,024 × U+0301 + `na` both estimate 1).
- Status: claimed fixed, unverified until a follow-up review.

### R2 — major: Composite-index statistics read oversized original texts

- Disposition: fixed.
- Fix: `aca3dd3`. A prefix reads its key columns only up to the first one whose stored text or bytes are large (or
  whose length is invalid), as the column gathering does (ST1); the prefixes that hold that column count the row as
  large, and their pages are not touched. Regression: `table_collation::review::r2_analyze_never_reads_a_large_original_through_an_index_prefix`,
  the report's reproduction (analyze of all columns reads as many pages as analyze of the column).
- Status: claimed fixed, unverified until a follow-up review.

### R3 — major: Duplicate original text storage rejects rows below the row limit

- Disposition: fixed.
- Fix: `61ee7bf`. The entry value keeps one slot of original text per column (`EntryLayout::original_places`); every
  element of the entry's key of that column (index key and primary key) maps to it. Regression (internal, since the
  report's case needs a 134 MB row): `index_tests::a_collated_column_keeps_its_original_text_once`, a `ci_ai`
  primary key with a descending index on it has one slot.
- Status: claimed fixed, unverified until a follow-up review.

### R4 — minor: Collated index keys return borrowed texts despite the owned-copy contract

- Disposition: fixed.
- Fix: `1faea20`. `IndexEntry` returns the original text of a key or primary key column in a copy of its own (IX8);
  included columns keep borrowing. Regression: `table_collation::review::r4_an_entry_returns_a_collated_key_text_in_its_own_copy`.
- Status: claimed fixed, unverified until a follow-up review.

Review record: [2026-11-23-code.md](2026-11-23-code.md), wrapper status `findings` (R1–R4), thread
`01a207d9-9bbb-7d93-b319-dca19ff818e9`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.

Checks before the follow-up, on `1faea20`: `cargo fmt --all --check` passes; `cargo clippy --workspace --all-targets
-- -D warnings` gives no warning; `cargo test --workspace --no-fail-fast`: 65 test binaries, none failed.

## 2026-11-23 (verification)

Follow-up report: [2026-11-23-code-02.md](2026-11-23-code-02.md), wrapper status `findings` (R2, R5), thread
`01a207f8-da67-7ae0-9c25-86b39405e556`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.

- R1: "Verified resolved."
- R2: "Still open for defaults, as above; stored-row reproduction resolved."
- R3: "Verified resolved."
- R4: "Verified resolved."
- R5 (new): "Joint estimates treat the new large-value fraction as ordinary combinations".

## 2026-11-23 (round 2 fixes)

Artifact versions: fixes `a6a8679` (R2) and `9974d0a` (R5) on `feat/tables-collation`, after `1faea20`. Both
regressions failed with the source changes stashed (2 of 5 in `tests/table_collation/review.rs`).

### R2 — major: Composite-index statistics read oversized original texts (defaults)

- Disposition: fixed.
- Fix: `a6a8679`. A key column the row does not store (a column added after the row) reads its default, borrowed
  from the catalog, and measures it with `stats::is_large` (the original length first, so an oversized text is never
  folded) before the prefix reads it; a large default stops the prefix as a large stored text does. Regression:
  `table_collation::review::r2_analyze_never_folds_a_large_default_through_an_index_prefix`, the report's case
  (the column counts the default as large and the prefix holds no combination with `a`).
- Status: claimed fixed, unverified until a follow-up review.

### R5 — major: Joint estimates treat the new large-value fraction as ordinary combinations

- Disposition: fixed.
- Fix: `9974d0a`. A prefix group estimates as an equality of ST2 (ST18): when one of its values is large by its
  folded form, the group's fraction is the prefix's large fraction; otherwise an unmatched combination shares what
  the frequent combinations and the large fraction leave. Regression:
  `table_collation::review::r5_joint_estimates_keep_the_large_fraction_apart`, both reproductions of the report (the
  absent short combination estimates 0; the large predicate over thirteen rows estimates 1).
- Status: claimed fixed, unverified until a follow-up review.

Checks before the follow-up, on `9974d0a`: `cargo fmt --all --check` passes; `cargo clippy --workspace --all-targets
-- -D warnings` gives no warning; `cargo test --workspace --no-fail-fast`: 65 test binaries, none failed.

## 2026-11-23 (verification 2)

Follow-up report: [2026-11-23-code-03.md](2026-11-23-code-03.md), wrapper status `approved`, thread
`01a20812-c8c5-7c42-97e7-e707974998eb`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer `krummi-review`
`gpt-6.1-sol` / `high`. Verdict: "Approved." No finding open in scope.

- R2: "Verified resolved: Composite-index statistics read oversized original texts".
- R5: "Verified resolved: Joint estimates treat the new large-value fraction as ordinary combinations".
