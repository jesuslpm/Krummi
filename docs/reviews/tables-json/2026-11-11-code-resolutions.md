# Tables JSON design review resolutions

Resolutions of [2026-11-11-code.md](2026-11-11-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-11

Reviewed range: `d0d4ef6..62df5d5` (documents 13 and 14 of `docs/design/tables`). Fix commit: `e36a9b7` (R1, R2).

Review record: `docs/reviews/tables-json/2026-11-11-code.md`, wrapper status `findings` (open R1, R2), `threadId`
`01a1c9f7-0be1-7a10-bdcd-233538d7c4b5`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`. The eight open product questions the report lists are the ones
document 14 already defers until the indexes are planned (*Product questions*, decided with the user on 2026-11-11);
none is new, and none is answered here.

### R1 — major: One exact numeric term cannot cover rounded f64 equality

- Disposition: fixed.
- Verification before the fix: the cited lines say both "never a rounded value" (document 13, typed extraction;
  document 14, *Extraction*) and "`f64` correctly rounded" (document 13, *Numbers* and *Limits and security*), and
  document 14's rule 1 gave `Eq(JsonAs(f64, ...))` the list of one exact term. The report's model reproduces the
  false negative: `python -I` over `{1: "0.1", 2: "0.10000000000000001"}` with the `f64` parameter 0.1 prints
  `scan [1, 2] old [1]`.
- Fix: one extraction rule in both documents: integer and `decimal` types are exact or `NULL`, never rounded; `f64`
  is correctly rounded and `NULL` only outside its finite range. In document 14, rule 1 gives no list to an
  equality on `JsonAs(f64, ...)`; the exactness table splits the numeric row into `i64`, `u64` and `decimal`
  (candidates, recheck) and `f64` with ranges on typed numbers (no index; an expression index on `JsonAs(f64, ...)`
  answers it). *Tests and acceptance criteria* adds the case: `Eq(JsonAs(f64, n), 0.1)` returns both documents.
- Check after the fix: the same model with the corrected rule (no list, evaluated as a filter) prints `new [1, 2]`,
  equal to the scan.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — minor: The conformance corpus requires accepting forbidden duplicate names

- Disposition: fixed.
- Verification before the fix: document 13's conformance layer said "`y_` files must be accepted" with no
  exception, while stored-form rule 5 and the user's decision (question 2) refuse duplicate members; the report
  names `y_object_duplicated_key.json` and `y_object_duplicated_key_and_value.json` from the suite.
- Fix: a `y_` file that breaks a stricter rule of Krummi is a pinned exception, listed by name with the rule and the
  expected error; the two duplicate-name files are listed, refused as duplicate members, and an unlisted difference
  fails the test.
- Status: claimed fixed, unverified until a follow-up review.

Checks for the follow-up: `git diff --check 62df5d5..e36a9b7`, no output. The range changes two Markdown documents
only; `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`
do not apply to it, as in the original review.

## 2026-11-11 (verification)

Follow-up report: [2026-11-11-code-02.md](2026-11-11-code-02.md), wrapper status `approved`, `threadId`
`01a1ca00-4b08-7b03-9e4f-0ee3bea97d3b`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`. Fix range `62df5d5..7f3bd04`. Verdict, quoted: "Approved — R1 and
R2 verified resolved; no findings remain open in the requested follow-up scope."

- R1: "Verified resolved"; the reviewer reran the model ("scan [1, 2] old [1] corrected [1, 2]").
- R2: "Verified resolved".

No product question was added; the eight deferred questions of document 14 stay open until the indexes are planned.
