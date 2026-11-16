# Table projection code review resolutions

Resolutions of [2026-11-16-code.md](2026-11-16-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-16

Artifact versions: reviewed range `28f9d3b..daccfcd`; the R2 fix in the commit after `daccfcd`
(`fix(tables): never draw again an identity a projection may carry`).

### R1 — blocker: AC-PY11 is marked satisfied despite failing its required ratio check

Disposition: pending, a product decision. The figures are right: L10 of
[table-projection.md](../../../odd/tasks/table-projection.md) already says the single-load clause holds only for
W22 and W29, and that T5 was closed on a Krummi-only `main`/head A/B instead, as a decision taken during the run
(no regression of the branch; W27 up on both systems; digests match). That decision changes how the criterion is
met, so it is the user's call: accept the A/B, measure again, or amend AC-PY11. The requirement was not edited.
T5 stays recorded as done with that decision listed in the closing report as open until the user answers.

Status: pending.

### R2 — major: an aborted schema identity can alias another schema and return another column's value

Disposition: fixed. Reproduced first with the new test
`ac_py5_a_repeated_identity_never_aliases_a_schema` (`crates/krummi/tests/table_projections.rs`), which forces the
generator to repeat values through `with_identity_source`, in two cases: the abandoned write's schema state drawn
again by the next write (the review's probe), and a deleted table's identity and schema state, read from the files of
a reopened store, drawn again for the table created with its name in a later write. Before the fix the first case
failed: the old projection bound the new schema's row. Fix: while open, the store keeps the identities of the tables it
created, also in abandoned writes, or deleted (`Projections::retire_table`), and for each table the schema states its
schema changes drew, also in abandoned writes (`retire_schema`). `create_table` never draws a retired table identity,
and `change_schema` never draws a state retired for that table. `SchemaId` is the store, the table and the state, so
two schemas the store shows never share one: binding fails with `DifferentSchema` and the cache keeps them apart. A
new table's state may still equal another table's, as D6 allows and `d6_identities_equal_to_known_ones_are_drawn_again`
checks (a first version of the fix retired bare states and broke that test). Mutation check: without the retire on
delete, the second case fails at the bind; without the retire in schema changes, the first fails at the bind. Checks:
`cargo fmt --all --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo test
--workspace` 852 passed, 0 failed, 34 ignored.

Status: claimed, unverified until a follow-up review.

Review record: [2026-11-16-code.md](2026-11-16-code.md), wrapper status `findings` (R1, R2 open; one open question,
AC-PY11), thread `01a1e315-1c7d-7170-82ff-27fbdc6d977b`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.

## 2026-11-16 (verification)

Follow-up report: [2026-11-16-code-02.md](2026-11-16-code-02.md), over `daccfcd..df72e3f`, wrapper status `findings`,
thread `01a1e334-2aac-7141-b04a-2eb57497248c`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`. Verdict, quoted: «Changes requested: R1 remains open; R2 is verified
resolved.»

- R1: «blocker, still open». Codex: «keep AC-PY11/T5 open until the specified evidence satisfies it or the user
  explicitly approves a revised acceptance condition.» T5 of
  [table-projection.md](../../../odd/tasks/table-projection.md) now reads pending that decision (L17). Status:
  pending, a product question for the user.
- R2: «major, verified resolved». No correction-induced regressions were confirmed.
