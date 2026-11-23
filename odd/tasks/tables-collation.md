# Collation of text columns — ODD execution document

Objective: implement the per-column text collation of `docs/prd/krummi/tables/tables-collation.prd.md` (7.7, approved
by the Codex PRD review on 2026-11-23). Branch: `feat/tables-collation` (base `6b583a2`; `main` holds the PRD at
`0736e16`). Delivery: `ask-on-risk` (single branch, left unmerged). Test runner: `cargo test --workspace` (Windows;
Linux in WSL 2 Ubuntu). Estimate: 12 h – 18 h. Real start: 2026-11-23 08:26:24 +00:00.

## Specs

- **S1** — The collation PRD, `docs/prd/krummi/tables/tables-collation.prd.md`, and its ancestry up to the root:
  decisions IC1–IC13, *Validaciones y mensajes*, *Salida* (with the examples table), *Fuera de alcance* and *Dejado a
  la implementación*, verbatim there. Its acceptance criteria AC-IC1–AC-IC12 are the tests of the library.
- **S2** — IC12 and AC-IC11: one-round pass of W1–W32 before and after, 10 to 20 rounds of W23–W32 and of any load
  that moves more than 3 %, and the informative measurement of a 100,000-row table with a `string` key of about 24
  bytes and a unique index on another `string` column, `ci` against `binary`, on Windows and Linux.
- **S3** — The Unicode 17.0 data, vendored byte for byte in `crates/krummi/unicode/17.0.0/` (`CaseFolding.txt`,
  `UnicodeData.txt`, `Scripts.txt`, `CompositionExclusions.txt`, `NormalizationTest.txt`, `LICENSE.txt`), with a
  generator script whose output (the Rust tables) is committed (user, L2).
- **S4** — The project's checks (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`), RDD on each work-unit commit, the Codex code review over the whole range, and the closing
  report.
- **S5** — The user's instruction for the run (L1): «Haz la implementación con ODD en la rama, todas las preguntas al
  principio, después de la estimación de tiempo no preguntes, toma la decisión recomendada/conservadora.»

## Execution design

- **Public API.** `Collation { Binary, Ci, CiAi }` (`table::collation`, re-exported), `Display` `binary` / `ci` /
  `ci_ai`. `Column` gains `collation: Option<Collation>` and `with_collation`; `SchemaColumn` gains
  `collation: Option<Collation>`, `Some` for every `string` column. `fold(collation, text) -> Cow<str>` is the one
  folding function every key path calls.
- **Folding (IC2).** Own tables generated from the vendored files by `scripts/unicode/gen_collation_tables.py` into
  `crates/krummi/src/table/collation_tables.rs`: canonical decomposition and combining classes, composition pairs
  (without exclusions), full case folding (C + F), general category Mn, and the Latin / Greek / Cyrillic script ranges.
  Hangul is algorithmic. An ASCII fast path skips the tables. No new library dependency (IC11).
- **Schema (IC1, IC9).** Catalog column flag `COLLATION = 8` followed by one byte (1 `ci`, 2 `ci_ai`); absent means
  `binary`. `SchemaRule::CollationOnNonText` right after `CosineOnBit`; `SchemaDifference::Collation` checked after
  `Type`. `modify_column` keeps its signature: it gives no collation, so it expects `binary` (IC1), and a new
  `modify_column_with_collation` also takes the collation (L4). Format 15 (IC10).
- **Keys (IC3, IC4, IC7).** `KeyColumn` and the primary-key type list carry the collation; a `string` element is folded
  before it is measured and encoded, so sizes, bounds and prefixes follow the folded form. An update or upsert through
  an equivalent key keeps the stored key columns (IC4).
- **Index entries (IC5, IC6).** The entry value appends, after the included columns, the original text of every `ci`
  or `ci_ai` column of the index key and of the primary key; `IndexEntry` reads those columns from there. A change of
  that text alone is a `Rewrite` of the entry. Duplicate-key texts come from row values on writes and from the entry
  value on index builds.
- **Statistics (IC8).** The statistics column folds through the same `KeyColumn`; a text is large when it or its folded
  form passes 2,048 bytes; bounds are shortened at a `char` boundary of the folded form.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1: IC2, IC11; AC-IC2, AC-IC12; S3 (data, generator, tables, `fold`) | delegated: parallel with T2 (Writer trigger a) | done (RED 4 of 5 on the stub; 5 + 6 tests pass) | `29fb1cc`, `320e9a9`, `82c6d47` |
