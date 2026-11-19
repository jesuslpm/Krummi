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
| T1 | S1 (PRD amendment before the run) | inline | pending | |
| T2 | S1: ST4, ST13 (counter, format 13), AC-ST3, AC-ST11 | inline | pending | |
| T3 | S1: ST1, ST7, ST8, ST16 (build, store, analyze, read), AC-ST1, AC-ST6, AC-ST12 | inline | pending | |
| T4 | S1: ST2, ST3, ST5, ST12 (estimate, sampling, memory), AC-ST2, AC-ST9 | inline | pending | |
| T5 | S1: ST6, ST9, ST10, ST11, ST14 (thread, failures, close), AC-ST4, AC-ST5, AC-ST7, AC-ST8, AC-ST10 | inline | pending | |
| T6 | S1: ST17, ST18 (index prefixes, multi-column), AC-ST13, AC-ST14 | inline | pending | |
| T7 | S2, S4 (measurements, reviews, closing report) | inline | pending | |

## Log

- **L1** — 2026-11-19, user's request, verbatim: «Implementa 7.3 con ODD». Confirmation of the start: «Sí, Sin
  preguntar. Si surge alguna pregunta toma la decisión más conservadora o recomendada. Puede que nos quedemos sin
  tokens».
- **L2** — Before the run, two product questions from the code map, answered by the user: rows stored in a run are
  sampled by reading only the needed pages without the run checksum («La a) sí»); in a write, the estimate samples the
  snapshot a new read sees («a) ya que las estadísticas son estadísticas y no tienen que ser exactas ni estar siempre
  al día. Es el principio fundamental.»). Recorded in ST1 and ST3 and in G6 of the root.
