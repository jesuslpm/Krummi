# Table column projections, run pool and short checksum — ODD execution document

Objective: implement the prepared column projection of `docs/prd/krummi/tables/tables-projection.prd.md` (v1.0), and
two optimizations from the 2026-11-15 performance diagnostic with no product change (no PRD): M1, a pool of value run
buffers in the store, and M2, the benchmark's short checksum in both engines. Branch: `feat/table-projection` (from
`main`). Delivery: `ask-on-risk` (single branch, left unmerged). Test runner: `cargo test --workspace` (Windows; Linux
in WSL 2 Ubuntu). Estimate: 7 h – 11 h. Real start: 2026-11-15 17:51:45 +0000.

## Specs

- **S1** — The projection PRD, `docs/prd/krummi/tables/tables-projection.prd.md` v1.0, and its ancestry up to the
  root: decisions PY1–PY7, *Validaciones y mensajes*, *Salida*, *Fuera de alcance* and *Dejado a la implementación*,
  verbatim there. Its acceptance criteria AC-PY1–AC-PY10 and AC-PY12 are the tests of the library; AC-PY13 (the
  annotations) is done in `a7043fc`.
- **S2** — PY7 and AC-PY11 of the same PRD: W26 and W27 read with a projection, prepared inside each repetition's
  measured time, with a test hook that shows it and a way to run W27 without it for the measurement of AC-PY11; full
  benchmark runs on Windows and Linux compared with the last ones (Windows 2026-11-13, Linux 2026-11-12). AC-PY11's
  single-load clause is met, for the loads it misses, by the Krummi-only A/B before and after the change (the PRD's
  annotation of 2026-11-16, user's decision, L19).
- **S3** — M1, decided by the user on 2026-11-15 from the diagnostic (`docs/performance/2026-11-15-diagnostic/`,
  follow-up section 4): the private buffers of value runs are kept after a commit for the runs of the next writes;
  «Sí en el almacén»: the pool lives in the store, not in a thread, and closing the store frees it; at most 32 MiB of
  idle buffers per store, a fixed constant (the user's «sí» to the recommended 32 MiB); a reused buffer is zeroed and
  passes the allocation fault point as a new one would; the pool's own bookkeeping reserves its memory fallibly, and
  keeping a buffer never fails nor aborts the operation: a buffer that cannot be kept is freed.
- **S4** — M2, decided by the user on 2026-11-15 (follow-up section 1): the benchmark's byte sum takes a short path
  for up to 16 bytes in both engines, Rust a scalar sum and C# SSE2 for exactly 16 bytes and a scalar sum below; the
  same checksum as the current one for every input, proved by equivalence tests.
- **S5** — The project's checks (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`), RDD on each work-unit commit, the Codex code review over the whole range, and the closing
  report.

## Execution design

- **Projection identity (PRD *Dejado a la implementación*).** A table's schema is identified by the store instance
  (a serial unique in the process, drawn when the store opens), the table's identity and its schema state (P12). Every
  table reader carries them; binding a row compares the three numbers. A copy of the files opened as another store has
  another serial; an abandoned write's schema state is random and never equals another's.
- **Cache.** The store owns a cache of at most 1,024 projections keyed by table, schema state and the list's column
  positions. A projection holds the reader that prepared it (columns, layout, defaults), a plan per listed column for
  rows of the current version, and plans for each older version built on the first row of that version.
- **Prepared columns.** `ProjectedColumn<T>` carries the projection's key (shared by every preparation of the same
  list) and its position; reading compares the key by address first, then by content, and panics when it differs.
  `T` is the column's Rust type where it is unique and a marker otherwise; decimals, uuids and vectors accept any of
  their parameters (PRD *Dejado a la implementación*).

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S4 (M2): short byte sum in Rust and C#, equivalence tests | inline | done (equivalence: `the_short_byte_sum_equals_the_sum_of_every_byte` and the Voron selftest `byteSum(0..33 bytes) ok`) | `0175298` |
| T2 | S3 (M1): run pool in the store | inline | done (RED: `run_pool_tests` did not compile without the pool; GREEN 4 of 4; mutation without zeroing fails `a_commit_keeps_its_run_buffers_and_the_next_run_gets_one_zeroed`; krummi-tx and krummi-pager suites pass) | `bee2dcb` |
| T3 | S1: projections in the library (prepare, cache, prepared columns, bind, read) with AC-PY1–AC-PY10, AC-PY12 | inline | done (RED: the tests did not compile without the API; GREEN: `table_projections` 10 of 10 and `projection_tests` 2 of 2 in debug and release; mutations without the schema check and with the current plan on older rows fail AC-PY5, AC-PY10 and AC-PY2; workspace suite passed) | `9c9cba6`, `2277110` |
| T4 | S2: W26 and W27 read with a projection, test hook, W27 without it | inline | done (`w26_and_w27_read_with_a_projection`) | `5b17ff0`, `2277110`, `c00f2dc` |
| T5 | S2 (AC-PY11): full runs on Windows and Linux | inline (background runs) | done (L9, L10, L19: AC-PY11 met with the A/B, by the user's decision on R1 of the Codex code review; 29 of 29 on both; W27 0.91 → 0.96 and 0.95 → 1.04; drops checked alone and with a main/head A/B) | reports in the T6 commit |
| T6 | S5: final checks, RDD of the last slice, Codex review, closing report | inline | done (L12–L21; [closing report](table-projection-report.md)) | `3d296fb`..`3c7cb4b`, the report commit |

## Log

- **L1** (2026-11-15) — The user's request, verbatim: «Escribamos el PRD y luego arranquemos todo.» — the projection
  PRD, then one run with the projection, M1 and M2. The estimate went to the user as «Para la ejecución completa (la
  proyección, M1 y M2) estimo **entre 7 h y 11 h**. ¿Empiezo?», and the user answered «Sí» (start 17:51:45 +0000).
- **L2** (2026-11-15) — `a7043fc` commits the PRD, its translation, its Codex review, the annotations (AC-PY13) and the
  copy of the diagnostic. Exploration: rows read by position through `TableRow::typed` (`rows.rs:1011`); readers are
  built per transaction (`catalog.rs:178`, `:269`, `:508`); the engine already keeps page payloads for the next copies
  up to 32 MiB per store (`krummi-tx/src/lib.rs:136`, `:258`), the precedent M1 follows.
- **L3** (2026-11-15) — T1: `byte_sum` takes a scalar `u32` sum up to 16 bytes; `Data.ByteSum` one SSE2 step at 16
  bytes and a scalar sum below; the Voron `selftest` gained `byteSum(0..33 bytes)`, which printed `ok` (run with
  `dotnet run -c Release -p:RavenDbRoot=work/ravendb-7.2.7 -- selftest`). RDD assess (base `main`, committed only):
  `medium`, `under_budget`.
- **L4** (2026-11-15) — T2: the engine keeps up to 32 MiB of value run buffers per store on disk after
  `writer.commit` copied them (`Engine::keep_runs`), handed zeroed through `reuse_run`, the allocation fault point of
  `alloc_run`; the list grows with `try_reserve` and a buffer that cannot be kept is freed. A temporary store keeps
  none, as for the page payloads (P10). RDD assess: `medium`, `under_budget`.
- **L5** (2026-11-15) — T3 decisions within *Dejado a la implementación*: names `ReadTransaction::projection`,
  `WriteTransaction::projection`, `Projection::column::<T>`, `Projection::bind`, `ProjectedRow::get`; `T` is the Rust
  type of the column or a marker of `krummi::table::projection` (`Text`, `TextBytes`, `Bytes`, `Date`, `Time`,
  `Timestamp`, `Duration`, `Vector`); `Decimal`, `Uuid` and `Vector` accept any precision and scale, key order,
  element and dimension. Binding fails with the new `SchemaDifference::Projection`; an empty list with the new
  `ValueRule::EmptyProjection`. The typed reads of `TableRow` now use the same `ReadAs` decoders (DRY). Test hooks:
  `Projection::is_kept`, `Projection::same_as`, `Store::kept_projections`.
- **L6** (2026-11-15) — T4: W26 and W27 read through `read_by_key` and `read_ranges`, which prepare the projection
  inside the measured time; `KRUMMI_BENCH_PROJECTION=off` reads by position (diagnostic hook for AC-PY11). Risk of
  T3: item 3 (the typed reads of `TableRow`, which every existing column read uses, now go through the projection
  decoders) and item 4 (a store-wide cache behind a mutex, older plans in `OnceLock`).
- **L7** (2026-11-15) — RDD of `a7043fc..4774ba4` (M2, M1, the projection, the benchmark; assessed `high` by the item 3
  escalation), lenses risk, readability and reliability: approved, lineage `review-4976a6e6cdfe7836`, acknowledged
  (authority burned). Advisory findings: R2-001 (WARNING, this document listed T3 and T4 as pending): updated here.
  R2-002 (SUGGESTION, `key` meant both the list and the row key in `ProjectedRow::get`): renamed `list`. R2-003
  (SUGGESTION, the benchmark's prepared columns by literal index): taken by the position of their names. R3-001
  (WARNING, a kept projection prepared again after a rename named the old table in its errors): RED in
  `ac_py5_rows_of_another_table_store_or_schema_do_not_bind` (`left: "t"`, `right: "renamed"`), fixed by keeping the
  name in each handle. All in `2277110`. Risk lens found nothing.
- **L8** (2026-11-15) — A/B of W26 and W27, both engines, two rounds pinned to the performance cores under the bench
  power plan (`work/tp/ab.sh`, `work/tp/ab-*.tsv`): with the first plan W27 read **slower** with the projection,
  8.89 M rows/s against 9.20 M by position in both rounds (−3.4 %); W26 even. Cause: each read followed the list to the
  reader's columns for the type and matched the slot's place again. `c00f2dc` puts the type, the live position and,
  for a fixed-width column, its byte range and null bit in each column's plan. A/B of Krummi alone after it, three
  alternated rounds (`work/tp/ab2.py`, `work/tp/ab2-opt.out`): W27 95.2 ns per row with the projection against
  106.1 by position (−10.3 %, all three rounds); W26 1,351 against 1,372 ns (−1.6 %). Tests: `table_projections` 10 of
  10, `projection_tests` 2 of 2; clippy clean.
- **L9** (2026-11-15) — T5 (AC-PY11), full runs of `f4b6c2f` with `--solo-krummi-voron` under the bench power plan
  (`work/tp/final-runs.sh`): Windows pinned to the performance cores, 1,262 s, and WSL 2, 2,685 s, both **29 of 29**
  (`docs/benchmark/2026-11-15-windows-projection.md`, `docs/benchmark/2026-11-15-linux-projection.md`). The data
  digests of both engines match in every load (the orchestrator stops on a mismatch). W27's ratio with Voron rises
  on both: Windows 0.91 → 0.96, Linux 0.95 → 1.04. Against the previous full runs (Windows 2026-11-13, Linux
  2026-11-12), these loads drop more than 0.05. Windows: W1 1.47 → 1.40, W9 1.27 → 1.15, W12 1.15 → 1.08,
  W13 1.22 → 1.10 and W23 1.47 → 1.34. Linux: W9 1.24 → 1.16, W18 1.63 → 1.44, W21 0.95 → 0.88, W22 1.17 → 1.10,
  W26 1.10 → 0.97 and W29 1.90 → 1.84. Each of them alone, both engines, three rounds with the engines' order
  alternating (`work/tp/solo.py`, `work/tp/solo-windows.out`, `work/tp/solo-linux.out`; digests match), median
  ratio:
  - Windows: W1 1.40, W9 1.15 (rounds 0.93–1.32), W12 0.99 (0.69–1.09), W13 1.03 (0.73–1.13) and W23 1.40.
  - Linux: W9 1.07, W18 1.34, W21 0.89, W22 1.13, W26 1.04 and W29 1.85.

  Only W22 and W29 come back within the margin. On Windows Krummi does not drop in W1 (966 k against 953 k) or in
  W23 (469 k against 467 k): Voron rises (691 k against 646 k, 335 k against 317 k). W9 and W12/W13 spread more
  across rounds than the drop. To separate the branch from the machine and from Voron, Krummi alone, `main`
  (`28f9d3b`) against `f4b6c2f`, five alternated rounds (`work/tp/abk.py`): Windows W1, W9, W12/W13 and W23,
  Linux W9, W18, W21 and W26.
- **L10** (2026-11-15) — A/B of Krummi alone, head over `main`, median of five rounds, digests match
  (`work/tp/abk-windows.out`, `work/tp/abk-linux.out`). Windows: W1 1.025, W9 1.019, W12 1.103, W13 1.090 and
  W23 0.996. Linux: W9 1.044, W18 1.150 (rounds 0.81–1.19), W21 1.001 and W26 1.002. The branch makes no load slower;
  the drops of L9 are Voron's rises and the machine's variance between days. **Decision taken during the run**
  (listed in the closing report): AC-PY11 is taken as met. W27 rises on both systems and the digests match. The
  single-load clause, read literally, holds only for W22 and W29; for the other drops this A/B, which isolates the
  branch, stands in for it. T5 done.
- **L11** (2026-11-15) — AC-PY12 differential: `main` (`28f9d3b`) writes W23's store at reduced scale; `f4b6c2f`
  runs W26 and W27 on it with the projection and with `KRUMMI_BENCH_PROJECTION=off`. All four exit 0 (the worker
  checks each read's checksum) with the same digests (W26 `f9ff3b9819b073e3`, W27 `5a536accc3f3dfb3`), which are also
  the digests of a store written by `f4b6c2f`. Scratch in a `mktemp -d` folder, removed.
- **L12** (2026-11-15) — Final checks at `704d18b`: `cargo fmt --all --check` clean, clippy clean, `cargo test
  --workspace` 851 passed, 0 failed, 34 ignored. Independent verifier (Verify role, sonnet/high, for Risk items 3 and
  4), in `mktemp -d` copies: **no blockers**; S1–S5 and items 3 and 4 pass. Item 3: a differential of the 18 typed
  accessors, `text_bytes`, `ulid` and `value()` on every position of every column type, over 8 schema versions
  (added columns with defaults, a dropped column, nulls) and every row path, gives byte-identical output at `28f9d3b`
  and HEAD (998 lines, wrong-type panic text included). Item 4: 1 writer changing the schema and 6 readers
  preparing, binding and reading projections (no mismatches, binding refused exactly when the schema changed); 8
  threads preparing 1,452 lists twice keep exactly 1,024; old-version plans shared by 6 threads; the run pool with 4
  readers holding snapshots (reused buffers zeroed, old snapshots unchanged; the probe fails with `fill(0)` removed).
  Advisories, kept as they are: a pooled run is reused only at the same length (performance only); projections
  prepared in aborted write transactions keep their cache slots (as PY5 allows); a projection read of a malformed
  row may panic with an index error instead of "a malformed row" (PY6 allows either; AC-PY9 passes). Scratch removed.
- **L13** (2026-11-16) — RDD of the last slice `4774ba4..704d18b` (lenses readability and reliability, chosen for
  the plan and read code and the reports): **approved**, lineage `review-0f3dc3663cbdd4a8`, acknowledged and burned.
  Five advisory findings, none blocking. R2-001 (WARNING, `projection.rs` the fixed-section read repeats
  `slot_bytes`'), R2-002 (SUGGESTION, `Slot::fixed` returns an unnamed 4-tuple) and R3-002 (SUGGESTION, no test of a
  nullable fixed-width column through a projection): **fixed** in the next commit. `FixedSlot` in `row.rs` is the one
  read of the fixed section, used by `RowLayout::slot_bytes` and `Access::Fixed`; AC-PY2 now reads the nullable `z`,
  null and not null, in rows of an older version and of the current one (it passed before the refactor too). No A/B
  run: the hot paths do the same work (one place match, one null-bit test, one slice). R2-003 (WARNING) and R3-001
  (SUGGESTION), "Krummi: desconocido" in the Linux report: **kept**. WSL builds from `git archive` of `f4b6c2f`, which
  carries no `.git`, so the bench cannot read the commit; earlier Linux reports say the same, and L9 names the
  commit. Checks after the fix: fmt and clippy clean, `cargo test --workspace` 851 passed, 0 failed, 34 ignored.
- **L14** (2026-11-16) — RDD of `704d18b..3d296fb` (assessment: medium, `under_budget`; reviewed as the last slice
  before the Codex review; lenses readability and reliability): **approved**, lineage `review-ba947c7922df2d79`,
  acknowledged and burned. One advisory finding: R2-001 (SUGGESTION, deterministic, `row.rs` `slot_bytes` kept a
  `Fixed` match arm that the early `FixedSlot` return made unreachable) — **fixed** in the next commit: `slot_bytes`
  matches the place once and reads a fixed column through `Slot::fixed_at(..).read`, with no dead arm. Checks: fmt and clippy clean, `cargo test --workspace` 851 passed, 0 failed, 34 ignored.
- **L15** (2026-11-16) — Codex code review, round 1, over `28f9d3b..daccfcd`:
  [2026-11-16-code.md](../../docs/reviews/tables-projection/2026-11-16-code.md), status `findings`. R1 (blocker,
  AC-PY11 accepted on the A/B of L10 instead of its single-load clause) → pending, a product question for the user
  after the loop. R2 (major, a repeated schema state of an abandoned write, or a deleted table's identity, aliases
  another schema) → fixed in the next commit, with `ac_py5_a_repeated_identity_never_aliases_a_schema` (RED before
  the fix, two mutants fail). Resolutions:
  [2026-11-16-code-resolutions.md](../../docs/reviews/tables-projection/2026-11-16-code-resolutions.md).
- **L16** (2026-11-16) — Independent verifier of `3a548b2` (Verify role, sonnet/high, Risk items 3 and 4): **no
  blockers**. Forced repeats (abandoned create, abandoned change three rounds, committed and same-write delete then
  create, create and change in one abandoned write, a reopened store) all refuse the bind with `DifferentSchema`
  (`Projection`) and do not share the kept preparation; at `daccfcd` the abandoned-change cases bind (R2). A
  differential of the identities drawn on paths with no retired value, D6's sequence included, is identical at both
  commits; a new table's state may still repeat another table's. The lock is taken for one set operation at a time,
  with no user code under it. `table_projections` 11/0, `tables` 26/0. Advisories, kept: the retired sets grow by
  one entry per table created or deleted and per schema change while the store is open (documented); a delete in an
  abandoned write leaves the table's identity retired, with no observable effect since the catalog still holds it.
  Scratch removed.
- **L17** (2026-11-16) — Codex code review, round 2 (follow-up, `daccfcd..df72e3f`):
  [2026-11-16-code-02.md](../../docs/reviews/tables-projection/2026-11-16-code-02.md), status `findings`. R2 verified
  resolved, no regressions of the fix. R1 still open: the single-load clause of AC-PY11 is not met, and the A/B of
  L10 does not replace it without the user's approval. T5 reopened as pending that decision; the question goes to
  the user after the loop. No further round: the only open finding is a product decision.
- **L18** (2026-11-16) — RDD of the last slice `3d296fb..605dd96` (assessment of the R2 fix; lenses risk, reliability
  and readability): **approved**, lineage `review-7b867378ec08eb40`, acknowledged and burned. Advisory findings:
  R2-001 (WARNING, the comment of the deleted-table case said it draws the deleted identity and state again, and the
  test did not check which identities were used) and R3 (WARNING, no committed test of a table created in an abandoned
  write) — **fixed** in the next commit: the comments say the generator offers the value again and the store draws the
  next one, each case asserts the identities drawn (state 60; table 110 with state 130), and a third case covers the
  abandoned create (table 210 with state 230); without `retire_table` in `create_table` it fails. R1-001 and R3
  (SUGGESTION, the retired sets only grow while the store is open) — kept: one entry per table created or deleted and
  per schema change, documented on `Retired`. R2-002 (SUGGESTION, the retired sets live on `Projections`, the cache
  type) — kept: `Projections` is the store-level state every transaction already reaches, and the sets exist only so
  projections and rows never alias; their doc says so. Checks: fmt and clippy clean, `cargo test --workspace` 852
  passed, 0 failed, 34 ignored.
- **L19** (2026-11-16) — User's answer to R1 of the Codex code review (AC-PY11), verbatim: «Aceptar el A/B
  (Recommended)», the option "Anoto en el PRD que AC-PY11 se da por cumplido con el A/B main/head de Krummi solo,
  cierro T5 y lo dejo en el informe de cierre." AC-PY11 of
  [tables-projection.prd.md](../../docs/prd/krummi/tables/tables-projection.prd.md) and its translation now carry the
  annotation of 2026-11-16; S2 rewritten; T5 done. The wait for the answer is in the phase log.
- **L20** (2026-11-16) — Codex code review, round 3 (follow-up, `df72e3f..dc35e10`):
  [2026-11-16-code-03.md](../../docs/reviews/tables-projection/2026-11-16-code-03.md), status **approved**. R1 verified
  resolved (the user's decision, annotated on AC-PY11); R2 verified in round 2. No finding open. Three rounds, the
  loop's limit.
- **L21** (2026-11-16) — RDD of the last slice `605dd96..f2566fb` (the strengthened test, the AC-PY11 annotation and
  the review records; lenses readability and reliability): **approved**, lineage `review-8aaa0b251c91e621`,
  acknowledged and burned. R2-001 (WARNING, the expected identities (110, 130) and (210, 230) were unexplained: the
  comments did not say that a creation draws one identity per column between the table's and the state) and R3-001
  (SUGGESTION, the abandoned create's own identities were not asserted) — **fixed** in the next commit: the comments
  give the draw order and the first table of each case asserts its identity and state ((100, 120), (200, 220)).
  Final checks after it: `cargo fmt --all --check` clean, clippy clean, `cargo test --workspace` 852 passed, 0
  failed, 34 ignored.
- **L22** (2026-11-16) — Closing report: [table-projection-report.md](table-projection-report.md). The phase log got
  an `end` at 2026-11-15 21:14:57 and a `mark fix` at 2026-11-16 06:21:09, both from the session history: the session
  stopped while a background command waited on standard input, and resumed with the user's message; without them the
  9 h 06 min pause counted as fixes.
