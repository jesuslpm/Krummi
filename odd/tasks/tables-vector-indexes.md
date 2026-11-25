# Approximate vector indexes — ODD execution document

Objective: implement `docs/prd/krummi/tables/tables-vector-indexes.prd.md` (7.6: IVF-SQ8 vector indexes, asynchronous
maintenance, filter-first k nearest neighbours, public exact search). Branch: `feat/tables-vector-indexes` (from
`main` at `7793a51`). Delivery: `ask-on-risk` (single branch, left unmerged). Test runner: `cargo test --workspace`
(Linux only, Kubuntu). Estimate: 26 h – 40 h. Real start: 2026-11-25 13:34:26 +0000.

## Specs

- **S1** — The PRD `docs/prd/krummi/tables/tables-vector-indexes.prd.md` and its ancestry up to the root: decisions
  VI1–VI16, *Validaciones y mensajes*, *Salida*, *Fuera de alcance* and *Dejado a la implementación*, verbatim there.
  Its acceptance criteria AC-VI1–AC-VI17 are the tests of the library, except the Windows parts (S2).
- **S2** — The user's decision on Windows (L2): «Solo Linux, anotado»: the PRD is annotated so that this run validates
  only on Linux and Windows stops being a criterion; the PRD can close without Windows. AC-VI10 becomes the
  determinism of the same file and the same data on Linux (and a golden test of the kernels' bits); AC-VI11 and VI15
  are measured on Linux only.
- **S3** — VI15 on Linux: the thresholds of document 12 with the Cohere data of the experiment (1 M × 768, L2, one
  thread, warm cache, no pending rows, idle worker): recall@10 ≥ 0.95 with p95 ≤ 20 ms without a filter; the p95 of
  inserting one row with the index ≤ 1.10 × without; a worker batch holds the turn ≤ 15 ms. The informative pgvector
  comparison (IVFFlat, `ivfflat.iterative_scan`, `max_parallel_workers_per_gather = 0`, same machine and data, a
  filter that passes few rows and one that passes many, insert p95, `SELECT 1` p95, versions). One round of every bench
  load before and after, and 10 to 20 rounds of W23–W32 and of any load more than 3 % apart.
- **S4** — The technical starting state: `docs/design/tables/12-vector-indexes.md` (*Recommended design*) and
  `05-vectors.md` (*Decided design*, kernels), with the three parts the PRD changes (filters, rebuilds, HNSW criteria).
- **S5** — The project's checks (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`), RDD on each work-unit commit, the Codex code review over the whole range, and the closing
  report.

## Execution design

- **Kernels.** `table/distance.rs`: document 5's exact kernels over the stored bytes (`f32`, `f16`, `i8`, `bit`), 16
  `f64` lanes in the fixed order of `vector_norm`, `i8` exact in `i32`, Hamming with `count_ones`; and `f32` work kernels
  in the same fixed order for training, assignment and SQ8 (ported from `krummi-ann-lab`). Distances in VI8's units;
  order and ties by the compared value (the square for L2), then by primary key.
- **Storage.** Each vector index is in the table's catalog entry (after the secondary indexes and the statistics
  part): identity, name, column identity, metric, an instance identity redrawn when it is created and when the table is
  emptied, the root and entry count of its `pending` tree (primary key → empty), and the root of its `data` tree. The
  data tree holds, by a leading tag byte: the state record (state, active and building generations, cursor, rebuild
  request, rows when trained), each generation's header (lists, quantizer, counters), its centroids, its postings
  (`gen ‖ list ‖ pk` → SQ8 code), its locator (`gen ‖ pk` → list), its list sizes, and generations retired by a
  publication, which later batches delete a bounded slice at a time. Format 16.
- **Foreground.** Row writes upsert `pending[pk]` for each vector index (an update only when the vector's bytes
  change), in the write's transaction; emptying, dropping and the bulk load follow VI10.
- **Worker.** One thread per store for the vector indexes, started at open when an index has work and woken by row
  writes and index changes; it trains (sample in short reads, single-thread k-means and quantizer), backfills from a
  cursor, reconciles pending keys, publishes and rebuilds, each batch computed in a read and applied in a deferred write
  whose apply loop stops at a time budget inside 15 ms. Identity checks by table, index and instance drop stale batches.
