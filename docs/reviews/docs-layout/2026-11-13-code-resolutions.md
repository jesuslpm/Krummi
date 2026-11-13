# Docs layout code review resolutions

Resolutions of [2026-11-13-code.md](2026-11-13-code.md), the review of `80027e1..a406e8e` (the docs layout run).
Append-only: each update is dated and keeps the earlier ones. A claimed fix stays unverified until a follow-up review
confirms it.

## 2026-11-13

Artifact versions: fixes in commit `ec5011b`, on branch `feat/docs-layout`.

### R1 — major: an invalid destination leaves rewritten references behind

- Disposition: fixed.
- Evidence: reproduced by the new test `test_a_destination_inside_a_file_changes_nothing` (a move of `a.md` to
  `blocked/a.md`, where `blocked` is a tracked file, and a move whose destination is inside another destination),
  which fails on `a406e8e`: the first case raised `FileExistsError` after the references were written.
- Fix (`ec5011b`): before writing anything, `move` checks every folder of each destination: none may be a file
  (`El destino blocked/a.md queda dentro de blocked, que es un archivo`) or another destination of the same move.
  The test also asserts that the referencing file and the tracked files are unchanged.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — major: archiving breaks valid angle-delimited Markdown destinations

- Disposition: fixed.
- Evidence: reproduced by the new test `test_angle_bracket_destinations_follow_their_target` (inline links with
  `<../../README.md>` and `<../../my notes.md#top> "Notes"`, a reference definition `[root]: <../../README.md>`, and
  both forms in a Rust doc comment), which fails on `a406e8e`.
- Fix (`ec5011b`): one `DESTINATION` pattern takes a destination between angle brackets (with spaces inside) or one
  without them, in inline links and in both kinds of reference definitions; `link_path` reads the path inside the
  brackets, and a rewritten destination keeps them. `check` resolves them the same way, so they are neither missed
  nor reported as broken.
- Status: claimed fixed, unverified until a follow-up review.

### R3 — minor: the new CLI messages are in English

- Disposition: fixed.
- Evidence: `CLAUDE.md`, *Repository rules*: «CLI messages and PRDs in Spanish.»
- Fix (`ec5011b`): every message the command prints or exits with is Spanish (`Se movieron 1 archivos y se
  reescribieron referencias en 1 archivos.`, `No hay enlaces rotos.`, the validation errors), and the help is a
  Spanish `USAGE` text instead of the English module docstring, which stays as the code's documentation. Output is
  printed as UTF-8. The tests assert the messages.
- Status: claimed fixed, unverified until a follow-up review.

Checks after the fixes: `python -m unittest discover -s scripts/docs` 18 tests pass; the eight new or changed tests
fail when run against `a406e8e`'s script. `python -I scripts/docs/move_docs.py check` lists the 37 earlier broken
links and four example links of the review report itself, written as code (inline code and a fenced block); the
module docstring now says that links in code count as links.

Review record: [2026-11-13-code.md](2026-11-13-code.md), wrapper status `findings`, thread
`01a1d4a1-ef38-7522-a67e-ae13fa7e7ba8`; telemetry: coordinator `gpt-6.1-sol` / high, reviewer `krummi-review`
`gpt-6.1-sol` / high.

## 2026-11-13 (verification)

Follow-up report: [2026-11-13-code-02.md](2026-11-13-code-02.md), fix range `a406e8e..87c0ce0`, thread
`01a1d4a8-7e2c-7b20-8d10-d13e960f0b92`, wrapper status `findings`; telemetry: coordinator `gpt-6.1-sol` / high,
reviewer `krummi-review` `gpt-6.1-sol` / high. Per-ID outcome, quoted: R1 «still open, partially corrected by
`ec5011b`»; R2 «Verified resolved: angle-delimited Markdown destinations»; R3 «Verified resolved: explicit CLI
messages and help»; R4 new, «the angle-link fix breaks reference destinations containing parentheses».

## 2026-11-13 (round 2)

Artifact versions: fixes in commit `349fb94`, on branch `feat/docs-layout`.

### R1 — major: destination aliases still allow a rejected move to leave partial changes

- Disposition: fixed.
- Evidence: reproduced by the new test `test_paths_of_a_move_are_compared_normalized` (the report's manifest,
  `a.md` → `./x.md` and `b.md` → `x.md/b.md`), which fails on `87c0ce0` with the `FileExistsError` of the report.
