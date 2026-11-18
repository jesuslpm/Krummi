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
| T1 | S1, S3: index key encoding (direction, nulls, entry key = index columns ‖ primary key), decoding into owned values, value bounds of IX9 per direction; unit and property tests of order | delegated (backstop) | done (RED: `index_key_tests` did not compile; GREEN 14 of 14; six injected encoding defects each failed 3–7 tests) | `2657249` |
| T2 | S1: definitions in the catalog (format 12, IX1, IX3, IX14), create (both modes, in-memory sort over rows) and drop (IX2, IX4, IX11), schema changes (IX5), Describe with identities and index state (IX6), changes and references (IX13), error data (IX10); AC-IX1, AC-IX7 (in-memory), AC-IX9–AC-IX12, AC-IX15 | delegated (backstop) | done (7 injected defects each caught; `table_indexes` 16 of 16; verified, L6) | `9d57e7f`, `e7b95e9` |
| T3 | S1: maintenance in every row write (IX7, IX10, IX12), bulk load, empty; AC-IX2, AC-IX6, AC-IX8, AC-IX13 | delegated (backstop) | done (RED: 7 new tests hit the T2 stopgap panic; GREEN `table_indexes` 24 of 24; 7 injected defects caught; verified, L8) | `4a4c345` |
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
- **L3** (2026-11-18) — T1 (`2657249`): `key.rs` gains `Direction`, `Nulls` and `KeyColumn` over the existing
  ascending element encoder (descending inverts the element, `00 01` terminator for text and bytes; nulls `00`, `4F`,
  `B0`, `FE FF FF` per document 3); `index_key.rs` has `encode_entry`, `entry_len`, `uniqueness_range` (`None` with a
  null), `split_entry`, `decode_entry` (owned values) and `index_range` (IX9, its own validation order; bound
  construction shared with `key_range` through `range_bounds`). `fit` now cuts any bound over 2,048 bytes; the old
  2,049-byte case is a special case and the primary-key range tests pass unchanged. Left for later tasks: map the
  plain size errors to `Prefix` (T4) and to an index-entry subject (T3); malformed entries decode to `None` (T4).
  Checks by the writer: fmt, clippy and the workspace tests pass; parent spot check: `cargo test -p krummi --lib
  index_key` 14 of 14. RDD assess (base `0158fd4`, committed only): `medium`, `under_budget`. Risk: none (the shared
  `fit` keeps its only reachable primary-key case).
- **L4** (2026-11-18) — T2: definitions after the primary key in the catalog extra bytes (index state, then each index
  in name order: identity, name, root, unique, key columns with direction and nulls, included columns, an empty
  reserved partial-index condition), `FORMAT_VERSION = 12`; public `IndexDefinition`, `IndexKey`, `IndexMode`,
  `Direction`, `Nulls`, `create_index`, `drop_index`, Describe's `indexes()`, `index(name)` and `index_state()`;
  identities and index states recorded in the schema history tree so later draws avoid them; creation over rows reads
  without checking content, checks entry sizes and duplicates on the sorted entries before allocating, then appends in
  order through a memory-only `Sorter` (the seam of T5). Errors: optional index fields in `TreeAlreadyExists`,
  `DuplicateKey`, `InvalidSchema` and `InvalidSize`, new `SizeSubject::IndexName`/`IndexEntry`, seven `SchemaRule`
  and three `SchemaDifference` values, and a separate `Error::DifferentIndex` of the different-schema kind because
  `DifferentSchema` is already at clippy's error-size limit on Windows (pinned by a `size_of::<Error>() <= 120` test);
  existing Display output is unchanged. Row writes on a table with indexes panic in `live_rows` before changing
  anything, the hook point T3 fills. S5 decisions by the writer: column names of a definition are size-checked like
  the index name; included columns are stored as nullable; an included value over the row maximum fails with the row
  size error before allocating. The format bump needed mechanical edits in nine more test files (approved by the
  parent: pins of 11, a recomputed data-file hash, full-field error patterns, one more identity in `ac_py5`).