| T2 | S1: IC1, IC9, IC10; AC-IC1, AC-IC9, AC-IC10 (schema, catalog, errors, format, Describe, PY2) | inline | done (RED: 34 compile errors; 5 tests pass; suite green) | `bead82f` |
| T3 | S1: IC3, IC4, IC7; AC-IC3, AC-IC7 (primary key paths, ranges, bulk load) | inline | done (RED 4 of 4; code in the RDD correction) | `10c1740`, `7094255` |
| T4 | S1: IC5, IC6, IC7; AC-IC4, AC-IC6 (index keys, unique checks, entries, duplicate data) | inline | done (RED 3 of 3, folded texts) | `10c1740`, `d4b74ea` |
| T5 | S1: AC-IC5 (property test with a reference model, also in a temporary store) | inline | done (no RED: the code existed; mutant without folding killed) | `90ce084` |
| T6 | S1: IC8; AC-IC8 (statistics and estimates) | inline | done (RED: large fraction without the change) | `10c1740`, `65211fb` |
| T7 | S1: docs (guides), S2: IC12, AC-IC11 (bench hook and measurements) | inline | done (docs, example; Windows only without the 10 rounds, by the user, L12–L14; key fast path) | `cdb4df2`, `9d75c3b`, `cb1c412` |
| T8 | S4 (reviews, closing report, archive) | inline | done (verifier, Codex approved in round 3, RDD last slice approved; report) | `f6f0d88`, `22da401`, `61ee7bf`, `e5d9b8f`, `aca3dd3`, `1faea20`, `a6a8679`, `9974d0a`, `b33cc40`, report commit |
| T9 | S5 follow-up (L17): abandon the RDD lineage `review-b00b0f72ef61e47d` (false positive) | inline | done (L18) | — |
| T10 | S5 follow-up (L17): the seven advisories of the RDD last slice (fix or dismiss with evidence) | inline | done (L19) | `3373bc7`, `7056968` |
| T11 | S1: IC2, IC12 (L17): optimize the folding (stable runs copied, only unstable segments normalized) | inline | done (L20) | `154dfec` |
| T12 | S2 (L17): measure `cb1c412` (one-column key shortcut) and the folding optimization, Windows only | inline | done (L21) | — (`work/tc2`) |

## Log

- **L1** (2026-11-23, user): «Ya está subido. Haz la implementación con ODD en la rama, todas las preguntas al
  principio, después de la estimación de tiempo no preguntes, toma la decisión recomendada/conservadora.»
- **L2** (2026-11-23, user, on how to bring in the Unicode 17.0 data files): «Versionarlos todos (Recomendado)».
- **L3** (2026-11-23): exploration mapped the code (explorer handoff). Estimate 12 h – 18 h sent; start recorded at
  08:26:24 without a confirmation question, per L1. Unicode 17.0 files and the Unicode license downloaded from
  `https://www.unicode.org/Public/17.0.0/ucd/` and `https://www.unicode.org/license.txt`.
- **L4** (2026-11-23, decision): `modify_column` keeps its signature and expects `binary`;
  `modify_column_with_collation` takes the collation (conservative: no public signature changes).
- **L5** (2026-11-23): T1 (delegated, parallel) and T2 (inline) done. T1: RED 4 of 5 fold tests on the `todo!()`
  stub; GREEN 5 integration + 6 unit tests; generator deterministic. T2: RED the new test did not compile (34
  errors); GREEN 5 tests; suite green after moving the pinned format 14 to 15 (and the data-file hash of
  `bytes_tests`).
