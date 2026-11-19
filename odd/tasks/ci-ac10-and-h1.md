# CI: AC10 sweep timeout and flaky AC-H1 test — ODD execution document

Objective: find and fix the two CI failures of `main` since 2026-11-15 (no product change, no PRD). Branch:
`fix/ci-ac10-and-h1` (from `main` at `e5b5f0b`). Test runner: `cargo test --workspace`. Estimate: 2 h – 4 h. Real
start: 2026-11-19 06:38:20 +0000.

## Specs

- **S1** — The CI step «Every-byte damage sweep (AC10)» (`.github/workflows/ci.yml:79-84`, 30-minute limit) passed on
  `6b49bea` (2026-11-16: 19 min on Ubuntu, 13 min on Windows) and timed out after 30 minutes on `0cb816c` (Windows,
  2026-11-17) and `d7fe899` (Ubuntu, 2026-11-18). Find the cause; if a commit made the engine or the sweeps slower, fix
  the regression; raise the limit only if the extra work is legitimate and measured.
- **S2** — `writer_check_tests::ac_h1_a_write_never_checks_the_nodes_it_changed` fails intermittently on Windows CI
  (`crates/krummi/src/writer_check_tests.rs:145`, «the committed nodes are checked when read») on 2026-11-15
  (`43698475785`) and 2026-11-18 (`44357038048`). Reproduce, find the cause, make the test deterministic without
  weakening what it proves (or fix the engine if the cause is a defect).
- **S3** — The user's request (L1). The project's checks (`cargo fmt --all --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo test --workspace`), RDD per work-unit commit, the Codex code review over the
  range, the closing report.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S2: reproduce and fix the flaky AC-H1 test | delegated (parallel with T2) | pending | |
| T2 | S1: measure the AC10 sweeps between `6b49bea` and `0cb816c`/`d7fe899`, find the slowing commit | delegated (parallel with T1) | pending | |
| T3 | S1: fix the cause found by T2 | pending route | pending | |
| T4 | S3: checks, RDD, Codex review, closing report | inline | pending | |

## Log

- **L1** (2026-11-19) — The user's request, verbatim: «Analiza los últimos checks de la CI», then, after the
  analysis, «Sí, investiga y corrije los dos». The estimate went to the user as «Entre 2 h y 4 h. ¿Empiezo?», and the
  user answered «Sí» (start 06:38:20 +0000).