- **Search.** The filter is evaluated first (a secondary index narrows it when one leads with a filter column, else the
  rows are scanned), computing exact distances while the passing rows stay under the threshold; above it, IVF probes
  the nearest lists over the passing set, skipping pending keys, widening the probe until k candidates, and reranks
  with the exact kernels, adding the pending rows' exact distances.
- **Routes.** The parent works inline, one task per commit; the feature document and its Engram mirror kept current.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S2 (PRD annotation of the Linux-only decision, ES and EN) and the worktree hook made POSIX for Linux | inline | done | `69918a0`, `d9fe1da` |
| T2 | S1: VI2, VI8 (distance kernels), AC-VI12, AC-VI10 golden bits | inline | done | `caa533e` |
| T3 | S1: VI7, VI16 (filters, public exact search, errors), AC-VI12, AC-VI13 (nulls) | inline | done | `caa533e` |
| T4 | S1: VI1, VI10, VI14 (definitions, catalog, Describe, schema changes, format 16), AC-VI1, AC-VI9, AC-VI14 (format) | inline | done | `c2fbfa0` |
| T5 | S1: VI3 foreground (pending log in every row write, bulk load), AC-VI2 (pending part) | inline | done | `15b67d3` |
| T6 | S1: VI3, VI4, VI12, VI13 (worker, training, build, reconcile, close and open, failures), AC-VI3, AC-VI4, AC-VI14, AC-VI15, AC-VI17 | inline | done | `8ee9045`, `597fee2` (RDD correction), review fixes and AC-VI4/AC-VI14 tests `308516e` |
| T7 | S1: VI5, VI6 (approximate filter-first search), AC-VI2, AC-VI5, AC-VI6, AC-VI7, AC-VI13 | inline | done | `9986904` |
| T8 | S1: VI9, VI11 (information, generations, automatic and requested rebuilds), AC-VI8, AC-VI16, AC-VI17 | inline | done | code in `8ee9045`, tests `6b06454` |
| T9 | S3 (measurements: threshold, defaults, VI15 on Linux, pgvector, bench rounds), AC-VI11 | inline | done | `4d82143`, `2fad948`, `0cd926c`, `3d4ab30`, `96bb348` (L14–L17) |
| T10 | S1 (guide and reference docs) | inline | done | `587907e` |
| T11 | S5 (RDD of the last slice, Codex review, closing report, archive) | inline | blocked: Codex review waits for the user (L21); RDD done, [report](tables-vector-indexes-report.md) written | |

## Log

- **L1** — 2026-11-25, user's request, verbatim: «Implementa el PRD tables-vector-indexes.prd.md pero ten encuenta que
  estamos en Kubuntu, no podemos hacer pruebas ni mediciones en windows.»
- **L2** — 2026-11-25, product question before the run: what to do with the Windows parts (AC-VI10, AC-VI11, VI15).
  Answer, verbatim: «Solo Linux, anotado» (option: «Anoto en el PRD que esta ejecución valida solo en Linux y que lo de
  Windows deja de ser un criterio. El PRD se puede cerrar sin Windows.»). Estimate sent: 26 h – 40 h; confirmation,
  verbatim: «Sí, arranca».
- **L3** — Before the run, the project's worktree guard hook failed on Kubuntu (`/bin/sh` is `dash`, and the hook used
  the bash-only `${d//"$b"//}`), blocking every Bash, Write and Edit call; the user replaced it with a POSIX `tr`, as
  proposed. Exploration and the product question were not in the phase log (it started at the confirmed start).
- **L4** — Defaults taken during the run (CLAUDE.md: no questions once started): pgvector runs in Docker
  (`pgvector/pgvector` image) on the same machine, which needs no `sudo`; the Cohere data is downloaded again from the
  public VectorDBBench replica into the ignored `work/` folder.
- **L5** — T1 done. RDD assessment of `7793a51..d9fe1da`: medium, `under_budget` (configuration change), not
  reviewed; the reviewed boundary stays at `7793a51`. The Engram mirror is pending: auto mode could not check the
  `mem_save` call because a hook changes its input.