- Fix (`349fb94`): `move` normalizes every path of the move before any check (forward slashes, no `.` or `..`
  steps), so the folder and duplicate checks compare one spelling; a source given twice under two spellings
  (`El archivo a.md aparece dos veces`) and a path that leaves the repository (`La ruta ../a.md queda fuera del
  repositorio`) are refused. The test asserts that the referencing file and the tracked files are unchanged, and that a
  valid move written with `./` steps moves the file and rewrites its link.
- Status: claimed fixed, unverified until a follow-up review.

### R4 — major: the angle-link fix breaks reference destinations containing parentheses

- Disposition: fixed.
- Evidence: reproduced by the new test `test_reference_destinations_keep_their_parentheses` (the report's
  `[g]: ../../guide(v2).md`, archived), which fails on `87c0ce0`.
- Fix (`349fb94`): a Markdown reference definition's destination is again everything up to a space, as at the base,
  or one between angle brackets; inline links keep the destination pattern of `ec5011b`, which ends at a `)` as at
  the base.
- Status: claimed fixed, unverified until a follow-up review.

Checks after the fixes: `python -m unittest discover -s scripts/docs` 20 tests pass; `check` lists the same 41 links
as the follow-up counted (37 earlier ones and four examples written as code in the first report).

## 2026-11-13 (verification, round 2)

Follow-up report: [2026-11-13-code-03.md](2026-11-13-code-03.md), fix range `87c0ce0..9518704`, thread
`01a1d4af-eeb2-7981-90ba-76bcd20e7bbb`, wrapper status `findings`; telemetry: coordinator `gpt-6.1-sol` / high,
reviewer `krummi-review` `gpt-6.1-sol` / high. Per-ID outcome, quoted: R1 «still open, partially corrected by
`349fb94` and `780c08a`» («Windows destination aliases still permit partial changes before rejection»); R4
«Verified resolved: reference destinations containing parentheses».

## 2026-11-13 (round 3)

Artifact versions: fix in commit `b2674f1`, on branch `feat/docs-layout`. This follows the third Codex round, the
limit of automatic rounds: its verification needs a further round authorized by the user.

### R1 — major: Windows destination aliases still permit partial changes before rejection

- Disposition: fixed.
- Evidence: the report's manifest (`a.md` → `x.md`, `b.md` → `x.md./b.md`) reproduced the half-done move on Windows;
  the new tests `test_windows_spellings_are_refused` and `test_a_move_that_fails_midway_is_undone` fail on
  `9518704`.
- Fix (`b2674f1`), at the root rather than for this alias alone:
  - `normalize` refuses any name Windows does not keep as written: one ending in a dot or a space, a reserved device
    name with or without an extension (`nul.md`, `Com1`), or one with a character Windows refuses (`La ruta x.md./b.md
    tiene un nombre que Windows no admite: x.md.`).
  - `move` is undone when it fails midway for a reason the checks could not foresee: the `git mv` already done are
    reversed, the rewritten texts restored and the empty folders it made removed, and it exits with `No se pudo
    completar el movimiento (...); se deshicieron sus cambios`. The test makes the second `git mv` fail and finds the
    texts, the tracked files and `git status` as before.
  - The report's exact reproduction, rerun through the CLI in a disposable repository: exit 1 before any write,
    `git status --porcelain` empty, README unchanged.
- Status: claimed fixed, unverified until a follow-up review (a fourth round needs the user's authorization).

Checks after the fix: `python -m unittest discover -s scripts/docs` 22 tests pass; `check` lists 45 links: the 37
earlier ones and eight example links written as code in the three review reports.

Addendum (2026-11-13, after the RDD review of `b2674f1`): commit after `bf74160` makes the undo itself
failure-safe (each step goes on when another fails; a text is restored where its file is; the folders `mkdir` made
before failing are removed) and narrows the reserved names to Windows' exact set. Tests
`test_a_rollback_that_fails_restores_every_text_where_its_file_is` and
`test_a_folder_that_fails_midway_is_undone_with_the_folders_it_made`; 24 tests pass. Status unchanged: claimed
fixed, unverified until a follow-up review.

## 2026-11-13 (verification, round 3)

Follow-up report: [2026-11-13-code-04.md](2026-11-13-code-04.md), fix range `9518704..438889a`, a fourth round
authorized by the user; thread `01a1d51a-094a-72f2-aadb-e938ff7d71c9`, wrapper status `approved`; telemetry:
coordinator `gpt-6.1-sol` / high, reviewer `krummi-review` `gpt-6.1-sol` / high. Checks before it:
`python -m unittest discover -s scripts/docs` 25 tests pass; `check` lists the same 45 links. Per-ID outcome, quoted:
R1 «Verified resolved: invalid moves leave partial changes»; verdict «R1 is verified resolved. No finding remains
open in the requested scope.»
