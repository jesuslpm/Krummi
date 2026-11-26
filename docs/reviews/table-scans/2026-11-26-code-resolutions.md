# Resolutions: code review of table-scans (2026-11-26)

Report: [2026-11-26-code.md](2026-11-26-code.md). Range reviewed: `76d8820..8ed2a76f`.

## 2026-11-26

Artifact versions: fix commit `19f1e784`.

### R1 — blocker: Unbounded text and bytes filters discard all matching rows

- *Disposition:* fixed.
- *Fix:* `19f1e784`. `Filter::check` now keeps the rank form of a condition only for a column of a fixed-width type
  (`ColumnType::fixed_width`), chosen by the column's type instead of by whether every value has a rank; a range
  without bounds holds no value, so it had taken the rank path on a text or bytes column and rejected every row.
- *Reproduction and regression test:* `exact::an_unbounded_range_on_text_or_bytes_passes_every_row_that_is_not_null`
  (text and bytes, with and without a secondary index, on disk and in memory). Before the fix it failed with
  `left: [], right: [1, 3]` for `string, index false`; after it, it passes.
- *Checks after the fix:* `cargo test -p krummi --test table_vector_indexes exact::` 8 passed; `-- approximate::
  pending::` 11 passed; `cargo test -p krummi --lib rank_tests` 2 passed; `cargo clippy -p krummi --all-targets -- -D
  warnings` clean; `cargo fmt --all --check` clean.
- *Status:* claimed, unverified until a follow-up review.

Review record: [2026-11-26-code.md](2026-11-26-code.md), wrapper status `findings`, `threadId`
`01a217c4-2794-7a12-855f-72fa69d4c479`; telemetry: coordinator `gpt-6.1-sol`/`high`, reviewer `krummi-review`
`gpt-6.1-sol`/`high`.

## 2026-11-26 (verification)

Report: [2026-11-26-code-02.md](2026-11-26-code-02.md), wrapper status `approved`, `threadId`
`01a217d3-9b9b-7321-8973-47419f01284e`; telemetry: coordinator `gpt-6.1-sol`/`high`, reviewer `krummi-review`
`gpt-6.1-sol`/`high`. Verdict: "Approved: R1 verified resolved; no open findings in the requested follow-up scope."

- R1: verified resolved.
