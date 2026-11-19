# Statistics of the tables — ODD execution document

Objective: implement the statistics of `docs/prd/krummi/tables/tables-statistics.prd.md` (7.3, approved by the Codex
PRD review on 2026-11-19, amended before the run: ST1 partial run reads, ST3 snapshot in a write). Branch:
`feat/tables-statistics` (from `main` at `ba7f27a`). Delivery: `ask-on-risk` (single branch, left unmerged). Test
runner: `cargo test --workspace` (Windows; Linux in WSL 2 Ubuntu). Estimate: 14 h – 22 h. Real start: 2026-11-19
12:07:36 +0000.

## Specs

- **S1** — The statistics PRD, `docs/prd/krummi/tables/tables-statistics.prd.md`, and its ancestry up to the root:
  decisions ST1–ST18, *Validaciones y mensajes*, *Salida*, *Fuera de alcance* and *Dejado a la implementación*,
  verbatim there. Its acceptance criteria AC-ST1–AC-ST14 are the tests of the library.
- **S2** — ST15 and AC-ST15: single-load measurements of W1–W32 before and after the change, the first estimate on
  the W23 table, and W23 and W28 with a second thread that estimates, on Windows and Linux.
- **S3** — The technical starting state: `docs/design/tables/03-keys-and-indexes.md` (*Statistics*, with its
  2026-11-19 annotation) and `04-query-tree-and-planner.md`.
- **S4** — The project's checks (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`), RDD on each work-unit commit, the Codex code review over the whole range, and the closing
  report.
- **S5** — The user's instruction for open questions during the run (L1): «Si surge alguna pregunta toma la decisión
  más conservadora o recomendada.»

## Execution design

- **Counter and storage.** The table's catalog extra bytes gain, after the indexes, the modification counter (u64),
  the root of a nameless stats tree and the counter of the last emptying (format 13). Every row path adds to the
  counter in the write's table state, so it commits and aborts with the rows.
- **Stats tree.** Key: a kind byte and the column identity, or the index identity and the prefix length; value: the
  encoded statistics with their origin row count, rows read and origin counter.
- **Memory.** A store-level registry, like the projections', keyed by table identity, holds the newest built
  statistics per column and prefix, and the emptying marks.
- **Sampling.** Tables up to 30,000 rows are read whole; larger ones by random descents to distinct leaves, read
  whole. Rows in runs are read by pages without the run checksum (ST1). Order uses the key encoding of the column, so
  histogram bounds compare as bytes (N5).
- **Background thread.** One per store, started on the first request, joined at close; the store's shared state moves
  behind an `Arc` so the thread can open reads and writes.
- **Routes.** The parent works inline (budget: the user warned that tokens may run out), one task per commit, the
  feature document and its Engram mirror kept current so a later session can resume.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1 (PRD amendment before the run) | inline | done | `b04cba7` |
| T2 | S1: ST4, ST13 (counter, format 13), AC-ST3, AC-ST11 | inline | done | `8d1fb6e` |
| T3 | S1: ST1, ST7, ST8, ST16 (build, store, analyze, read), AC-ST1, AC-ST6, AC-ST12 | inline | done | `7fa8911` |
| T4 | S1: ST2, ST3, ST5, ST12 (estimate, sampling, memory), AC-ST2, AC-ST9 | inline | done | `93bac6d` |
| T5 | S1: ST6, ST9, ST10, ST11, ST14 (thread, failures, close), AC-ST4, AC-ST5, AC-ST7, AC-ST8, AC-ST10 | inline | done | `0ddf2a1` |
| T6 | S1: ST17, ST18 (index prefixes, multi-column), AC-ST13, AC-ST14 | inline | done | `34cbe58` |
| T7a | S1: ST1 (random-leaf sampling over 30,000 rows), part of AC-ST1 and AC-ST12 | inline | done | `ea5d7b3` |
| T7b | S1: ST1 (partial read of rows stored in runs, without the run checksum) | inline | pending | |
| T7 | S2, S4 (measurements, reviews, closing report) | inline | pending | |

## Log

- **L1** — 2026-11-19, user's request, verbatim: «Implementa 7.3 con ODD». Confirmation of the start: «Sí, Sin
  preguntar. Si surge alguna pregunta toma la decisión más conservadora o recomendada. Puede que nos quedemos sin
  tokens».
- **L2** — Before the run, two product questions from the code map, answered by the user: rows stored in a run are
  sampled by reading only the needed pages without the run checksum («La a) sí»); in a write, the estimate samples the
  snapshot a new read sees («a) ya que las estadísticas son estadísticas y no tienen que ser exactas ni estar siempre
  al día. Es el principio fundamental.»). Recorded in ST1 and ST3 and in G6 of the root.
- **L3** — T2 done in `8d1fb6e`: the catalog's statistics part (counter, stats root, emptied mark), the counter on
  every row path (`apply_rows`, the append fast path, the same-length overwrite, emptying) and format 13. Checks:
  fmt, clippy and `cargo test --workspace` pass. AC-ST3 is checked here through a test hook on the counter; its
  staleness part comes with T4. Risk: item 1 (persisted format). RDD: assessed `medium`/`under_budget`, escalated with
  item 1; review `review-73b4e0b9c2555c7d` over `b04cba7..8d1fb6e`, lens risk, approved with no findings,
  acknowledged. Reviewed boundary: `8d1fb6e`.
- **L4** — Decision (S5, recommended): T3 first samples through the row scan, which reads a run whole; the partial
  read of runs without their checksum (ST1) and the random leaf sampling of tables over 30,000 rows follow as their own
  steps, so each is reviewable alone.
- **L5** — T3 done in `7fa8911` (build, save and read column statistics; analyze in a write and on the store; ST8
  lifecycle), T4 in `93bac6d` (estimate with the shared in-memory statistics, sampling at once, a write's private
  statistics) and T5 in `0ddf2a1` (background thread, the store's shared state behind an `Arc`, save retry, hooks of
  ST14). Checks pass after each. Decisions (S5): a dropped column's statistics are ignored by identity rather than
  removed from the tree (observably the same); memory entries are ignored after an emptying the transaction sees; a
  renewal finds a renamed table among the catalog's tables; a save waits for the turn in 20 ms polls so that close
  can drop it; unit tests that alter the engine rebuild the shared state (`with_engine_mut`). Not yet covered: the
  partial read of runs and the random-leaf sampling over 30,000 rows (L4), the fault cases of AC-ST8 and AC-ST15's
  measurements. Risk: item 4 (concurrency) for T5.
- **L6** — RDD of the slice `8d1fb6e..0ddf2a1` (T3–T5), escalated with item 4 (concurrency): review
  `review-2013388a3a92733c`, lenses risk, resilience and reliability, approved with no findings, acknowledged.
  Reviewed boundary: `0ddf2a1`. The independent verifier that item 4 calls for runs once at the feature's end.
- **L7** — T6 done in `34cbe58` (index prefix statistics, equality in several columns with the biased backoff,
  index statistics readable) and T7a in `ea5d7b3` (`krummi_tree::sample_leaves`: random descents to distinct
  leaves, about 30,000 rows, seed fixable by a test hook). Checks pass after each. Decisions (S5): a write's private
  statistics keep columns, not index prefixes, so a multi-column estimate in a write on a table it created samples
  again each time; the committed snapshot serves a write's estimate only when it has every column and index of the
  write's schema. Pending: T7b, the partial read of runs (ST1's G6 exception), which needs a run read by pages in
  krummi-tx and a row decoder on partial bytes; today a sampled row stored in a run is read whole.
