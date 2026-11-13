# Docs layout — execution document

Objective: keep `odd/tasks` and `docs/prd/krummi` small as the project grows, by archiving finished executions by
module and grouping the PRDs by the module of their tree. Branch `feat/docs-layout` from `main` at `80027e1`. Tooling
and documentation only, no PRD (`CLAUDE.md`). Tests: `python -m unittest` for the new script; `cargo test --workspace`
because code comments change.

## Specs

- **S1** — Finished executions leave `odd/tasks`: their files (document, report, phase log, measurements) move
  together to `odd/done/<module>/`, with the mapping of L3. `odd/tasks` keeps only the executions in progress.
- **S2** — The PRDs move to `docs/prd/krummi/<module>/`, following their tree (L3); `krummi.prd.md` and its
  translation stay at `docs/prd/krummi/`.
- **S3** — Every reference to a moved file is rewritten: relative Markdown links (with their anchors) and paths from
  the repository root in any text file, the Codex reports included (L1). No link that worked before the move is broken
  after it.
- **S4** — The tools and instructions follow the new layout: the closing report skill archives the execution when it
  closes it, the PRD skill places new PRDs in their module's folder, and `CLAUDE.md`, `AGENTS.md`, the review skills and
  the Codex wrapper's examples name the new paths.
- **S5** — One script with tests does the moves and the rewriting, for this migration and for every later archiving.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S3, S5 | inline | done (L4, L5, L6) | `405e155`, `0956809`, `d71f11e` |
| T2 | S1, S2, S3 | inline | done (L5) | `2503b38` |
| T3 | S4 | inline | done (L6) | (this commit) |
| T4 | checks, reviews, closing report, archiving of this execution | inline | pending | |

## Log

- **L1** (2026-11-13, the user's requests, verbatim): «Quisiera organizar docs y odd/tasks según va creciendo el
  proyecto termina habiendo muchos archivos en cada carpeta y con el tiempo va a ser peor. Se admiten sugerencias.» To
  the proposal (1: `odd/tasks` for what is active and the finished executions archived by module in
  `odd/done/<módulo>/`; 2: the PRDs in subfolders that follow their tree; `docs/reviews` and `docs/benchmark` left as
  they are) and «¿Lo hago así, con las propuestas 1 y 2 y reescribiendo también los enlaces dentro de los informes de
  Codex?», «Sí». To «¿Incluyo también los PRD de tablas en la reorganización, o los dejo donde están hasta que esa
  sesión termine?», «Inclúyelos». To the estimate (1–2 h) and «¿Arranco?», «Sí».
- **L2** (2026-11-13): run started at 10:07:24 (`date`), estimate 1–2 h. Starting state: `odd/tasks` 114 files (36
  executions of three files and six single ones); `docs/prd/krummi` 48 files (24 PRDs and their translations). About
  120 tracked files mention `odd/tasks` and 170 `docs/prd/krummi`, among them code comments, `CLAUDE.md`, `AGENTS.md`,
  three skills, the Codex wrapper and the Codex reports.
- **L3** (2026-11-13): mapping, shown to the user before the start (the PRD each execution cites; without one, the
  module it changes). Executions: `pager/` pager, pager-mapping, integrity-once, version-checks, write-path, margin,
  cheap-gaps, fast-xxh3, scratch-lock, fork-safe-lock, max-size-growth; `journal/` journal, journal-followups,
  journal-group-commit, page-diffs, transfer-background-thread, transfer-pacing, commit-copies, write-reuse;
  `transactions/` transactions, turn-handoff, turn-timeout-wake; `trees/` every trees-* (their `-measurements`
  included), trees-append-split, range-step; `tables/` tables-schema, table-row-reads; `benchmark/` benchmark,
  benchmark-krummi-voron; `free-space/` free-space; `tooling/` codex-review, github-actions-ci, rename-to-krummi.
  PRDs (by the parent chain): `pager/` pager, pager-integrity-once, pager-writer-checks, pager-commit-checks,
  pager-version-checks; `journal/` journal, journal-group-commit, journal-page-diffs; `transactions/` transactions;
  `trees/` trees and its six children; `tables/` tables, tables-schema, tables-row-reads; `benchmark/` benchmark,
  benchmark-krummi-voron; `free-space/` free-space; `tooling/` rename-to-krummi.
- **L4** (2026-11-13): T1. `scripts/docs/move_docs.py` with `archive <feature> <module>`, `move <manifest>` and
  `check`: relative Markdown links (inline and reference definitions) are rewritten when their target or their own file
  moves, a link whose target did not exist is left alone, and paths from the root are rewritten in any text file only
  as whole paths (a sentence's full stop ends one). Moves go through `git mv`. Six tests in
  `scripts/docs/test_move_docs.py` on temporary git repositories, all passing; written after the script, so checked
  with mutations instead of RED: a full stop as a path character fails the whole-path test, and not recomputing the
  links of a moved file fails two. Baseline of `check` before the move: 37 broken links, all older than this run
  (copies of chicago-crime's methodology, paths of `muninndb`; list in `work/docs-layout-broken-before.txt`).
- **L5** (2026-11-13): T2. Manifest of 160 moves from L3 (`work/docs-layout-manifest.tsv`): every file of
  `odd/tasks` but this run's, and every PRD but `krummi.prd.md` and its translation. `move_docs.py move`: «moved 160
  files, rewrote references in 258 files». `check` after the move: the same 37 broken links as before, each with the
  same target and its file at its new path (compared by script). Old paths left: an example in the closing report
  skill (T3), the chicago-crime references (already broken), test fixtures, and one relative link in a Rust doc comment
  (`crates/krummi-bench/src/lib.rs:1`), which the script did not rewrite because it rewrote links of `.md` files only:
  fixed by hand, and the script now also rewrites the links of `.rs` files (commit before this one; new test
  `test_links_in_rust_doc_comments_follow_their_target`, which fails when `.rs` is left out; 7 tests pass).
  `cargo fmt --all --check` clean.
- **L6** (2026-11-13): T2 committed (`2503b38`, with `tools/guid-vectors/Program.cs`, a rewritten comment first left
  out of the commit). T3: `move_docs.py reopen <feature>` moves an archived execution back to `odd/tasks/` (new test
  `test_reopening_brings_an_archived_execution_back`; 8 tests pass). The closing report skill says where finished
  executions live and archives the execution as the run's last commit (step 6), with `odd/done/trees/range-step-report.md`
  as its example; the PRD skill places a module's PRDs in `docs/prd/<domain>/<module>/`; `CLAUDE.md` (PRD folder,
  calibration from `odd/done/*/*-report.md` instead of chicago-crime's, archiving at closing), `AGENTS.md` (the
  artifacts table), `README.md` (layout table), the `review-code` and `review-prd` skills and the `codex-review` skill
  name the new paths. `review-code` also gave a vibe coding report path in `odd/tasks/`, which the closing report skill
  puts in `docs/reports/`: aligned with it.
