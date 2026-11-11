# Table row reads code review resolutions

Resolutions of [2026-11-11-code.md](2026-11-11-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-11

Artifact versions: the reviewed range `bb8acca..7cf73ca`; fix in commit `9941b62`, on branch `feat/table-row-reads`.

### R1 — major: reading compound keys repeatedly decodes their prefixes

- Disposition: fixed.
- Evidence: the finding holds. `key_value` walked the key from its start for each column, so reading the n columns of
  a key walked n(n+1)/2 elements; an identical update read the old values twice.
- Fix (`9941b62`): a row whose key has several columns keeps where each column starts (`key::key_starts`, found by
  the same walk that checks the key), and `key_value` decodes the element at its start; a key of one column keeps
  nothing and starts at 0. An update decodes the old values once, for both the merge and the comparison. The test
  `is_key_accepts_the_keys_that_decode_key_decodes` now also checks that `key_starts` accepts the same keys and that
  `key_value` at each start gives the value `decode_key` gives.
- Reproduction after the fix (a temporary example with the review's schemas, one row of n non-null `Bool` key
  columns, read every column and identical update, mean ns per operation, Windows, release): 16 columns 2,382 and
  2,085; 128 columns 10,688 and 12,928; 1,024 columns 81,817 and 95,579. The review's base figures were 1,708 and
  1,776, 10,075 and 11,793, 81,243 and 89,204: the cost is linear again and near the base.
- Single loads after the fix (Windows, median op/s): W26 997,950, W27 5,043,318, W28 116,014.
- Status: claimed fixed, unverified until a follow-up review.

Checks after the fix: `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean;
`cargo test --workspace` 787 passed, 0 failed, 31 ignored.

Review record: [2026-11-11-code.md](2026-11-11-code.md), wrapper status `findings`, thread
`01a1caca-4885-7892-8311-845aa1798ad8`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.

## 2026-11-11 (verification)

Follow-up report: [2026-11-11-code-02.md](2026-11-11-code-02.md), fix range `7cf73ca..f1889a4`, thread
`01a1cae2-c045-7d52-9626-71d3c4c0be44`, wrapper status `approved`, verdict "Approved: R1 verified resolved; no open
findings in the requested follow-up scope." R1: "Verified resolved". Review record: telemetry: coordinator
`gpt-6.1-sol` / `high`, reviewer `krummi-review-critical` `gpt-6-astra` / `xhigh`.