- **L6** — T2 and T3 in one commit (`caa533e`: the work kernels wait for T6 under `allow(dead_code)`); the engine's
  f32 kernels give the golden bits the experiment took on win-x64. RDD assessment `7793a51..caa533e`: medium,
  `under_budget`.
- **L7** — T4 done (`c2fbfa0`): vector index definitions share the name space and the 64-index limit with the secondary
  ones; `drop_index` drops either kind; emptying gives each vector index new trees and the next instance. Format 16:
  the tests that pinned 15 now pin 16 and 15 is refused. Known environmental failure, also on `main` at `7793a51` with
  this machine's rustc: 7 `trybuild` mismatches of `tests/ui/trees/*_cannot_be_sent_to_another_thread.rs`
  (`trees_compile`), a diagnostic text difference. Items the next tasks use carry a temporary `allow(dead_code)` with
  reason «used by the worker and the searches (T5–T8)»; remove them as they are used.
- **L8** — RDD reviews (the whole `7793a51..c2fbfa0` passed the reviewer's context budget, `lens_context_budget_exceeded`,
  so it was split; the review takes `base..HEAD`, so the first slice was reviewed with HEAD detached at `caa533e`):
  - `7793a51..caa533e` (hook, PRD annotation, kernels, filters, exact search), lenses risk and reliability, approved, no
    findings, lineage `review-65892a7ace81cb36`.
  - `caa533e..c2fbfa0` (format 16, definitions; escalated with item 1, a persisted format change), lenses risk and
    reliability, approved with two warnings, lineage `review-7fa2180227cd5afb`: R3-001 (WARNING) the partial-index
    condition test's offset missed the new 2-byte vector count; R3-002 (WARNING) no unit test of a non-empty vector index
    section, its rejections or its identities. Both fixed in `9d9add9` (offset `2 + 24 + 4`; round trip, 12 broken cases,
    the 64-index limit, renames, drops and stored identities with vector indexes).
  Reviewed boundary: `c2fbfa0`.
- **L9** — T5 done (`15b67d3`): every insert, delete, range delete and update that changes the vector marks the row in
  each vector index's pending tree (count kept in the catalog entry); the bulk load marks through its inserts. The
  information operation of VI11 came forward (state and pending rows; lists and indicators wait for the generations)
  so the tests can observe the pending rows, and the worker's test hook `Store::pause_vector_work` exists already so
  that the tests keep working once T6 drains the pending rows. No RED was observed first: the marking was written
  before its tests; the suites of indexes, rows, bulk loads, ranges and tables pass unchanged.
- **L10** — T6/T7 decisions taken during the run (defaults, recorded as CLAUDE.md asks):
  - The build's end: the training write fixes the table's greatest key (`end`) in the state record; the backfill reads up
    to it, and pending keys beyond it reach the building generation through the pending rows (AC-VI3 found that a build
    chasing rows appended at the end never published).
  - Automatic rebuilds: each published generation keeps its indicators (`BASELINE`); a rebuild is due when an indicator
    passes its threshold (saturated codes 10 %, largest list 12 × the mean, rows 2 × or ½ × the trained ones with
    another number of lists) and grew 1.5 × over that baseline, so data whose rebuild cannot lower an indicator (one
    found looping on clustered data) does not rebuild forever. Thresholds and growth are test hooks.
  - Resuming after an opening: reading the catalog at the opening broke AC-C9, AC-X19 and AC-Q1 («a new opening
    remembers nothing»), so the worker resumes the first time a transaction opens a table with a vector index, which
    reads no page more.
  - Search defaults before measuring (T9): exact under 20,000 passing rows; 5 % of the lists probed first; 2 × k
    candidates reranked, at least 20. Quantizer margin 2 % of each sampled range. Batch apply budget 8 ms.
  - The worker's tests run the default suite at 12,000 rows; AC-VI3 at 100,000 rows with the 15 ms turn is an
    acceptance test (`#[ignore]`, release).