- **L6** (2026-11-23, RDD, T1–T2): assessed medium/`under_budget`, escalated to high with item 1 (the catalog
  encoding and format 15). The first START failed with `lens_context_budget_exceeded` (the Unicode data, 5.5 MB, and
  the generated tables, 229 KB, in the candidate), so the branch was rebuilt as data (`29fb1cc`), generated tables
  (`320e9a9`), T1 (`82c6d47`) and T2 (`bead82f`), and the review ran on `320e9a9..bead82f` (lineage
  `review-5fc999f939b056fe`, lenses risk, readability, reliability; consent granted per `CLAUDE.md`). Reliability
  found **R3-collation-not-applied** (critical, valid: the declared collation was not yet applied, which T3–T6
  would do); fixed as the one bounded correction `10c1740` (199 lines: folded `KeyColumn`, primary key paths,
  statistics, IC4). The correction changed scope, so the review recovered as lineage `review-b00b0f72ef61e47d` over
  `320e9a9..10c1740`: risk and readability completed; reliability raised **R3-missing-generated-tables** (blocker),
  a false positive: the generated tables and the Unicode data are in the review base (`git ls-tree -r c8be8bb`
  lists `crates/krummi/src/table/collation_tables.rs` and `crates/krummi/unicode/17.0.0/*`), which the reviewer's
  diff manifest does not show. No correction applies; abandoning the lineage needs the maintainer's approved
  binding, so the lineage stays `correction_required` and goes to the closing report as **Needs your decision**
  (per L1, no question during the run).
- **L7** (2026-11-23): T3–T6 done. The RDD correction `10c1740` carried the folding of T3, T4 and T6 (keys, unique
  checks, statistics, estimates, IC4). T4 added the original texts in the entry value after the included columns and
  the duplicate-key texts from them. T6: a text is large when it or its folded form passes 2,048 bytes. A first T6
  test put a 2,000-byte `ŉ` text in an indexed column, which IC7 rightly refused (3,000 folded); the index moved to
  the `ci_ai` column. Suite: 65 test binaries, no failure; clippy clean.
- **L8** (2026-11-23): T7, docs: the guides `data-types` (a *Collations* section with a doctest), `errors`, `tables`,
  `indexes` and `statistics` (`cdb4df2`); doctests 122 + 22 pass. Bench: the informative example `collation_measure`
  (`9d75c3b`); a quick check at scale 20 gave `ci` 2.6 (insert), 3.4 (by key) and 3.1 (by index) times `binary` on
  accented text with capitals. Measurement scripts in `work/tc` (adapted from `work/tj`), launched after the
  verifier and the Codex review so that their tests do not disturb the timings.
- **L9** (2026-11-23): independent verifier (sonnet, high; whole document; baseline `6b583a2`; probes in `mktemp -d`
  copies of HEAD and of the baseline): PASS for IC1–IC11, IC13, AC-IC1–AC-IC10 and AC-IC12, no blocker. A probe of
  28 texts over keys, two indexes, scans, ranges, sizes, statistics, estimates, schema changes and bulk loads on
  `binary` columns gave the same 166 output lines on the baseline and on HEAD. Advisory: no committed assertion of the
  shortened bounds of AC-IC8; pinned in `f6f0d88`. Clippy then caught two lints in the model test of `90ce084`,
  which ran before clippy covered it: fixed in `22da401`. Scratch removed.
- **L10** (2026-11-23): Codex code review of `6b583a2..22da401` ([report](../../docs/reviews/tables-collation/2026-11-23-code.md),
  [resolutions](../../docs/reviews/tables-collation/2026-11-23-code-resolutions.md)). Round 1: R1 (estimates decide a
  large value by the original), R2 (prefix statistics read large originals), R3 (an original kept twice), R4 (a
  borrowed key text): all fixed (`61ee7bf`, `e5d9b8f`, `aca3dd3`, `1faea20`), regressions red before the fixes.
  Round 2: R1, R3, R4 verified; R2 still open for defaults of added columns; R5 new (joint estimates ignored the large
  fraction my R2 fix produced): fixed (`a6a8679`, `9974d0a`). Round 3: approved. Checks green before each follow-up.
  Records committed in `b33cc40`.
- **L11** (2026-11-23): measurements launched (`work/tc/all.sh`: base `6b583a2` and head builds, Windows then WSL;
  one-round W1–W32, 10 rounds W23–W32, `collation_measure` 10 rounds). RDD last slice `320e9a9..b33cc40` assessed
  medium/`under_budget` (50 paths, 4,151 lines); its preflight STATUS failed twice with `operation_timeout`
  (`pre_native`, no mutation, `retry_safe`) while the builds ran; retried after the measurements so that it neither
  times out nor disturbs them.