- **L5** (2026-11-18) — RDD: the first candidate (T1 + T2, `0158fd4..0cb4c39`, 4 lenses) and then T2 alone stopped
  with `lens_context_budget_exceeded` before creating any authority. The local, unpushed T2 commit `0cb4c39` was
  undone with `git reset --soft 036c982` and recommitted in two slices. Reviews, all approved and acknowledged:
  T1 slice `0158fd4..036c982` (reliability, readability; lineage `review-fa901f8836220898`; advisories: two vacuous
  assertions in `index_key_tests.rs` around `:984`, malformed entries not tested in `key.rs:128-161`, a doc
  suggestion); T2a `036c982..9d57e7f` (code, unit tests and format pins; risk, resilience, readability, reliability;
  `review-37246ff2b6d8aa00`; 13 advisories: the row-write panic until T3, the in-memory build until T5, undocumented
  row-size error, and the claim that an index key layout follows column changes, checked and dismissed: a non-null
  value encodes the same whether or not the column admits nulls, `nullable` only gates the null element and the null
  exclusion of bounds, `key.rs:149`, `:186`, `:443`, `:449`); T2b `9d57e7f..e7b95e9` (integration tests; reliability;
  `review-6793bcb98f2f04dd`; advisories: allow-nulls test is a no-op at `table_indexes.rs:1509`, dropping an included
  column unproved at `:1487`, weak page-free assertion at `:1576`, read-open damage untested at `:1975`). The
  untracked integration test file was kept outside the worktree during the T1 and T2a reviews because an untracked
  file blocks a committed-only preflight.
- **L6** (2026-11-18) — Independent verifier of T2 (risk items 1, persisted format, and 3, public error variants),
  over the original `0cb4c39` in a scratch archive: every item covered, no blockers; workspace 918 passed. Probes:
  a failed unique creation leaves `index_trees` and P12 info identical and the write commits other changes; `[id, id]`
  in Crear si no existe gives esquema no válido with the repeated column; existing error texts byte-identical to the
  baseline; a real format-11 store is refused with `DifferentFormat { found: 11, expected: 12 }` and left unchanged;
  N4 normalization in unique creation; a 30,000-row descending nulls-last index in 116 ms. Advisories: A1, a column
  name over 1,024 bytes in a definition fails with tamaño no válido (column name), following E17's pattern for rows
  and projections, while IX10's first step names only the table and index names (kept as the writer's S5 decision;
  reported in the closing report); A2, the error variants changed shape as IX10 requires; A3, AC-IX15 rewrites a
  format-12 header instead of a committed 7.1 fixture (the probe used a real one); A4, the row-write stopgap until T3.
  Scratch removed by the parent.
- **L7** (2026-11-18) — T3 (`4a4c345`): `table/maintain.rs` plans each write's effect on every index (keep, add,
  move, rewrite, remove) and checks everything before applying it: entry sizes in index-name order, the primary key,
  then each unique index (probing `uniqueness_range` on the entries only); an identical update returns before any
  index; an update probes only the unique indexes whose key columns changed and writes only the entries that change.
  A missing entry to remove, or an already-present entry to add, fails with damage of the index leaf (T11).
  `delete_row` reads the row without checking content; `delete_range` works in batches of 1,024 rows, and a panic
  escaping it after removing rows marks the write failed (`Damaged { page: None }`); a panic before any removal leaves
  it committable (IX12's «puede llegar con parte del intervalo borrado, y entonces ...», kept as the writer's reading).
  `EntryLayout::entry` is shared by creation and row writes; `Folder::entry_too_large` maps the size to
  `SizeSubject::IndexEntry`. The identical-update test uses the existing `record_layout` page-change log. Left for
  later: a unique probe costs about 3× a tree lookup in debug (range setup); W30 uses non-unique indexes. Risk: item 3
  (row writes shared with tables without indexes). RDD `e7b95e9..4a4c345` (resilience, reliability, readability;
  escalated item 3): approved, lineage `review-6b19eb30331bd5ee`; advisories: Vaciar not covered by a test of the
  index trees at `rows.rs:1457`, an error after a partial range delete at `maintain.rs:346`, doc and lint
  suggestions, and this document's task table not yet updated. Independent verifier of T3 launched.