- **L11** — RDD reviews of `c2fbfa0..6b06454` (reviewed slice by slice with HEAD detached at each slice's last commit):
  - `c2fbfa0..15b67d3` (T5 and the catalog test fix), lens reliability, approved with two suggestions, lineage
    `review-84a601c123f2e5d1`: R3-merge-path-coverage (SUGGESTION) dismissed with evidence: `Reader::patch` takes only
    fixed-width columns, so it never changes a vector; the test's update of `n` alone goes through it and every vector
    update through the merge (comment added to the test); R3-range-delete-second-index (SUGGESTION) fixed: the range
    delete now asserts the second index's pending rows too.
  - `15b67d3..8ee9045` (T6, the worker), lenses risk, resilience and reliability, correction required, lineage
    `review-dd7a80cc74645096`. The native correction covered R3-lost-wakeup-on-stop (CRITICAL: `stop` notified without
    the waiters' mutexes, so a close could hang): fixed in `597fee2` on top of `8ee9045`, the targeted validator approved
    (its first run failed with the provider killed and was rerun once as the bound status reoffered it), acknowledged;
    T7 and T8 were replayed on top (`9986904`, `6b06454`). The warnings, fixed in `308516e`: R4-head-of-line and
    R3-one-index-starves (a failing index stopped the pass: errors are now kept per index), R4-retry-backoff-defeated (the
    pause after a failure waited on the condvar every commit notifies and gave up after 50: it now has its own condvar,
    woken only by the close, and never gives up), R4-generation-cache-unbounded-bytes (bounded at 256 MiB),
    R3-stale-roots-new-snapshot (step, cleanup and the rebuild check now take the roots from their own snapshot),
    R4-training-starves-reconcile (mitigated: pending rows of a ready index go before a training; one thread is VI4's
    decision), R3-missing-coverage (crash tests AC-VI4 in `crash.rs`, failure tests AC-VI14 in
    `vector_fault_tests.rs`). Dismissed: R1-001 and R3-state-format-break (the state record's encoding changed between
    two commits of this branch, before any store of format 16 exists outside it).
- **L12** — AC-VI3 acceptance in release (`308516e`): 100,000 rows, the index created while a thread writes without stop; 52 batches, the longest held the turn 9.14 ms (≤ 15 ms); states training, building, ready; the index in step with the rows.
- **L13** — RDD reviews: `597fee2..6b06454` (T7 search, T8 tests), lenses risk and reliability, approved, lineage
  `review-051f2d94bfafcfca`: R3-sleep-sync (WARNING, tests synchronized by sleeps) fixed in `57c56c7` with the hook
  `Store::wait_vector_held` (also in `crash.rs`). `6b06454..57c56c7` (the worker review's fixes), lenses risk,
  resilience and reliability, approved, lineage `review-40e5fb82a059637f`: R1-001 and R3-backoff-unbounded (WARNING,
  a failing pass retries forever, at most every 1.28 s) dismissed: VI13 retries a failed batch at the next pass and
  reports it to no one, and the previous review asked for no permanent give-up; the close ends it and T12 stops it.
  R3-cache-size-accounting (SUGGESTION) dismissed: the generation holds `f32`s, 4 bytes each; one generation over
  256 MiB (over 16,000 lists at 4,096 dimensions) is beyond the lists VI4 gives below 256 M rows. Reviewed boundary:
  `57c56c7`.
- **L14** — T9, VI15 on Linux (Kubuntu, 16 logical cores, one search thread), Cohere 1 M × 768 L2 from the
  VectorDBBench mirror (`train.parquet` 3,131,995,162 bytes, as the experiment), `krummi-ann-lab motor`
  (`work/ann/motor.md`):
  - Insert one row, durable commit: p95 3.515 ms without the index, 3.263 ms with it and the worker paused (0.928 ×,
    ≤ 1.10 met); with the worker running 6.305 ms (its batches and their durability wait). Deferred commit: 0.031 /
    0.035 ms (1.118 ×, a 4 µs difference).
  - Training and build 188.4 s; 1,287 batches, the longest held the turn 11.65 ms (≤ 15 ms met); 1,000 lists,
    saturated codes 0.20 %, largest list 3.82 × the mean.
  - Without a filter: probes 40 (4 %) recall@10 0.9582, p95 16.40 ms after `3d4ab30` (≥ 0.95 with ≤ 20 ms met); the
    default probes went from 5 % (p95 20.48 ms) to 4 % with this measurement.
  - The threshold of VI6: exact against the index cross between 10,000 and 20,000 rows that pass (9.0 against 10.4 ms,
    18.0 against 15.7 ms), where the index's recall is 0.82–0.86; kept at 20,000.
  - Filters: `cat = 7` (10,000 rows, exact path) 134 ms without a secondary index (the table scan filter-first needs),
    8.3 ms with `by_cat`; `cat < 50` (500,000 rows, approximate, recall@10 0.9425) p95 27.6 ms after `2fad948` and
    `3d4ab30`, which check the filter on the index candidates in distance order instead of building the whole passing
    set (it was 230 ms, and 360 ms through `by_cat`); the results are the same rows (checked by AC-VI6 with every list
    probed and every candidate reranked).
- **L15** — T9, pgvector (`scripts/bench/pgvector_compare.sh`, PostgreSQL 17.12 and pgvector 0.8.7 in Docker on the
  same machine, the client in a `python:3.13-slim` container, one thread, `ivfflat.iterative_scan = relaxed_order`,
  IVFFlat 1,000 lists; `work/ann/pgvector.md`, `work/ann/pgvector-filters.md`): at the same recall@10 (0.958) Krummi's
  p95 is 16.4 ms and pgvector's 42.1 ms (30 probes); at 0.9706 pgvector needs 54.2 ms. Insert p95 3.065 / 3.676 ms
  (1.199 ×). `SELECT 1` p95 0.026 ms. With the filters PostgreSQL scanned exactly (recall 1.0, `cat = 7` 41 ms,
  `cat < 50` 354 ms) also with `enable_seqscan = off`; its plan was not observed. Build 47.5 s (parallel maintenance
  workers by default) against Krummi's 188.4 s on one thread (VI4).
- **L16** — T9, N11: one A/B round of every load, base `7793a51` against `3d4ab30` (`scripts/bench/ab_loads.py`,
  Krummi only, full scale, one load per process): every load within ±3 % except W10, 5.0 % faster on the branch;
  then 10 rounds of W23–W32 and W10.
- **L17** — T9, N11: 10 A/B rounds of W23–W32 and W10 (`work/bench/ab.md`, 66 min): every load within ±1.7 %
  (W10 1.003, W23 1.000, W24 1.009, W25 1.015, W26 0.995, W27 1.017, W28 1.003, W29 1.003, W30 1.008, W31 0.992,
  W32 1.004 in op/s of the branch over the base).
- **L18** — RDD review of `57c56c7..587907e` (worker durable wait, filter-first search in distance order, the
  measurement tools, the guide), lenses risk, resilience, reliability and readability, approved, lineage
  `review-28b9280c3ad6ab2c`. Fixed in `ccc2564`: R1-bench-hardcoded-pw (WARNING; PostgreSQL now listens on 127.0.0.1
  only, its container and throwaway password go at the end of the run), R2-default-probe-share-duplicated (WARNING;
  the tool reads `krummi::table::DEFAULT_PROBE_SHARE`), R2-pending-top-twice (WARNING; the pending rows are read once),
  R2-doc-step-list-stale (SUGGESTION; steps and the threshold comment), R3-durable-after-commit (WARNING; documented: a
  failure of the durable wait is a failed flush, which makes the store refuse writes and ends the thread, T12).
- **L19** — RDD review of `ccc2564` (the last fixes), lenses reliability and readability, approved, no findings, lineage `review-f185432df41a3778`. Every commit of the branch has been through RDD; reviewed boundary `ccc2564`.
- **L20** — Codex R1 first try: the wrapper found no CLI (`Codex CLI not found: codex`): it accepted only `.exe`, `.cmd` and `.bat`; fixed in `6e3d022` (any extension outside Windows). Its Pester tests use a `.cmd` fake and were not run on Linux.
- **L21** — Codex R1 second try: `Codex does not load the project config here (model '<default>', expected 'gpt-6.1-sol'). Trust this folder in Codex (/home/jesuslmp/projects/Krummi) and retry.` Adding the trust entry to `~/.codex/config.toml` was denied by the auto mode classifier; the Codex review waits for the user.
- **L22** — The Stop hook's selectorless review (`caa533e..HEAD`, 56 paths) stopped with `lens_context_budget_exceeded`;
  that range was reviewed slice by slice up to `ccc2564` (L8–L19). RDD review of `ccc2564..7c61c20` (the Codex
  wrapper fix and the closing documents), lenses reliability and readability, approved, lineage
  `review-99d04abda3d18e6a`: R2-isWindows-undefined-5x and R3-001 (WARNING, `$IsWindows` is undefined in Windows
  PowerShell 5.1) dismissed with evidence: the script starts with `#Requires -Version 7`, so 5.1 never runs it; a
  comment now states the intent. R2-report-codex-test-gap (SUGGESTION) stands as written in the report's *Pending*.
- **L23** — 2026-11-25, the user trusted the folder in Codex, verbatim: «hecho». RDD review of `7c61c20..7ce3e50` (the lookup comment), lens readability, approved, no findings, lineage `review-584893515d187337`.
- **L24** — Codex R1 ([report](../../docs/reviews/tables-vector-indexes/2026-11-25-code.md)), wrapper status `findings`: R1–R5 open and two product questions. Fixes: R1 `732cd88`, R3 `7f7a39c`, R2 `52fe704` (long primary keys get a number in the data tree, tags 9–13; RED `a key has 1 to 2048 bytes`), R5 `a21f742`. R4 is being remeasured: with `enable_seqscan = off` PostgreSQL can still read the primary key's index and sort, so the script now also sets `enable_sort = off` and records the plans. The report is in `3d170ff`, and the resolutions are in `2026-11-25-code-resolutions.md`.
- **L25** — RDD review of `7ce3e50..3d170ff` (R1, R3, R2, R5 and the report), lenses risk, resilience and reliability, approved, lineage `review-7f50fd181d458674`. R4-pending-scan-per-step (WARNING) fixed in `08a238f`: the rows with a vector are counted only when an indicator passes its threshold. R3-long-bucket-growth (WARNING) dismissed with evidence: a bucket holds keys that share their first 2,047 bytes, and a key has at most 2,048, so a bucket has at most 256 keys (about 527 KB), far below `MAX_VALUE` (256 MB, `crates/krummi-tree/src/node.rs:33`). Dropping the index or emptying the table frees the tree (`catalog.rs:796-800`). The numbers of removed long keys stay until then, which is documented in the R2 resolution.
- **L26** — RDD review of `3d170ff..08a238f` (the rebuild gate), lenses resilience and reliability, approved, no findings, lineage `review-0b0f5e0aba2d3f55`. Checks at `08a238f`: `cargo fmt` ok, `clippy` clean, `cargo test --workspace` 1,332 passed, 2 failed, 36 ignored. The failures are the known trybuild `trees_compile` and `krummi-tx` `group_commit::ac_e13_failures_of_the_journals_writer`, which is intermittent: it failed 1 of 20 runs here and 1 of 40 on the base copy `work/bench/base-src` (equal to `7793a51`), so it was not introduced by this branch. The first R4 pgvector run (report `work/ann/pgvector-filters-r4.md`, about 20:20–21:05 by its log and report times; the phase log's `bg-start`/`bg-end` failed on a wrong `--label` argument) measured 350 ms with recall 1.0 for `cat < 50` in both modes, although `EXPLAIN` of the same search with the vector inline used IVFFlat (19.6 ms). psycopg prepares a statement after five runs, and a prepared statement can go to a generic plan. The script now plans each search with its own vector (`prepare_threshold=None`). Its `cat = 7`, 50 probes row in the default mode (p95 76.99 ms) overlapped a local `cargo test`. The run was repeated as `pgvector-filters-r4b.md`.
- **L27** — R4 fixed in `abc006f`: the pgvector rerun without prepared statements (`work/ann/pgvector-filters-r4b.md`) uses IVFFlat with the filter, and its plans are recorded. The closing report compares points with recall@10 ≥ 0.95. The resolutions of Codex R1 are in [2026-11-25-code-resolutions.md](../../docs/reviews/tables-vector-indexes/2026-11-25-code-resolutions.md).
- **L28** — Codex R2 ([report](../../docs/reviews/tables-vector-indexes/2026-11-25-code-02.md)), follow-up of `daaacd1..5eaa9ba`, wrapper status `approved`: R1–R5 verified resolved, no open findings. Two product questions remain for the user.
