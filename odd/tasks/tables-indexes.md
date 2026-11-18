# Secondary indexes of the tables — ODD execution document

Objective: implement the secondary indexes of `docs/prd/krummi/tables/tables-indexes.prd.md` (v1.0, approved by the
Codex PRD review on 2026-11-18). Branch: `feat/tables-indexes` (from `main` at `0158fd4`). Delivery: `ask-on-risk`
(single branch, left unmerged). Test runner: `cargo test --workspace` (Windows; Linux in WSL 2 Ubuntu). Estimate:
10 h – 16 h. Real start: 2026-11-18 18:07:06 +0000.

## Specs

- **S1** — The indexes PRD, `docs/prd/krummi/tables/tables-indexes.prd.md` v1.0, and its ancestry up to the root:
  decisions IX1–IX15, *Cargas del banco*, *Validaciones y mensajes*, *Salida*, *Fuera de alcance* and *Dejado a la
  implementación*, verbatim there. Its acceptance criteria AC-IX1–AC-IX15 are the tests of the library.
- **S2** — IX15 and AC-IX16 of the same PRD: the loads W30–W32 in Krummi and Voron, 32 thresholds, the reduced run,
  and full benchmark runs on Windows and Linux, with the single-load measurements of W1–W29 before and after the change
  and of W30–W32 after.
- **S3** — The technical design of the indexes is the one of the table design documents, which are the PRD's technical
  starting state: `docs/design/tables/03-keys-and-indexes.md` (key encoding with direction and nulls, index layout,
  uniqueness probe), `02-row-storage.md` (H15: building an index over existing rows) and
  `11-query-contract-reconciliation.md` (index identity and index state, P12 and P17).
- **S4** — The project's checks (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`), RDD on each work-unit commit, the Codex code review over the whole range, and the closing
  report.
- **S5** — The user's instruction for open questions during the run (L1): «si surge una preguna toma tú la decisión
  más conservadora».

## Execution design

- **Layout.** Each index is a nameless unique-key B+ tree of `krummi-tree`: key = the index columns encoded with their
  direction and nulls (design document 3) followed by the encoded primary key; value = the encoded included columns.
  The index definitions, their identities, each index tree's root and the index state are stored in the table's
  catalog extra bytes after the primary key (format 12). An index has as many entries as its table has rows, so it
  keeps no count of its own.
- **Uniqueness.** A unique index probes the range of the index-key prefix (the index columns only) before any change;
  an entry with a null in any index column never collides (N7).
- **Creating over rows.** Read every row, encode its entries, sort them (in memory, then in `krummi.sort` run files
  when the memory budget is exceeded; memory only in a temporary store), detect duplicates and entry sizes in the
  sorted stream before allocating any page (IX11, H15), then build the tree by appending in order.
- **Routes.** The parent orchestrates and keeps git. Writers are delegated one unit at a time by the long-session
  backstop (the parent context is already long from the PRD session), each with its code, tests and docs; the parent
  checks each diff, runs the checks once, commits and runs the RDD assessment.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1, S3: index key encoding (direction, nulls, entry key = index columns ‖ primary key), decoding into owned values, value bounds of IX9 per direction; unit and property tests of order | delegated (backstop) | pending | |
| T2 | S1: definitions in the catalog (format 12, IX1, IX3, IX14), create (both modes, in-memory sort over rows) and drop (IX2, IX4, IX11), schema changes (IX5), Describe with identities and index state (IX6), changes and references (IX13), error data (IX10); AC-IX1, AC-IX7 (in-memory), AC-IX9–AC-IX12, AC-IX15 | delegated (backstop) | pending | |
| T3 | S1: maintenance in every row write (IX7, IX10, IX12), bulk load, empty; AC-IX2, AC-IX6, AC-IX8, AC-IX13 | delegated (backstop) | pending | |
| T4 | S1: index scans returning rows or entries, intervals (IX8, IX9); AC-IX3–AC-IX5 | delegated (backstop) | pending | |
| T5 | S1: external sort in `krummi.sort` and its cleanup (IX2: open step 8, close, failures, memory-only temporary store); AC-IX14 | delegated (backstop) | pending | |
| T6 | S2: W30–W32 in Krummi and Voron, 32 thresholds, reduced run | delegated (backstop) | pending | |
| T7 | S2: full benchmark runs on Windows and Linux (AC-IX16) | inline (background runs) | pending | |
| T8 | S4: final checks, RDD of the last slice, Codex review, closing report | inline | pending | |

## Log

- **L1** (2026-11-18) — The user's request, verbatim: «Implementa con ODD si surge una preguna toma tú la decisión
  más conservadora.» The estimate went to the user as «Entre 10 h y 16 h. ¿Empiezo?», and the user answered «Sí»
  (start 18:07:06 +0000).
- **L2** (2026-11-18) — Exploration (read-only explorer): the catalog entry stores the rows tree root and count, and
  the schema in its extra bytes (`TableSchema::encode`, `table/schema.rs:888`; size check `check_fits`, `:281`,
  against `MAX_EXTRA`); `FORMAT_VERSION = 11` (`krummi-pager/src/format.rs:41`, pinned in `krummi-pager/tests/open.rs`);
  the key encoding is ascending only with no nulls (`table/key.rs:1-3`); row writes go through `write_row`
  (`table/rows.rs:1320`) with checks before mutation, while `delete_row` (`:1305`) and `delete_range`
  (`table/ranges.rs:283`) do not decode rows; bulk load inserts row by row (`table/bulk.rs:110`); trees offer
  `create`, `free_all`, `try_add`, `append_last`, `remove`, `remove_range` and a double-ended `Range`
  (`krummi-tree/src/lib.rs`, `range.rs:236`); error messages are built in `krummi-pager/src/error.rs` (`DuplicateKey`
  `:268`, `SizeSubject` `:82`, `SchemaRule` `:138`); `krummi.temp` is created lazily and removed at open step 8 and
  at close (`krummi-pager/src/temp.rs:51`, `recovery.rs:99`, `scratch.rs:952`); there is no hook that measures a
  catalog entry's size; the benchmark has `Load::ALL: [Load; 29]` (`krummi-bench/src/workload.rs:130`) and the Voron
  harness defines only the primary key (`voron/Tables.cs:112`). RDD is on (global).
