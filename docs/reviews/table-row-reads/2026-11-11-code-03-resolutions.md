# Table row reads code review resolutions (whole run)

Resolutions of [2026-11-11-code-03.md](2026-11-11-code-03.md), the review of the run's whole range `bb8acca..3ecc294`.
Append-only: each update is dated and keeps the earlier ones. A claimed fix stays unverified until a follow-up review
confirms it.

## 2026-11-11

Artifact versions: the reviewed range `bb8acca..3ecc294`; fixes in commits `2bc4fe4` (R2) and `bb496a1` (R3), on
branch `feat/table-row-reads`.

### R2 — major: raw text reads panic for malformed text in older schema versions

- Disposition: fixed.
- Evidence: the finding holds. The new test `ac_l1_an_older_row_with_a_text_that_is_not_utf8_reads_it_as_bytes`
  (`table::malformed_tests`: the bad text row made older by renaming another column) failed before the fix at
  `row.rs:518` with "a malformed row: a column that does not decode".
- Fix (`2bc4fe4`): `TableRow::typed` reads the stored bytes of a column of an older row through its version's layout
  and the column's place in it, as it does for a row of the current version; a column added after the row still reads
  its default as a value. The test now passes, by key and in scans from both ends and lent, in debug and in release.
- Status: claimed fixed, unverified until a follow-up review.

### R3 — major: every row lookup scans the transaction's retained table readers

- Disposition: fixed.
- Evidence: the finding holds: `Readers::keep` walked a linked list from its head.
- Fix (`bb496a1`): the kept readers form an append-only hash trie: each has 16 children, chosen by the successive
  4-bit digits of the key's hash (a product by an odd constant, a bijection, so distinct keys part within 16 levels).
  New test `many_tables_read_in_turns_keep_their_own_schemas` (300 tables of different schemas, read in turns by key
  in a read transaction and by lent scans in a write transaction).
- Reproduction after the fix (a temporary test like the review's: one-row tables of one `i64` key column, median of
  five batches of 50,000 `get_row`, Windows, release, ns per read, first and last table): 1 table 146 and 145; 100
  tables 173 and 188; 1,000 tables 192 and 196. The review's figures were 152 and 152, 163 and 292, 204 and 4,066.
- Status: claimed fixed, unverified until a follow-up review.

Checks after the fixes: `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean;
`cargo test --workspace` 803 passed, 0 failed, 23 ignored; `cargo test --release -p krummi --lib malformed_tests` 10
passed.

Review record: [2026-11-11-code-03.md](2026-11-11-code-03.md), wrapper status `findings`, thread
`01a1cc6a-e729-79c3-9bb0-606c01ff1e8e`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.

## 2026-11-11 (verification)

Follow-up report: [2026-11-11-code-04.md](2026-11-11-code-04.md), fix range `3ecc294..8b87414`, thread
`01a1cc7c-84bc-7640-af6e-11e9b273a04e`, wrapper status `findings`, verdict "Changes requested: R2 and R3 verified
resolved; R4 remains open." Review record: telemetry as in the first round.

## 2026-11-11

Artifact versions: fix in commit `6167d9e`.

### R4 — minor: the R3 regression test fails the required Clippy check

- Disposition: fixed.
- Evidence: the finding holds; `cargo clippy --workspace --all-targets -- -D warnings` failed with
  `needless_borrows_for_generic_args` at `table_rows.rs:2223`. The earlier claim that Clippy was clean was wrong:
  the check command piped Clippy into `tail`, whose exit code hid the failure. The checks now run with `pipefail`.
- Fix (`6167d9e`): the test passes the formatted name by value.
- Checks: `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean (with
  `pipefail`); `many_tables_read_in_turns_keep_their_own_schemas` passes.
- Status: claimed fixed, unverified until a follow-up review.

## 2026-11-11 (verification)

Follow-up report: [2026-11-11-code-05.md](2026-11-11-code-05.md), fix range `8b87414..9ea1b59`, thread
`01a1cc83-a15b-71d3-b953-f4e8e9985c37`, wrapper status `approved`, verdict "Approved for the requested follow-up
scope: R4 verified resolved; no open findings." Review record: telemetry as in the first round.
