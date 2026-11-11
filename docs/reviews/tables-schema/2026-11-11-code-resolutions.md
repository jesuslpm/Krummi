# Tables with schema code review resolutions

Resolutions of [2026-11-11-code.md](2026-11-11-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-11

Artifact versions: the reviewed range `d487cde..1479c19`; fixes in commits `574f7d3` (R3) and `b0288e3` (R1, R2), on
branch `feat/tables-schema`.

### R1 — major: a valid large default panics while creating the table

- Disposition: fixed (the panic), with a provisional error class pending the user's decision.
- Evidence: the finding holds; the report's reproduction panicked at the tree's size assertion before the fix
  (`ac15_a_default_too_large_for_the_catalog_fails`).
- Fix (`b0288e3`): `check_fits` (`table/schema.rs`) sums the exact size the catalog encoding would write and fails
  without effect with `InvalidSchema` / `SchemaRule::InvalidDefault`, naming the column, when it passes
  `krummi_tree::catalog::MAX_EXTRA` (`MAX_VALUE` minus the entry's 25-byte fixed part). It runs where E17 puts invalid
  schema (after the catalog checks, before different schema), in every opening mode that takes a schema, in
  `add_column` and in the column rename branch (a longer name rewrites the same entry). Tests:
  `ac15_a_default_too_large_for_the_catalog_fails` (the reproduction in CreateNew, OpenOrCreate, CreateOrEmpty and
  `add_column`; no table, schema unchanged, the write commits other changes) and
  `ac15_a_schema_that_fills_the_catalog_entry` (an entry exactly at the limit is created; a rename past it fails).
- Pending: the error class and whether this limit is acceptable are a product question for the user, asked after
  the loop (the report: "any additional public schema limit needs an explicit product decision").
- Status: claimed fixed, unverified until a follow-up review.

### R2 — major: reading added-column defaults copies their payload for every old row

- Disposition: fixed.
- Evidence: the finding holds; `e11_old_rows_borrow_the_defaults_of_added_columns` failed before the fix with "the
  column b does not borrow its value".
- Fix (`b0288e3`): defaults live only in the table's catalog entry (the history versions hold none).
  `decode_defaults` decodes them borrowed from the catalog value; the per-read and per-scan cache `Versions<'t>`
  keeps them, decoded once when an old row first needs a `string`, `bytes` or vector default, borrowing the catalog
  page for the transaction's lifetime. A missing or mismatched entry is damage of the catalog page, which E19 allows
  for reading a row. The test reads two old rows by key and by scan, in the write that added the columns and in a
  later read, with a 1 MiB `bytes`, a `string` and an `f32` vector default: every value is `Cow::Borrowed` and
  points to the same memory.
- Status: claimed fixed, unverified until a follow-up review.

### R3 — minor: stored vector norms use a different tail reduction order

- Disposition: fixed.
- Evidence: the finding holds; `vector_norm_adds_the_tail_after_the_lanes_in_order` got bits `...801` instead of
  `0x41a0000000000000` before the fix.
- Fix (`574f7d3`): `fixed_order_norm` reduces the 16 lanes in the fixed tree, then adds each tail square on its
  own, in order, then takes the square root (design 05). The `f16` path shares it (a pinned case: 2^15, fifteen
  zeros, seven 2^-12 gives exactly 32768.0); the `i8` path sums exact integers, so the order cannot change it. The
  stored norm of some cosine vectors changes; the format is new in this unmerged run, so no migration is needed.
- Status: claimed fixed, unverified until a follow-up review.

Open question Q1 (temporal domains before or after rounding): pending, a product question for the user, asked after
the loop; behaviour unchanged.

Checks after the fixes: `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean;
`cargo test --workspace` passed; `cargo test -p krummi --test table_encoding --test table_rows` 42 and 26 passed.

Review record: [2026-11-11-code.md](2026-11-11-code.md), wrapper status `findings`, thread
`01a1c810-4d2a-78e0-8dcb-a1b96fb81cf5`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.

## 2026-11-11 (verification)

Follow-up report: [2026-11-11-code-02.md](2026-11-11-code-02.md), fix range `1479c19..ba8b5d2`, thread
`01a1c82f-ae14-7db1-91e7-bb5b34abd098`, wrapper status `findings`, verdict "Changes required." Per finding, quoted: R2
"Verified resolved."; R3 "Verified resolved."; R1 "still open, partially remediated": the panic is gone, but the
permitted schema of the reproduction is still rejected by a catalog limit no requirement approves. Open questions:
Q1 (carried forward) and Q2 ("is an additional limit on the aggregate encoded schema/defaults acceptable, and what
public error should it use?").

### R1 — major, follow-up: valid large defaults rejected by an undeclared catalog limit

- Disposition: pending, a product decision (Q2), asked to the user after the loop. No requirement is edited before
  the answer.
- Status: open.

Review record: telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer `krummi-review-critical` `gpt-6-astra` /
`xhigh`.

## 2026-11-11 (product decisions)

The user answered both open questions after round 2 (asked one at a time):

- **Q2 (R1):** «Aceptar el límite». A schema whose stored form, with its column names and defaults, passes 256 MiB
  fails without effect with invalid schema and a new last rule of E5, `SchemaRule::SchemaTooLarge` ("esquema demasiado
  grande"), naming the column where it passes the maximum, or the column of the change when adding or renaming a
  column. PRD annotation: `tables-schema.prd.md` and its translation, block of 2026-11-11 before *Cargas del banco*.
- **Q1:** «Se acepta». The temporal domain is checked on the value rounded to the microsecond (the stored one), which
  is the current behaviour; E8 is annotated to say so. No code change.

### R1 — major: valid large defaults rejected by an undeclared catalog limit

- Disposition: fixed per the user's decision.
- Fix (code `6e4a04a`, PRD `461bccc`): `check_fits` reports `SchemaTooLarge` with the column where the
  stored schema passes `MAX_EXTRA`; at creation it runs after every other E5 rule; for `add_column` and column renames
  it runs after the version check (P19), so the new rule is the last of E5 in every path, naming the added or renamed
  column. `ac15_a_default_too_large_for_the_catalog_fails` and `ac15_a_schema_that_fills_the_catalog_entry` assert the
  new rule (they failed against the provisional `InvalidDefault`, then passed). This also answers the RDD advisory
  R2-001 of `review-1b1106545ae7a5fc` (the provisional error named the last column with a default).
- Status: claimed fixed, unverified until a follow-up review.

## 2026-11-11 (third round verification)

Follow-up report: [2026-11-11-code-03.md](2026-11-11-code-03.md), fix range `ba8b5d2..` the head of the record commit
of L17, thread `01a1c931-677e-7eb2-ae47-942c25087a99`, wrapper status `approved`, verdict "Approved." No finding and no
open question remains in this chain. Review record: telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.
