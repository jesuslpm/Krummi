# Writer brief for the tables-indexes run (common to every task)

You are a bounded ODD writer in the Krummi repository (`c:\projects\Krummi`, Rust workspace, Windows, Git Bash and
PowerShell available). The parent orchestrator owns git: **do not commit, branch, stash, reset or checkout**; leave
your changes in the working tree. Another task of the same run may have left committed work you build on.

## Read first

1. `CLAUDE.md` (project rules: engineering principles, English code and comments, Spanish CLI messages and PRDs).
2. `odd/tasks/tables-indexes.md` until `## Log` (specs S1–S5, execution design, tasks), then its `## Log` entries.
3. The PRD `docs/prd/krummi/tables/tables-indexes.prd.md` (authoritative, Spanish) — the decisions and acceptance
   criteria your task names, plus anything they reference in its ancestry (`tables.prd.md`, `tables-schema.prd.md`,
   `tables-row-reads.prd.md`, `tables-projection.prd.md`, `../krummi.prd.md`) as needed.
4. `docs/design/tables/03-keys-and-indexes.md` (encoding, layout, uniqueness), `02-row-storage.md` (H15) and
   `11-query-contract-reconciliation.md` (identity, index state) where your task touches them.

## How to work

- Reuse the existing helpers for shared state (key encoding in `crates/krummi/src/table/key.rs`, catalog helpers in
  `crates/krummi/src/table/catalog.rs`, identities in `table/identity.rs`, error builders such as
  `Folder::invalid_schema`/`different_schema` and `Fault::error`, tree functions of `krummi-tree`). Never re-implement
  a step those already do with a different shape. Keep their invariants (check everything before mutating; T11 on
  damage; references ended by `&mut self`).
- Test first: for each rule your task covers, write the test, observe it fail (RED; "does not compile because the API
  is missing" counts), implement (GREEN), refactor. Tests go through the public interface (integration tests in
  `crates/krummi/tests/`, a new file per area such as `table_indexes.rs`, with `mod support;`), unit tests only for
  internals such as encoding order. Name tests as snake_case sentences, prefixed with the AC id when they implement
  one (`ac_ix3_...`). Each test asserts every observable effect the rule names (result, error kind and its data,
  persisted state after commit/reopen).
- For every existing behavior that shares code you change (row writes, schema changes, Describe, error Display,
  catalog encoding), keep or add a test that proves the earlier behavior still holds.
- Decisions the PRD leaves to the implementation are yours: take the simplest correct one. If a real product question
  appears, take the most conservative decision (S5), and report it.
- Scratch state (temporary folders, `TMPDIR`) goes outside the worktree (`mktemp -d`), or under ignored `target/`
  or `work/`; report any scratch path you leave.
- Keep authored changes proportionate; about 400 changed lines is a planning heuristic, not a cap.

## Verification (run in the foreground before returning)

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` (it stops at the first failing test binary; rerun after fixes until it passes)

## Return

- Changed files (`git status --short`) and a summary of the design choices you made.
- Which `S#`, decisions (IX#) and acceptance criteria (AC-IX#) you covered, each with its test names.
- RED evidence (what failed before the implementation) and GREEN evidence.
- `<command>: <observed result>` for each verification command.
- Any conservative decision taken under S5, and anything left for a later task.
- Close with a `## Key Learnings` block of 1–5 numbered standalone factual sentences (omit it if none).