- **L12** (2026-11-23, user, at 11:40 UTC, after asking how long the measurements would take): «No hagas la 10 rondas
  ni en window ni en linux». The 10 rounds of W23–W32 are dropped on both systems, against the 10–20 rounds of IC12
  (S2), by the user's decision: `work/tc/linux.sh` no longer runs them, and a watcher stops the Windows 10-round run
  with its process tree when it starts, so that `all.sh` goes on with `collation_measure` and Linux. The one-round
  pass of W1–W32 and `collation_measure` stay.
- **L13** (2026-11-23, user, at 11:43 UTC): «Estamos utilizando demasiado tiempo en las mediciones comparado con la
  implementación y los tests»; asked what to do with the rest (Windows only, `collation_measure` only, or as is), the
  user chose «Solo Windows (Recomendado)». Linux is not measured, against the «en Windows y en Linux» of IC12, by the
  user's decision: `work/tc/linux.sh` now exits at once (the former script is `linux.sh.skipped`). Windows keeps the
  one-round pass of W1–W32 and `collation_measure`. At that point code and tests had taken 59 min 37 s of active
  time and the measurements were forecast at about 3 h 20 min.
- **L14** (2026-11-23): Windows measurements done at 11:49:42 (base `6b583a2`, head `b33cc40`; the watcher stopped the
  10-round run at 11:48:41 with its process tree; Linux skipped, `work/tc/linux/skipped.txt`). One round of W1–W32
  (base/head speed ratio): 26 of 32 within ±3 % or faster; slower W8 0.836 and W9 0.772 (tree loads this change does
  not touch), W24 0.744, W1 0.933, W23 0.945, W5 0.952; faster with no cause W2 1.335, W28 1.343, W29 1.245: single
  rounds, mostly noise. Reading W23/W24 against the code found one real cost: the primary key path lost its
  one-column shortcut and collected the values of every key; restored in `perf` commit after the measurements (not
  measured). `collation_measure`, 10 rounds, 100,000 rows: insert `binary` 9,705 ns, `ci` 25,430 ns (2.62); by key
  1,625 / 4,577 ns (2.82); by index 3,837 / 9,934 ns (2.59).
- **L15** (2026-11-23, RDD last slice): final checks first on `cb1c412` (fmt; clippy no warning; 65 test binaries,
  1,257 passed, 0 failed, 35 ignored). `320e9a9..cb1c412` failed START with `lens_context_budget_exceeded` (no
  authority created). Reduced to `10c1740..cb1c412` (24 files, +1,945/−52; `10c1740` itself was covered by lineage
  `review-b00b0f72ef61e47d`, risk and readability completed): lenses risk, readability, reliability, consent granted
  per `CLAUDE.md`, lineage `review-b2a93e2d2f57646d`: **approved**, non-blocking advisories, none acted on:
  R1-001 SUGGESTION (`collation_measure` deletes its `binary`/`ci` subfolders of the given work folder without a
  warning); R2-001 WARNING (`EntryLayout::original` turns a malformed payload into `None`, read as "not collated");
  R2-002 SUGGESTION (original-text slots use the placeholder column id 0 without a comment); R2-003 SUGGESTION (stored
  values and old-row defaults are classified large differently: original length only against `is_large`);
  R2-004 SUGGESTION (`_ if large` arm before the frequent lookup); R2-005 SUGGESTION (a four-way zip in the prefix
  gathering); R3-invalid-len-counted-large WARNING (a damaged row with an invalid length now counts as large in the
  prefixes, where it added nothing before).
- **L16** (2026-11-23): closing report [tables-collation-report.md](tables-collation-report.md); phase log ended at
  12:11:49. Pending: the RDD lineage `review-b00b0f72ef61e47d` (Needs your decision), the measurements left out by
  the user, the RDD advisories of the last slice.
- **L17** (2026-11-23, user, after the closing report): «Mide El arreglo que restauré después de medir (cb1c412, el
  atajo para claves de una sola columna) no está medido. Implementa optimización del plegado. Revisa los siete
  avisos, abandona la revisión de falso positivo.» Estimate 2 h – 4 h sent; the user answered «Sí»; resumed at
  12:36:15 (execution reopened with `move_docs.py reopen`). Defaults taken without asking (L1, L13): measurements on
  Windows only, 5 rounds of the loads the shortcut touches (W23, W24 and W28: insert and update through
  `encode_key_of`) A/B `b33cc40` against `cb1c412`, and `collation_measure` before and after the folding
  optimization.