- **L8** (2026-11-18) — Independent verifier of T3 over `4a4c345` in a scratch archive: no blockers. Differential
  check of tables without indexes against `7c7e3c3` (insert, duplicate, oversized row, identical/partial/full/null
  updates, older schema versions, upsert, delete, range delete, abort, bulk loads with failures), on disk and
  temporary: transcripts identical, including error texts and P12 counts. Failed unique writes leave rows, entries,
  roots and counts unchanged and the write commits other changes; range deletes across batch boundaries (1,023–2,049
  rows) and reopen keep every index in step; float collisions agree between creation, insert and update; five
  mutations each fail a test. It agrees with the writer's two readings (the range-delete panic fails the write only
  after a removal; an already-present entry is damage). Advisories: every row write on a table without indexes now
  looks up its maintained indexes (one extra lookup; to watch in T7's runs); a stale entry in a unique index reads as
  a duplicate key (IX12 allows it). Workspace in scratch: 944 passed. Scratch removed by the parent.
- **L9** (2026-11-18) — T4 (`025531a`, tests `ba3d843`): `index_rows` and `index_entries` on both transactions with
  `(table, index, prefix, bounds)`; rows reuse `ReadRows`/`WriteRows` through an optional `Through` in the shared
  `ranges::Scan` (lending and projection binding unchanged); entries (`IndexEntry`: `len`, `name`, `value`, `get`,
  `values`, `columns`) decode index-key and primary-key values lazily into owned copies and lend included columns;
  stepping never decodes, so a malformed entry is returned and reading a bad value panics (S5 decision, IX12/L2); an
  entry that cannot be split or whose row is missing is damage of its index leaf when rows are requested. A missing
  index is `DifferentIndex` with the new `SchemaDifference::MissingIndex`; an oversized prefix is `InvalidSize` with
  `SizeSubject::Prefix`. The T2 test hook became `stored_index_entries`. Splitting the diff (2,818 lines) up front:
  T4a code `ba9de2e..025531a` (reliability, readability, resilience; escalated item 3, the shared scan): approved,
  `review-f1a75d485521db85`; T4b tests `025531a..ba3d843` (reliability): approved, `review-6074aa258a6f86ef`.
  Advisories to address in a test-quality pass: the property test's model is rebuilt from the table under test
  (`table_index_scans.rs:1036`), row scans compare only identities (`:195`), the post-commit whole-index check
  (`:1077`), and an untested accept side of the combined prefix limit (`:692`). Risk: item 3. Independent verifier of
  T4 launched; T5 launched.
- **L10** (2026-11-18) — Independent verifier of T4 over `ba3d843`: no blockers. Primary-key scans byte-identical to
  `ba9de2e` across a 534-line transcript (prefixes with `-0`, NaN, null, sizes 2048–5000; forward, backward,
  lending, projection binding, write scans, `delete_range`). Entry column order, copy versus borrow (observable as
  the `Cow` variant of `Value`), descending/nulls-last bounds, `-0`/NaN, equal keys in primary-key order and exact
  reverse, missing index, the exact 2,048-byte prefix boundary, and scan-only writes leaving files byte-identical all
  hold; two mutations caught. Advisories: the crate doc in `lib.rs:32-36` does not mention scans (to fix);
  `IndexEntry::value(i)` skips earlier elements on each call (quadratic in key columns); `Scan` carries an optional
  `Through` on every primary-key scan (watch W27 in T7). Scratch removed by the parent.
- **L11** (2026-11-18) — T6 (`e340580`, a parallel writer with a disjoint surface while T5 ran): W30–W32 in
  `workload.rs` (`Load::ALL` 32, `SIZED` 13 with W30), data helpers `n`, `ts`, `ts_order`, `index_range_starts`,
  `expected_index_ranges`; the Krummi worker creates the table and both indexes before the timer (the PRD's starting
  point «Almacén vacío con la tabla y sus dos índices»), W31 uses `index_rows(ix_n, [n])` and checks one row, W32
  `index_rows(ix_ts, [], ts..)` up to 100 rows, both with a projection (PY7). Voron: a new `Indexed` schema laid out
  `n, ts, id, x, flag, name` with `n` and `ts` big-endian sign-flipped, `ix_ts` over `ts, id` so equal `ts` follow the
  primary key (S5 decision), `ix_n` over `n`. Report tests pin 32 thresholds. Checks: `cargo test -p krummi-bench`,
  clippy on the crate, the Voron selftest (known digests W30 `04ee91a577139ff6`, W31 `b3118bdd51c932a8`, W32
  `2d301645b54fc739`), the all-engine reduced test (181 s) and a reduced `--solo-krummi-voron` run with matching
  digests (cold, one repetition: W30 5.28, W31 7.55, W32 8.51 Krummi/Voron). Risk: item 6 (the C# harness has no
  cargo test; the selftest and the reduced run are its checks). RDD `ba3d843..e340580` (reliability, readability):
  approved, `review-38cf23bdc604d2e6`, no advisories (T5's three new files were kept outside the worktree during the
  preflight).
- **L12** (2026-11-18) — T5: `krummi-pager/src/sort.rs` (`SortFiles`, `SortWriter`, `SortReader`, `SortRun`,
  `SORT_DIR`; folder created on the first run file, removing any leftover first; 64 KiB buffers; drop removes it best
  effort; fault points `SortCreate`, `SortWrite`, `SortRead`, `SortRemove`; an I/O failure while writing goes through
  `fail_write`, T12); open step 8 removes `krummi.sort`, folder or file, after the journal and `krummi.temp`, an
  access failure there fails the open; close and drop remove it after the temporary area. `krummi/src/table/sort.rs`
  replaces T2's memory-only `Sorter`: budget `SORT_MEMORY` = 64 MiB, run records with a 12-byte header, heap merge of
  up to 64 runs, group merges beyond; a temporary store never spills; test hooks `with_sort_memory`,
  `with_sort_pause`. AC-IX14 in `table_index_sort.rs` (8 tests, about 136 runs with the reduced budget), AC-IX7
  re-checked through the spill path; 8 injected defects caught; workspace 961 passed (with T6 in the tree, the seam
  check). S5 decisions: a failure to remove `krummi.sort` at close is classed like the temporary area's; a leftover
  that cannot be removed at the start of a sort fails that creation with access; only an I/O failure while writing
  gets T12. Not tested: P10/P12 exclusion (separate files the pager never accounts). Risk: item 1 (the open deletes
  the reserved folder's contents) and item 3 (shared recovery and close paths). RDD and an independent verifier
  launched.
- **L13** (2026-11-18) — RDD of T5 `e340580..7c33f57` (risk, resilience, reliability, readability; escalated items 1
  and 3): approved, lineage `review-a4fa65d8e5d34771`. A docs commit (`f058814`) made during the review moved HEAD,
  so the driver's re-query offered a fresh START for the new target (a consent envelope, nothing created); the
  approved lineage was then acknowledged with its own provider-issued acknowledgement (authority burned). Lesson: no
  commits while a review runs. Advisories: a run file's end is not verified (`sort.rs:283`, a truncated run would read
  as a normal end: fixed in the fix pass), the sort failure order doc (`index.rs:787`), the 64 KiB buffer boundary
  untested. Independent verifier of T5 over `7c33f57`: no blockers. The open removes only `krummi.sort` (folder,
  file, any case on Windows), never on a failed open, a locked store, a damaged journal or a format mismatch; user
  files and look-alike names stay byte-identical; a folder holding only `krummi.sort` is not a store and is
  untouched; open, close and drop byte-identical to `e340580` in 26 scenarios; spilling and memory-only builds give
  the same entries and info; a 1 MiB-maximum store gives the same info and max-size error either way. Advisories:
  `README.md:82` does not list `krummi.sort` (fix pass); a junction named `krummi.sort` makes every open fail with
  access (the target is never followed; allowed by IX2). Scratch removed by the parent. A fix-pass writer was
  launched for the accumulated advisories (L5, L7, L9, L10, this entry).
