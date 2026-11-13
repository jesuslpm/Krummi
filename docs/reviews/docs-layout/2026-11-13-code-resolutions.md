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