- **L18** (2026-11-23, T9): `gentle-ai review abandon` of `review-b00b0f72ef61e47d` (reason `operator_disposition`,
  actor the user, binding from `gentle-ai review status` entries: revision `sha256:921ac8ae…`, snapshot
  `sha256:d2ea5281…`, captured lens results risk, readability, reliability, findings present): `committed` at
  12:37:00, moved to the audited quarantine, nothing deleted. Its predecessor `review-5fc999f939b056fe` (the valid
  finding, fixed in `10c1740`) and `review-ed0f0e4daf403899` (not of this run) stay `correction_required`: not asked.
- **L19** (2026-11-23, T10, the advisories of lineage `review-b2a93e2d2f57646d`): R1-001 fixed, the usage of
  `collation_measure` warns that it deletes its `binary` and `ci` subfolders (`7056968`). R2-001 fixed,
  `EntryLayout::original` returns `Some(None)` for a value that does not decode, so that `IndexEntry` panics with the
  message of a malformed entry and a duplicate key reports `(?)` (IX12), instead of falling back to the folded key;
  internal test extended (`3373bc7`). R2-002 fixed, a comment on identity 0 of the slots of original text
  (`3373bc7`). R2-003 fixed, a default is measured by its original length, as a stored value is, with the reason a
  folded form over the maximum never reaches an index key (IC7) (`7056968`). R2-004 fixed, `if`/`else` instead of the
  guarded wildcard arm (`7056968`). R2-005 fixed, one loop per prefix instead of a four-way `zip` (`7056968`).
  R3-invalid-len-counted-large dismissed with evidence: the column gathering already counts a malformed length as
  large and does not read it ("a large value or a malformed one counts as large and is not read",
  `crates/krummi/src/table/statistics.rs:212`, the ST1 rule of the damaged-run reads); the prefixes now follow
  the same rule, which keeps the fractions of the column and its prefixes consistent. Checks: clippy clean;
  `table_collation` 21, `table_statistics` 31, `table_indexes` 48 tests pass.
- **L20** (2026-11-23, T11): `fold` now copies the stable scalars (lowering ASCII capitals) and runs `fold_slowly` only
  over each run of unstable scalars, from the simple scalar before it (a base its marks may compose with or follow)
  to the next simple one (`154dfec`). Correctness: a stable scalar is a starter, no second of a composition, and
  folds to itself, so it blocks reordering and composition across it. New unit test: 20,000 random mixes give the
  same result by segments and whole (killed a mutant that started a segment at the unstable scalar; a mutant that
  started it at the first unwritten scalar survived because it is equivalent, only slower). AC-IC2 (every scalar,
  `NormalizationTest.txt`, random sequences against the reference) passes through the new path. Quick check, scale 20,
  3 rounds: `ci` 1.56–1.59 times `binary`, from 2.6–3.4 before.
- **L21** (2026-11-23, T12, Windows, bench power plan, `work/tc2/all.sh`, 12:52:27–13:09:15): 5 rounds of W23, W24 and
  W28, base `b33cc40` (before the shortcut) against head `154dfec` (base/head speed ratio): W23 1.016, W24 1.002, W28
  1.004, all within noise; the 0.744 of W24 in the single round of L14 was noise. `collation_measure` on `154dfec`,
  10 rounds, 100,000 rows: insert `binary` 8,906 ns, `ci` 13,598 ns (1.527, was 2.62); by key 1,576 / 2,382 ns (1.511,
  was 2.82); by index 3,798 / 5,544 ns (1.460, was 2.59).
- **L22** (2026-11-23): Codex review of the follow-up, `9974d0a..154dfec`
  ([report](../../docs/reviews/tables-collation-followup/2026-11-23-code.md)): no functional defect; R1 minor, the
  links of the report and of this log kept the depth of `odd/done/tables`. Cause: `move_docs.py reopen` stages its
  renames, and `3373bc7` took them with the code, while the link rewrites stayed in the working tree. Fixed by
  committing the documents as reopened.
