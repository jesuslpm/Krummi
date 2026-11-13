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
| T4 | checks, reviews, closing report, archiving of this execution | inline | in progress (L7–) | |

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
- **L7** (2026-11-13): T3 committed (`d71f11e`, `e3a390c`). Checks at `e3a390c`: fmt, clippy clean, 831 passed, 0
  failed, 34 ignored; script tests 8 pass; closing report scripts' tests pass; the Codex wrapper's Pester tests 20
  passed, 0 failed. RDD of the whole slice `80027e1..e3a390c` stopped at START with `lens_context_budget_exceeded`
  (59,711 lines: the migration) and created no authority («review this change as smaller candidates that each fit
  under it»). Split: the script commits `80027e1..0956809` (416 lines), lenses `review-reliability` and
  `review-readability`, lineage `review-0d938d1b24126f81`: **approved**, acknowledged, with warnings and suggestions,
  all handled in the next commit (each with a test that fails on `e3a390c`'s script, run from a temporary copy):
  - R2-001 and R3-manifest-path-after-chdir (WARNING): a relative manifest path was read from the root: now from the
    current folder (`test_a_relative_manifest_is_read_from_the_current_folder`).
  - R2-002 (WARNING) and R2-004 (SUGGESTION): `check` scanned only `.md` files and repeated the resolution of a
    target: it scans `.md` and `.rs`, through one `link_path` helper (`test_check_reports_broken_links_of_rust_doc_comments`).
  - R2-003 (WARNING): the pass of root paths also rewrites link targets left by the link pass (a broken link whose text
    is a moved root path): dismissed as a behavior, the docstring now says so (the link stays broken either way).
  - R3-partial-move-on-duplicate-destination (WARNING): two entries with one destination left a half-done move: the
    move is checked before anything is written (`test_two_files_moving_to_one_place_change_nothing`).
  - R3-rust-reference-definitions-unhandled (WARNING): reference definitions after `///` or `//!` were not rewritten:
    now they are (`test_rust_reference_definitions_follow_their_target`). Scanning Rust files then reported three
    lines of prose that start like a definition ("[`Store::write_with_timeout`]: when it expires"): a definition
    names a file only when its target has a `/` or a `.` (part of the same test of `check`, which fails without it).
  - R2-005 and R3-manifest-malformed-line (SUGGESTION): a malformed manifest line raised a bare `ValueError`: now a
    message names it (`test_a_malformed_manifest_line_is_named`).
  13 tests pass; `check` gives the same 37 broken links.
- **L8** (2026-11-13): fixes committed (`c8b751a`). RDD of `2503b38..c8b751a` (255 lines: `reopen`, the
  instructions and those fixes), lenses `review-readability` and `review-reliability`, lineage
  `review-144db57e6b9df6e1`: **approved**, acknowledged, with findings handled in the next commit:
  - R2-reference-indent-bound-dead and R3-reference-indent-unbounded (WARNING): the leading `\s*` added for Rust made
    the three-space bound of Markdown dead and could cross lines: one pattern per kind of file, `MD_REFERENCE` (at
    most three spaces) and `RS_REFERENCE` (after `///` or `//!`), with `[ \t]` instead of `\s`.
  - R3-reference-filter-applies-to-markdown (WARNING): the `/`-or-`.` filter also skipped Markdown definitions of a
    file without an extension: it applies to Rust only. Test `test_markdown_reference_definitions_keep_their_rules`
    (a definition of `../LICENSE` is rewritten, one indented four spaces is not), failing on `c8b751a`.
  - R2-docstring-example-old-layout (SUGGESTION): the docstring's example shows a PRD in its module folder again.
  - R3-reopen-ambiguity-untested (SUGGESTION): test `test_reopening_refuses_an_execution_archived_twice` (it passes on
    `c8b751a` too: the guard was there, untested).
  15 tests pass; `check` gives the same 37 broken links.
- **L9** (2026-11-13): those fixes committed (`8bb718f`), reviewed alone from `c8b751a` (57 lines), lens
  `review-reliability`, lineage `review-849d614c0293a1e6`: **approved**, acknowledged; one finding,
  R3-markdown-footnote-reported-broken (WARNING): without the filter, a GFM footnote (`[^1]: Some text`) read as a
  definition whose target is its first word. Fixed in the next commit: a label that starts with `^` is no definition,
  in both patterns (`test_footnotes_are_no_links`, failing on `8bb718f`). 16 tests pass; `check` gives the same 37
  broken links.
- **L10** (2026-11-13): footnote fix committed (`700e276`), reviewed alone from `8bb718f` (23 lines), lens
  `review-reliability`, lineage `review-939756838c9a509e`: **approved**, acknowledged; two suggestions on the test,
  R3-rust-footnote-lookahead-unproved (its Rust footnote had no path-like first word, so it passed without the
  exclusion) and R3-footnote-rewrite-path-untested (no move kept a footnote): both footnotes now start with a path
  and the test also moves a file they cite; it fails on `8bb718f`'s script, and removing only the Rust exclusion
  fails it too. 16 tests pass.
- **L11** (2026-11-13): stronger test committed (`c20308b`), reviewed alone from `700e276` (19 lines), lens
  `review-reliability`, lineage `review-52d682c98601b663`: **approved**, acknowledged, with two findings that correct
  L10: R3-footnote-move-assertion-vacuous (WARNING; the moved file was not the first word of either footnote, so the
  move assertions could not fail) and R3-l10-overstates-rewrite-coverage (SUGGESTION; L10's failures came from
  `check`, not from the move). Fixed in the next commit: each kind of file has one footnote that starts with a
  missing path (for `check`) and one that starts with the moved file (for `move`). Mutations
  (`work/docs-layout/mutate.py`), each failing the test: no exclusion in `MD_REFERENCE`, or in `RS_REFERENCE`, each
  once with `check` and once with `check` stubbed so that only the move assertion can fail. 16 tests pass.
- **L12** (2026-11-13): R3-mutation-evidence-not-in-candidate (RDD, SUGGESTION, L11) dismissed: by this project's
  convention measurement and probe scripts stay in the ignored `work/` folder (the logs cite them there), and L11
  names each mutation precisely enough to repeat it. Codex code review, round 1, `80027e1..a406e8e`
  ([report](../../docs/reviews/docs-layout/2026-11-13-code.md), committed `977e8b7`): `findings`, R1 (major: a
  destination under a tracked file passed the checks, and the references were rewritten before `mkdir` failed), R2
  (major: destinations between angle brackets were neither rewritten nor checked) and R3 (minor: the new command's
  messages were English, against «CLI messages and PRDs in Spanish»). Thread `01a1d4a1-ef38-7522-a67e-ae13fa7e7ba8`;
  telemetry: coordinator `gpt-6.1-sol` / high, reviewer `krummi-review` `gpt-6.1-sol` / high. All three fixed in the
  next commit, test first (eight tests failing on `a406e8e`, RED observed): every folder of a destination must be a
  folder or one the move creates, and not another destination; `DESTINATION` takes `<...>` destinations (spaces
  inside) in inline links and both kinds of definitions, and a rewritten one keeps its brackets; messages and the
  help (`USAGE`) are Spanish, printed as UTF-8. 18 tests pass. `check` now also reports four example links of the
  Codex report, written as code (the docstring says links in code count as links).
- **L13** (2026-11-13): Codex follow-up, round 2, fix range `a406e8e..87c0ce0`
  ([report](../../docs/reviews/docs-layout/2026-11-13-code-02.md), committed `687caad`): `findings`. R2 and R3
  verified resolved; R1 still open (a destination spelled `./x.md` escaped the folder check, and the move failed
  half-done); R4 new (major: sharing the inline destination pattern cut a reference definition's destination at a
  parenthesis, `../../guide(v2).md`, which the base handled). Thread `01a1d4a8-7e2c-7b20-8d10-d13e960f0b92`;
  telemetry: coordinator `gpt-6.1-sol` / high, reviewer `krummi-review` `gpt-6.1-sol` / high. Fixed in the next
  commit, test first (both new tests failing): every path of a move is normalized before the checks (forward
  slashes, no `.` or `..` steps), a source given twice and a path leaving the repository are refused
  (`test_paths_of_a_move_are_compared_normalized`); a Markdown definition's destination is again everything up to a
  space, or one between angle brackets (`test_reference_destinations_keep_their_parentheses`). 20 tests pass;
  `check` 41, as before.
- **L14** (2026-11-13): round 2 fixes committed (`349fb94`) and recorded (`e62cc4f`). RDD of `a406e8e..e62cc4f` (502
  lines), lenses `review-readability` and `review-reliability`, lineage `review-5fa68e0f7b624933`: **approved**,
  acknowledged, with findings fixed in the next commit: R3-case-alias-ancestor (WARNING: on a case-insensitive file
  system `X.md` and `x.md/b.md` are one path; the duplicate and folder checks now compare without case, test cases
  failing before the fix), R3-normalize-untested-branches (SUGGESTION: the backslash, leading slash and drive letter
  branches now tested), R2-001 (WARNING: the angle-bracket destination written three times: one `ANGLE` and one
  `LABEL` compose the three patterns), R2-002 (SUGGESTION: `link_path` returns whether the destination had brackets,
  instead of a second `unwrap`), R2-003 (SUGGESTION: the folder comment names the other-destination rule). 20 tests
  pass; `check` 41.
- **L15** (2026-11-13): those fixes committed (`780c08a`). RDD of `e62cc4f..780c08a` (69 lines), lenses
  `review-readability` and `review-reliability`, lineage `review-ac10c3b0dc80e6f7`: **approved**, acknowledged.
  R3-001 (WARNING) and R2-casefold-unconditional (SUGGESTION): the case-insensitive checks also refuse, on Linux,
  two destinations that differ only in case, which the base allowed; kept on purpose (the repository is checked out
  on Windows too, where such a move breaks the copy), and the comment now says so.
- **L16** (2026-11-13): Codex follow-up, round 3, fix range `87c0ce0..9518704`
  ([report](../../docs/reviews/docs-layout/2026-11-13-code-03.md), committed `075e2d9`): `findings`. R4 verified
  resolved; R1 still open with another alias: on Windows `x.md.` is `x.md`, so `a.md` → `x.md` and `b.md` →
  `x.md./b.md` passed the checks and failed half-done (as at the base). Thread
  `01a1d4af-eeb2-7981-90ba-76bcd20e7bbb`; telemetry: coordinator `gpt-6.1-sol` / high, reviewer `krummi-review`
  `gpt-6.1-sol` / high. This is the third round, the limit of automatic rounds. Fixed in the next commit, test first
  (both new tests failing), at the root instead of one alias at a time: a name Windows does not keep as written (a
  final dot or space, a reserved device name, a refused character) is refused (`test_windows_spellings_are_refused`),
  and a move that fails midway is undone, the moves in reverse, then the texts, then the new empty folders
  (`test_a_move_that_fails_midway_is_undone`, which makes the second `git mv` fail and finds the repository clean).
  Codex's exact reproduction now exits 1 before writing, with a clean status. 22 tests pass; `check` 45 (the 37
  earlier ones and eight example links written as code in the three Codex reports).
