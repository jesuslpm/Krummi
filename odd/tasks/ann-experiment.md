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
| T2 | S3 | inline | done | (this commit) |
| T3 | S5 | inline | in progress | |
| T4 | S5, S6 | inline | pending | |
| T5 | S5, S6, S7 | inline | pending | |
| T6 | S4, S6, S7 | inline | pending | |
| T7 | S7 | inline, only if T6 misses S7 | pending | |

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
