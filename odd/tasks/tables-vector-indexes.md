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
| T1 | S2 (PRD annotation of the Linux-only decision, ES and EN) and the worktree hook made POSIX for Linux | inline | pending | |
| T2 | S1: VI2, VI8 (distance kernels), AC-VI12, AC-VI10 golden bits | inline | pending | |
| T3 | S1: VI7, VI16 (filters, public exact search, errors), AC-VI12, AC-VI13 (nulls) | inline | pending | |
| T4 | S1: VI1, VI10, VI14 (definitions, catalog, Describe, schema changes, format 16), AC-VI1, AC-VI9, AC-VI14 (format) | inline | pending | |
| T5 | S1: VI3 foreground (pending log in every row write, bulk load), AC-VI2 (pending part) | inline | pending | |
| T6 | S1: VI3, VI4, VI12, VI13 (worker, training, build, reconcile, close and open, failures), AC-VI3, AC-VI4, AC-VI14, AC-VI15, AC-VI17 | inline | pending | |
| T7 | S1: VI5, VI6 (approximate filter-first search), AC-VI2, AC-VI5, AC-VI6, AC-VI7, AC-VI13 | inline | pending | |
| T8 | S1: VI9, VI11 (information, generations, automatic and requested rebuilds), AC-VI8, AC-VI16, AC-VI17 | inline | pending | |
| T9 | S3 (measurements: threshold, defaults, VI15 on Linux, pgvector, bench rounds), AC-VI11 | inline | pending | |
| T10 | S1 (guide and reference docs) | inline | pending | |
| T11 | S5 (RDD of the last slice, Codex review, closing report, archive) | inline | pending | |

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
