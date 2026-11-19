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
| T1 | S2: reproduce and fix the flaky AC-H1 test | delegated (parallel with T2) | done (2 of 320 under load before, 0 of 480 after; workspace 970 passed) | `58a9798` |
| T2 | S1: measure the AC10 sweeps between `6b49bea` and `0cb816c`/`d7fe899`, find the slowing commit | delegated (parallel with T1) | done (no slowing commit; L3) | — |
| T3 | S1: fix the cause found by T2 | inline | done (sweeps in their own job; YAML parsed) | `bcbe4ef` |
| T4 | S3: checks, RDD, Codex review, closing report | inline | pending | |

## Log

- **L1** (2026-11-19) — The user's request, verbatim: «Analiza los últimos checks de la CI», then, after the
  analysis, «Sí, investiga y corrije los dos». The estimate went to the user as «Entre 2 h y 4 h. ¿Empiezo?», and the
  user answered «Sí» (start 06:38:20 +0000).
- **L2** (2026-11-19) — T1 (`58a9798`): the test's last assertion counts `version_checks()`, which only counts layout
  checks of versions read from the scratch area (`krummi-pager/src/scratch.rs:1329-1338`); a background transfer pass
  between `commit` and the read moves the committed versions to the store, whose first read is checked outside that
  counter (Q2 of `pager-written-pages.prd.md`), so the assertion failed. A forced `transfer` after the commit failed it
  every time. The engine follows its PRDs; the test now pauses the transfer (`set_transfer_paused(true)`, an existing
  hook) before the write. Stress loop on Windows debug: before 0/200 alone and 2/320 under load (8 parallel loops plus
  2 whole-suite loops); after 0/200 and 0/480. Checks by the writer: fmt, clippy, workspace 970 passed.
- **L3** (2026-11-19) — T2 (read-only investigation in `work/ci/`): no file under `crates/*/tests`, the CI workflow,
  `Cargo.toml` or `Cargo.lock` changed in `6b49bea..0cb816c`, so the sweeps run the same cases; local timings at a
  step of 16 bytes (4,096 reopens per store) on Windows debug: `6b49bea` 1,055 s, `0cb816c` 1,099 s (+4 %, one round,
  within noise), about 55 ms per case dominated by writing the 64 KiB file and reopening it. The CI history of the
  step for near-identical code: Windows 8.7 min (11-08), 21.8 (11-09), 12.9 (11-16), over 30 (11-17, 11-19); Ubuntu
  11.6, 22.4, 16.3, 18.9, 16.3, over 30 (11-18), 27.4 (11-19). Conclusion: hosted-runner variance against a 30-minute
  step limit with no margin, not a regression; the checks job (60 minutes) had no margin either (Ubuntu 45.5 minutes
  on 11-19).
- **L4** (2026-11-19) — T3 (`bcbe4ef`): the four acceptance sweep steps move to a new job `sweeps` (matrix Ubuntu and
  Windows, 180 minutes), parallel with `checks`; the AC10 step gets 60 minutes, the other steps keep theirs; the
  README says so. The workflow parsed with PyYAML (jobs `checks`, `sweeps`, `benchmark`; step limits as intended); the
  real check is the next CI run, which needs the branch on GitHub. Risk: item 5 (CI). RDD `e5b5f0b..bcbe4ef`
  (resilience, reliability; escalated item 5): approved, `review-da0dc1b06955ab8f`; advisories: the raised limit's
  cause (stated in the job comment and L3), the moved steps (intended), `RUST_TOOLCHAIN` in the new job (a
  workflow-level `env`, available to every job), and the transfer path of AC-H1 left uncovered by that test (other
  tests cover the transfer).