- **L17** (2026-11-13): round 3 fix committed (`b2674f1`) and recorded (`bf74160`). RDD of `780c08a..bf74160` (234
  lines), lenses `review-readability` and `review-reliability`, lineage `review-e059736b6e56f71a`: **approved**,
  acknowledged. Fixed in the next commit, test first (three tests failing before it):
  R3-rollback-not-failure-safe (WARNING: a failing reverse `git mv` escaped and left the texts rewritten; `undo` now
  goes on step by step, restores each text where its file is, and names what it could not undo:
  `test_a_rollback_that_fails_restores_every_text_where_its_file_is`); R3-mkdir-partial-dirs (WARNING) and
  R3-oserror-path-untested (SUGGESTION: the folders are recorded before `mkdir`, and a `mkdir` that makes `p` and
  `p/q` and fails on `p/q/r` is undone with them: `test_a_folder_that_fails_midway_is_undone_with_the_folders_it_made`,
  whose first version passed without the fix because its stub failed before any parent existed); R2-001 and
  R3-reserved-name-set (SUGGESTION: the exact reserved set, COM1-9 and LPT1-9 with ¹²³, CONIN$ and CONOUT$; `com0` is
  accepted). R2-002 (SUGGESTION, the rollback catches only `OSError` and `CalledProcessError`) dismissed with
  evidence: the real `git` helper runs `subprocess.run(..., check=True)`, which raises `CalledProcessError`, the
  exception the test injects. 24 tests pass; `check` 45.
- **L18** (2026-11-13): those fixes committed (`240d1a6`). RDD of `bf74160..240d1a6` (106 lines), lens
  `review-reliability`, lineage `review-834aa9c11e321ef5`: **approved**, acknowledged. Fixed in the next commit:
  R3-rollback-where-mapping-unproved (WARNING: the rollback test's `a.md` did not change with its move, so it never
  proved where its text is restored; now it links to the README, and restoring at the old path fails it);
  R3-folder-cleanup-failure-silenced (WARNING: a folder the undo cannot remove is now reported,
  `test_a_folder_left_by_the_undo_is_named`, failing on `240d1a6`); R3-mkdir-stub-python-version (SUGGESTION: the
  folder test patches `Path.mkdir` itself, whatever the version; recording the folders after `mkdir` fails it).
  25 tests pass; `check` 45.
