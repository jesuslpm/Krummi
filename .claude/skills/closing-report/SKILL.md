---
name: closing-report
description: "Trigger: close a feature, closing report, informe de cierre, finish ODD/SDD/vibe-coding run, merge done. Write the Markdown closing report of a change and save it in the repository."
license: MIT
metadata:
  author: inercya
  version: "1.3"
---

## Activation Contract

Load when an implementation run ends (ODD, SDD or vibe coding), after the last verification and before
telling the user the work is done. The report is written to the repository, not only to the chat. It is the
record a reader opens to learn what changed, how long it took, what was verified and what is still pending.

## Hard Rules

- Facts only. Every figure comes from a command, a test run, a script report, `git log` or `date`; never
  from memory or an estimate. Mark anything not measured as not measured.
- Time: compare the real duration with the estimate given at the start. Waits for the user are reported
  apart from active time. Use only clock times (`date`, commit times), never guessed ranges.
- Time by phase: generate it with `scripts/phase_times.py report` from the phase log kept during the run
  (see Phase log); never split the time into phases by hand. A run with no phase log says "not measured by
  phase" instead of the table.
- Change report: generate it with `scripts/numstat_by_area.py`; do not add lines by hand.
- Report failed, skipped and pending checks honestly; never present a partial result as complete.
- Language: match the project's PRDs; translate the template headings consistently.
- The report never authorizes delivery. Push, PR and merge remain the user's decisions.

## Location

| Approach | Report path |
|---|---|
| ODD | Next to the feature document: `odd/tasks/<feature>-report.md` |
| SDD | Next to the change's artifacts (the change folder) as `report.md` |
| Vibe coding or no execution document | `docs/reports/<YYYY-MM-DD>-<change-name>.md` |

Link the report from the execution document's closing section and the execution document from the report.

## Phase log

The phase log is kept during the run, not at its end: the closing report can only show the phases that were
recorded when they happened. Its path is the report path with `-phases.tsv` instead of `-report.md` (ODD:
`odd/tasks/<feature>-phases.tsv`; SDD: `phases.tsv` in the change folder; vibe coding:
`docs/reports/<YYYY-MM-DD>-<change-name>-phases.tsv`). It is committed with the report.

Run `python -I .claude/skills/closing-report/scripts/phase_times.py <command> <log> ...` (`python3` where there
is no `python`) at each change of phase:

| When | Command |
|---|---|
| The first exploration of the run starts | `mark <log> explore` |
| The questions or the estimate go to the user and the run waits | `mark <log> wait --label "<reason>"` |
| The user confirms the start | `mark <log> <phase> --start` (usually `implement` or `plan`) |
| The foreground work changes phase | `mark <log> <phase> [--label <task id>]` (in ODD always pass the task id, so the time by commit names it) |
| A task runs in the background (an acceptance run, a long build) | `bg-start` / `bg-end <log> <phase> "<label>"`, chained in the same background command around the run with `;`, so the end is recorded even when the run fails |
| The run waits for the user again | `mark <log> wait --label "<reason>"`, then `mark` the next phase |
| The verification ends, before writing the report | `end <log>` |

Phases: `explore` (map of the code and the PRD), `plan` (execution document), `implement` (code and its tests,
RED/GREEN/refactor), `checks` (fmt, clippy, test suite), `acceptance` (acceptance runs), `verify`
(independent verifier), `review` (RDD review), `fix` (fixes of findings from tests, verifiers or reviews),
`docs` (documentation), `close` (report and merge), `wait` (wait for the user, never active time). Only one
foreground phase runs at a time; a foreground acceptance run is `mark acceptance`, a background one is
`bg-start`/`bg-end`. A `mark` after `end` resumes the run (a later session); the gap is reported as a pause.

## Execution Steps

1. Collect the evidence: final test and check results, acceptance results, review outcomes, commits. Build the
   RDD reviews table from the record kept during the run (execution document log, or the running list in
   vibe coding); Gentle AI keeps no findings after a review is acknowledged. Mark a review with no record as
   "not recorded" instead of reconstructing it.
2. Measure time: run `phase_times.py end <log>` when the verification ends, then
   `phase_times.py report <log> --lang es --commits <base>..HEAD`, with the same base as the change table
   (step 3). Paste its output in the Time section: time by phase, total active time, active time since the
   confirmed start (the figure to compare with the estimate), waits, the background tasks and the time by
   commit (each commit with its message, its task labels and its phases; in ODD a commit is a task, in vibe
   coding a working step). Name the phases that took the most time when explaining the deviation from the estimate.
   Without a phase log, use the start time recorded at the beginning and the end time from `date`.
3. Generate the change table from the branch point (or the commit before the first change):
   `python -I .claude/skills/closing-report/scripts/numstat_by_area.py <base> HEAD --lang es`, adding
   `--apart '<glob>'` for each generated file (for example an exported `slides.html`) so it stays out of the
   totals. Where there is no `python` command (most Linux systems), run it with `python3`.
4. Fill `assets/closing-report-template.md`, section by section; delete a section only when it truly does
   not apply, and say why in one line.
5. Save it at the path from the Location table, link it as described, and commit it with the change when the
   change is being committed (`docs: add the <feature> closing report`).

## Output Contract

Return the report path, the real-versus-estimated time line and the list of pending items.

## References

- `assets/closing-report-template.md` — report skeleton with per-section guidance.
- `scripts/numstat_by_area.py` — lines added and removed by area, with totals; lockfiles, binaries and
  `--apart` globs listed apart; renames counted under their destination. The lines added in code files
  (the languages of its `LANGUAGES` table, which the table's legend names) are split into code and comments
  (line and block comments, doc comments, Python docstrings); blank lines count in neither, and other files
  show `—`.
- `scripts/phase_times.py` — phase log during the run and, at its end, the time by phase and by commit.
- Tests of both scripts: `python -m unittest discover -s .claude/skills/closing-report/scripts`.
- Example: `odd/tasks/btree-report.md`.
