# ANN experiment of design document 12 — ODD execution document

Objective: measure, before PRD 7.6 is written, the recall and latency of the in-memory approximate SQ8 scan, IVF-Flat
and IVF-SQ8 against Krummi's exact search, and judge them against the thresholds of design document 12.
Branch: `feat/tables-vector-indexes`. Delivery: `ask-on-risk`; local commits only, no push. Test runner: `cargo test`.
Estimate: 3 h 30 min – 6 h. Real start: 2026-11-24 06:21:06 +0000.

## Specs

- **S1** — The experiment comes before the PRD (user's answer: "Experimento antes (Recomendado)"): it runs in the
  benchmark tooling with no change to the product, and PRD 7.6 is written afterwards with the measured algorithm.
  Design document 12, *Plan*: "**First:** the benchmark experiment (*Benchmarks*, steps 1 and 2), with no change to the
  product."
- **S2** — Location (user's answer: "Crate de experimento (Recomendado)"): a new workspace crate, not published
  (`publish = false`), only for the experiment and its report. It touches neither the engine nor the comparative
  benchmark, its report, thresholds or CLI (module 6). The kernels move into the engine in 7.6.
- **S3** — Data (user's answer: "SIFT, GloVe y Cohere 768 (Recomendado)"): SIFT1M (128, L2, fvecs), GloVe-100
  (angular, 1.2M, text) and 1M vectors of 768 dimensions from Cohere Wikipedia (parquet, not normalized, for L2 and the
  dot product). Downloaded once from their public sources to a folder ignored by git, never committed.
- **S4** — Platforms (user's answer: "Windows y Linux (Recomendado)"): both, and the doc 5 kernels must give
  bit-identical results on both.
- **S5** — Experiments, design document 12, *Benchmarks*, order of the experiments, steps 1 and 2:
  "1. The approximate SQ8 scan with rerank, computed in memory in the benchmark tool: recall lost by quantization
  alone." "2. IVF-Flat and IVF-SQ8: recall lost by partitioning, and by both." Ground truth: "Krummi's own exact scan".
  Scales "of 10,000, 100,000 and 1 M".
- **S6** — Measures, design document 12, *Search*: "recall@10 and @100 | Against the exact ground truth"; "QPS–recall
  curve | Sweeping nprobe"; "Latency | p50, p95 and p99, cold and warm cache"; "Memory | RSS plus the working set in
  the page cache"; "Index size".
- **S7** — Thresholds, design document 12, at 1 M × 768: "recall@10 ≥ 0.95 with a p95 ≤ 20 ms, one thread, warm cache,
  backlog drained". *When the recommendation would change*: "IVF-SQ8 needs more than ~10 % of the lists to reach 0.95
  recall, or cannot get under a 20 ms p95 at 1 M | HNSW (the fallback)"; "The dot product on unnormalized vectors
  dominates and IVF gives poor recall | A graph".
- **S8** — Exact kernels as design document 5 decides them: `l2_squared` Σ(aᵢ-bᵢ)², `dot` -Σaᵢbᵢ, `cosine`
  1 - Σaᵢbᵢ/(‖a‖‖b‖); every element converted to `f64` and accumulated in 16 lanes, lane j taking j, j+16, …, lanes added
  in the fixed tree (8, 4, 2, 1), then the tail in order; no `fma`; ties by id; a golden test pins the `u64` bits.

## Defaults taken

- Crate `crates/krummi-ann-lab`, binary `krummi-ann-lab`; CLI messages and the report in Spanish (repository rule).
- Elements `f32` only: the three data sets are `f32`; `f16`, `i8` and `bit` are left to 7.6.
- Metrics: SIFT `l2_squared`; GloVe `cosine`; Cohere `l2_squared` and `dot`.
- Queries: SIFT's own 10,000 queries and its ground truth file, used to cross-check the exact search; for GloVe and
  Cohere, vectors held out of the base with a fixed seed. Latency over 1,000 queries per configuration.
- IVF parameters from document 12: nlist by pgvector's rule (rows/1000 up to 1 M rows), k-means with a fixed seed and
  10 iterations, a sample of up to 256 points per list; SQ8 ranges per dimension from the sample; rerank of `k·r`
  candidates.
- The 10 M scale is out: it is for the stored index (data not in RAM), not for this in-memory experiment. "Cold cache"
  does not apply in memory; only warm latency is measured.
- External HNSW (step 3) only if IVF-SQ8 misses S7, to check the fallback.
- Parquet read with the pure-Rust `parquet` crate inside the lab crate (no Python packages are installed: neither
  `pyarrow` nor `numpy`), so the machine's Python stays untouched.
- Data and caches under `work/ann/` (ignored by git).
- Engram mirror pending: the first `mem_save` of the session could not run (auto mode could not review a call that a
  hook changes).

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S2, S8 | inline | done | `14f7346` |
| T2 | S3 | inline | done | `b703169` |
| T3 | S5 | inline | done | `b703169`, then the blocked walk in the T4–T5 commit |
| T4 | S5, S6 | inline | done | one commit with T5 (they share the measurement) |
| T5 | S5, S6, S7 | inline | done | one commit with T4 |
| T6 | S4, S6, S7 | inline | done | `ddefad1` |
| T7 | S7 | inline, only if T6 misses S7 | done | `d07655c` (code), `ddefad1` (results) |

- T1: the crate, the exact kernels with their golden test, and the exact top-k with a bounded heap.
- T2: download and convert the data sets to one raw cache format; held-out queries.
- T3: the exact ground truth (top-100), checked against SIFT's ground truth file.
- T4: the SQ8 quantizer, the approximate SQ8 scan and its rerank (step 1).
- T5: k-means, IVF-Flat and IVF-SQ8 with the nprobe sweep; recall, latency, memory and size (step 2).
- T6: the runs on Windows and Linux, the determinism check, the report in `docs/benchmark/` and the verdict, annotated
  in design document 12.
- T7: an external in-memory HNSW to check the fallback.

Route evidence: the units depend on each other (each uses the previous one), so no writer trigger holds; all inline.

## Log

- **L1** (2026-11-24, user, verbatim): "Empecemos con el PRD del punto 7.6, Índices vectoriales aproximados, en una
  nueva rama". Then, to the questions: "Experimento antes (Recomendado)", "Crate de experimento (Recomendado)",
  "SIFT, GloVe y Cohere 768 (Recomendado)", "Windows y Linux (Recomendado)"; to the estimate: "Sí, empieza".
- **L2** (2026-11-24): starting state verified by the explorer: no distance kernel, exact top-k or threshold search
  exists in the engine (only `vector_norm`, `crates/krummi/src/table/value.rs:562`); no vector data set or vector
  workload in `krummi-bench`; nothing of the experiment exists. Machine: i9-13980HX, 24 cores, 64 GB; WSL 31 GB.
- **L3** (2026-11-24, T1): golden bits pinned on win-x64 (`kernels::tests::GOLDEN`); linux-x64 checks them in T6.
- **L4** (2026-11-24, T2, default taken): the Cohere Wikipedia 22-12 copies on Hugging Face no longer answer
  (`401` on `Cohere/wikipedia-22-12-en-embeddings`); the same vectors come from VectorDBBench's public mirror
  `https://assets.zilliz.com/benchmark/cohere_medium_1m/` (`train.parquet` 3,131,995,162 bytes, `test.parquet` 1,000
  queries, `neighbors.parquet` the top-1000 by cosine). Not normalized (norms about 13.5–14.4), as S3 assumed. GloVe:
  `downloads.cs.stanford.edu` (the `nlp.stanford.edu` URL did not answer), 1,193,514 rows, no zero row; 1,000 held out
  with seed 12. SIFT: `ftp.irisa.fr`, its 10,000 queries cut to the first 1,000 with their ground truth. Prepared in
  1.8 s (SIFT), 14.8 s (GloVe) and 181.8 s (Cohere, the row API of `parquet`).
- **L5** (2026-11-24, T3 evidence): exact ground truth of SIFT at 1 M against the source's: recall@10 0.9994,
  recall@100 0.9999, and 0 queries whose 10 first distances differ, so every difference is a tie. One query on one
  thread: 0.118 s (about 1.1 G multiply-adds per second, document 5's estimate). The parallel ground truth walked the
  base once per query and took 63.1 s; walking it once per block of queries takes 21.3 s, with identical bytes.
- **L6** (2026-11-24, T3): ground truth computed for the three sets at 10,000, 100,000 and all rows. Cohere by cosine
  against the source's: recall@10 1.0000, recall@100 1.0000, 0 differing queries. GloVe has no source ground truth.
- **L7** (2026-11-24, T4–T5): first trial (SIFT 10,000) showed IVF-SQ8 slower than IVF-Flat: the SQ8 pass indexed its
  arrays and did not vectorize; walking code, query and steps in blocks of 16 made it about 4 times faster with the
  same results (digest `9d63d3c2eba2f703` before and after). Unpinned, the timed thread may land on an E core: the
  exact search of SIFT 10,000 takes p50 0.82 ms with affinity `0xFFFF` (logical 0–15, the P cores) and 1.55 ms with
  `0xFFFF0000` (the E cores). Default taken: the Windows runs use `start /affinity 0xFFFF` under the bench power plan;
  in WSL the hypervisor schedules the virtual CPUs, so the Linux runs are not pinned (stated in the report).
- **L8** (2026-11-24, T5 default): IVF assigns and probes by L2 in the space of the codes (vectors as given for l2 and
  dot, normalized for cosine), as document 12 assumes ("IVF partitions by L2"); the dot product on unnormalized
  vectors is thus measured as the risk it names.
- **L9** (2026-11-24, T6 evidence): the Windows run (06:52–08:20 UTC, bench power plan with boost off, affinity
  `0xFFFF`) wrote the 12 sections in `work/ann/out/windows.md`. At 1 M, IVF-SQ8 with r = 2 already has IVF-Flat's
  recall in every row (quantization loses nothing after the rerank). recall@10 ≥ 0.95 is reached by Cohere l2 between
  nprobe 32 (0.9415, p95 14.89 ms) and 64 (0.9773, p95 27.99 ms); by Cohere dot between 64 (0.9483, 6.4 %, p95
  19.65 ms) and 128 (0.9750, 12.8 %, p95 35.45 ms); GloVe stays at 0.9457 at 128 (11.7 % of the lists). The first
  Linux run failed (`cargo: not found`: a non-interactive `sh` in WSL does not read `~/.cargo/env`); the script now
  sources it and was run again. An 11 s `cargo clippy` overlapped the start of the Linux run.
- **L10** (2026-11-24, defaults taken): GloVe misses S7's first switch criterion (more than ~10 % of the lists), so
  T7 runs (an external HNSW, `hnsw_rs` 0.3.5, M 16, ef_construction 200, document 5's kernels as its distance, the
  candidates reranked exactly; Windows only, because its parallel build is not deterministic). The 0.95 crossings are
  measured with a finer sweep (`--sondeos`, `--metrica`): l2 40, 48, 56; dot 72, 80, 96, 112; GloVe 160, 192, 256. The
  bench plan turns boost off, so the P cores run at their base clock; SIFT 10,000 and Cohere 10,000 l2 are measured
  again with `-Boost` to report the clock's share of the 20 ms threshold.
- **L11** (2026-11-24, T6–T7 evidence): S4 holds. `cargo test` passes on Linux, golden bits included, and the 12
  result digests are identical on Windows and Linux. The fine sweep and the HNSW ran on Windows from 19:34 to 21:54
  UTC. IVF-SQ8 r = 2 reaches recall@10 ≥ 0.95 as follows:
  - Cohere L2: nprobe 40 (4.0 %), p95 17.20 ms; meets S7.
  - Cohere dot: nprobe 72 (7.2 %), p95 20.90 ms; misses the p95 by 0.9 ms at base clock.
  - GloVe: nprobe 160 (14.7 %), p95 6.18 ms; misses the share of the lists.

  The external HNSW reaches recall@10 ≥ 0.95 at:
  - L2: ef 320, p95 11.19 ms;
  - dot: ef 80, p95 5.89 ms;
  - GloVe: never; 0.9175 at ef 640.

  Builds in parallel: HNSW 1169–1791 s, IVF 579–773 s. With boost, full passes at 10,000 rows run 1.7–1.8 times
  faster. RSS was not measured (S6 gap, stated in the report); index sizes were.
- **L12** (2026-11-24, T6): report `docs/benchmark/2026-11-24-ann-experiment.md`, with the raw outputs beside it.
  Design document 12 was annotated with *Measured result*. The algorithm choice for PRD 7.6 is left to the user:
  - IVF-SQ8 meets the reference case;
  - the two switch criteria that fire do so narrowly;
  - HNSW fixes only the dot-product case.
- **L13** (2026-11-24, closing checks): `cargo test --workspace` failed in `ac_r5_the_workspace_has_only_the_krummi_packages`.
  The test lists the workspace packages, and `krummi-ann-lab` had been missing from it since T1; the earlier runs had
  tested only the lab crate. Fix: the package test lists the lab crate. `bench_dependencies` now also forbids
  `krummi-ann-lab`, `hnsw_rs` and `parquet` in the library, which pins S2. After the fix, `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass (1275 tests, 0 failed).
- **L14** (2026-11-24, RDD): the whole range `dfba65f..32ad2c6` exceeded the reviewers' context budget
  (`lens_context_budget_exceeded`, no authority created). It was reviewed as chained committed slices, each from a
  detached checkout of its last commit, and the branch was checked out again afterwards. Item 5 was named on the two
  slices that add dependencies. Consent was granted (project rule).

  | Slice | Lenses | Result | Findings and disposition | Lineage |
  |---|---|---|---|---|
  | `dfba65f..b703169` (T1–T3, parquet) | risk, reliability | approved | R3 warnings: stale truth cache, fixed (`check_truth`); untested tie check, fixed (test); unreadable empty ivecs, fixed by rejecting `--filas 0`. R3 suggestion: NaN recall, unreachable after that guard | `review-101b877689b09e1e` |
  | `b703169..8984bc9` (T4–T5) | reliability, readability | approved | R3 warning: the recall slice panics under 1600 rows, fixed (clamped). R2 warning: the k-means literal, fixed (`KMEANS_ITERATIONS`). R2 suggestion: the `push_codes` doc, fixed. R2 warning: the timed scan repeats `sq8_scan`, kept, so the measured code stays as measured. R3 suggestion: `measure` untested, partly covered by the new tests; the digests pin the rest across platforms | `review-257b7856991921e1` |
  | `8984bc9..d07655c` (T7 code, hnsw_rs) | risk, reliability | approved after one correction | R1-001 CRITICAL (`--sondeos` dropped silently above the lists): corrected in `9d0cf2d` (an error, with a test), validated, brought to the branch as `f36bc95`. Warnings R1-002 and R3 (options ignored by other commands), R3 (`--metrica` stops a run over every set), R3 (parse untested): fixed | `review-8333364a434ecddf` |
  | `d07655c..32ad2c6` (report, tests, log) | reliability, readability | approved | R2-001 and R3 warnings (HNSW recall@100 is flat for ef ≤ 80, because the effective ef is max(ef, k)): fixed with a note in the report. R2-002 suggestion (a phase-log label): kept, because the log is append-only; L9 states what ran when | `review-0ec2b05544894344` |

  The advisory fixes went into one commit after the reviews. Checks: the SIFT 10,000 and GloVe 10,000 digests are
  unchanged (`9d63d3c2eba2f703`, `13245170461c22a6`).
- **L15** (2026-11-24, RDD last slice): `32ad2c6..1fdce49` (the advisory fixes and `f36bc95`), assessed medium
  `under_budget`, reviewed under the closing rule with lenses reliability and readability. Approved, lineage
  `review-cc6339465020a82c`. Kept as follow-ups, all informational:
  - R2-001 (the truth error names both counts);
  - R2-002 (the wording "requested row" in a doc comment);
  - R2-003 (an empty selection is success);
  - R3-001 (`--sondeos` is checked per set, so a run over sets of different sizes can fail late);
  - R3-002 (no test of the clamped slice);
  - R3-003 (an option error is reported before an unknown command).
- **L16** (2026-11-24, Codex R1): report `docs/reviews/ann-experiment/2026-11-24-code.md`, status `findings`, with R1
  (blocker), R2 (major) and R3 (minor). Resolutions are in `2026-11-24-code-resolutions.md`.
  - R1 fixed in `56c24ce`.
  - R3 fixed in `f5b4294`.
  - R2 fixed in `96ee454`, plus the report and the RSS run.

  Default taken for R2: the RSS is measured on Linux only, because the workspace forbids unsafe code and std has no
  working-set API on Windows. It is measured at 100,000 rows, each figure the first section of its process.
  - The first RSS run's log file was written at 23:07:25 UTC; its start was not logged.
  - The dot-product run went from 23:08:10 to 23:15:49 UTC (phase log).

  The `fix` phase mark was written late (23:07:38): `mark fixes` was rejected, because the phase is `fix`. Codex
  ended at 22:43:19 (`bg-end`), and the fix commits are at 22:52:29.

