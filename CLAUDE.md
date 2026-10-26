# Krummi — project instructions

Rust embedded storage engine inspired by Voron, the storage engine of RavenDB. See `README.md` for the layout
and `docs/methodology/` for the working method, which comes from the chicago-crime repository.

## Workflow

- **PRD first.** Every change to product behavior starts with a PRD written with the `prd` skill, in
  `docs/prd/krummi/`. The Spanish PRDs are authoritative; `*.prd.en.md` are translations. A PRD does
  not authorize implementation; wait for the user to ask for it.
- **Execution after the PRD.** ODD by default. Use vibe coding or SDD only when the user asks for it. Bug
  fixes, refactors and tooling with no product change need no PRD.

## External review reports

Codex saves consolidated PRD and code reviews under `docs/reviews/<feature>/`, using the *Review reports*
protocol in `AGENTS.md`. Preserve those reports. When addressing a finding, record its ID, disposition,
evidence and fix commit or artifact version in the report's separate `*-resolutions.md` file, with dated,
append-only updates. Link the report and resolutions from the execution log or closing report when relevant.
A claimed fix remains unverified until a follow-up review confirms it; do not rewrite the reviewer's verdict.
**Codex cross review, always.** Run it with the `codex-review` skill (review, fix or dismiss with evidence,
verify, up to three rounds) without waiting to be asked, as RDD runs:

- **PRD:** once the draft is agreed with the user and no product question is open, before its translations and
  its elaboration time.
- **Code:** at the end of every implementation run (ODD, SDD, vibe coding), once its checks pass and after the RDD
  review of the last slice, over the run's whole range, before the closing report. The closing report links its
  reports and resolutions.

Product questions it raises are asked after the loop, one at a time.

## Engineering principles

These apply to every flow (ODD, SDD, vibe coding) and to any code change.

- **Simplicity, efficiency, and maintainability.** Strive for the best balance between these three qualities.
  Prefer straightforward designs that are easy to understand, verify, and maintain, without sacrificing
  performance unnecessarily.
- **DRY over WET.** Prefer reusing existing code and consolidating duplicated logic. Before implementing new
  functionality, look for opportunities to reuse or extend existing components.
- **Avoid premature abstractions.** Do not introduce abstractions, generic frameworks, or additional layers unless
  they provide a clear benefit. A small amount of duplication is preferable to unnecessary complexity.
- **Performance by design, optimization by measurement.** Choose efficient algorithms and data structures from the
  outset, but introduce additional complexity for performance only when justified by measurements or clear
  requirements.
- **Preserve correctness.** Never compromise correctness, safety, or established invariants for simplicity, reuse,
  or performance.

## Before starting any execution (ODD, SDD or vibe coding)

1. **Questions first.** Ask every open product question before the run starts. Once it starts, do not stop to
   ask: take a reasonable default, record it in the execution document and list it in the closing report.
2. **Estimate, then ask to start.** When no questions are left, send a message with only the time range for
   the whole run and the question whether to start, then stop and wait. Nothing else goes in that message (no
   plan, map or progress), so the estimate does not get lost in the agent's activity. After the user confirms,
   record the estimate and the real start time from `date`, and mark it in the phase log with `--start`. Past runs finished in about half the estimated
   time; calibrate with the earlier closing reports in `odd/tasks/*-report.md` (until this repository has its
   own, use those of `C:\rust\chicago-crime\odd\tasks`).
3. **Always verify.** Never ask whether to run tests, verifiers or reviews (RDD and Codex): run them. Automated
   review consent is always granted.

## Vibe coding from a PRD

When the user asks for vibe coding, follow the `vibe-coding` skill. It sets the bookends: a map of the
existing code, questions and estimate before the first change; DRY while working (search before writing,
refactor after each green step); a test per acceptance criterion, verification against the PRD and a closing
report with the criterion → test → result table and the reuse outcome at the end. No execution document,
phases or subagents.

## Subagent models

Gentle AI saves the Claude Code model assignment of the ODD roles but does not apply it yet: the subagents
inherit the session model (gentle-ai issue [#5252](https://github.com/Gentleman-Programming/gentle-ai/issues/5252),
fix pending in PR [#5269](https://github.com/Gentleman-Programming/gentle-ai/pull/5269)). Until a release applies
it, pass this table's `model` and `effort` every time a subagent is launched for one of these roles:

| Role | Launched for | `model` | `effort` |
|---|---|---|---|
| ODD Explorer | Read-only exploration that maps code for a decision | `opus` | `medium` |
| ODD Worker | A delegated writer: one unit with its code, tests and docs | `opus` | `high` |
| ODD Verify | An independent verifier | `sonnet` | `high` |
| General delegation | Any other subagent: research, analysis | `sonnet` | `medium` |

The table mirrors the assignment made in the Gentle AI TUI (saved in `~/.gentle-ai/state.json` on 2026-10-17):
change both together. RDD reviewers and Judgment Day agents are not listed, because their own definitions in
`~/.claude/agents` already carry their model. Remove this section when a Gentle AI release writes its own
Claude ODD assignment table into the global `CLAUDE.md`.

## During the run

- Work on a feature branch with one local commit per task (Conventional Commits, no AI attribution).
- Write only measured times in logs: `date`, commit times or script reports, never estimates.
- If the run must wait for the user, log when the wait starts and ends; waits are not active time.
- Keep the phase log from the first exploration to the end of the verification (`closing-report` skill, *Phase
  log*): one `phase_times.py mark` at each change of phase (exploration, code and tests, checks, acceptance,
  verification, RDD review, fixes, docs, closing, waits), and `bg-start`/`bg-end` chained around every
  background run, such as an acceptance run. The closing report's time by phase comes only from this log.
- Record each RDD review when it ends: commits, lenses, result, findings with severity and resolution, and
  lineage. Write it in the execution document's log (in vibe coding, the running list). Gentle AI keeps none
  of it after the review is acknowledged, and the closing report's RDD reviews table is built from this record.
- Checks: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`.

## Closing

- **Review the last slice.** When RDD is on and commits remain after the last reviewed boundary (the last
  assessment said `under_budget`), request their review before the final checks and the closing report, and
  follow its transitions to the end, so every merged commit has been through RDD:

  ```text
  gentle-ai review status --cwd <repo> --contract gentle-ai.review-integration/v2 --agent claude-code \
    --next-transition --base-ref <last reviewed boundary> --committed-only \
    --lenses <lenses> --lenses-reason "<what changed and how>"
  ```

  Generated files (for example `docs/methodology/slides.html`) go in a commit of their own, after their source
  has been reviewed or assessed. A last slice made only of generated files needs no review: it is reviewed
  through its source.

- Write the closing report with the `closing-report` skill and save it in the repository next to the
  execution document (for ODD, `odd/tasks/<feature>-report.md`).
- **Never merge automatically.** Close the run on its feature branch, fully verified and with its report, and
  leave it unmerged: the user reviews it first (for example with an independent review). Merge into `main` only
  when the user asks (in a worktree copy, see *Working in a worktree*). Do not push until the user asks.

## Working in a worktree

A worktree copy holds one change at `C:\wt\krummi-<n>` on branch `feat/<n>`; its `.git` is a file, while
in the main copy (`C:\projects\Krummi`) it is a folder. Create, list, merge and remove copies only with the
`worktree` skill. Guide: `docs/worktrees.md`.

- **Merge into the base, only when asked.** In a copy, the developer decides what is merged and what is
  discarded, so the run closes without merging, as in the main copy (Closing section above), and merges into
  the copy's base only when the developer says
  `Fusiona el cambio en la base`, through the worktree merge (`worktree` skill), never with git by hand.
  Nothing is pushed.
- **Acceptance work folder.** Acceptance runs and generated datasets use the copy's `work` folder (ignored by
  git), for example `-WorkDir work\acceptance`, never a fixed folder such as `D:\acceptance`.
- **Stay inside the copy.** Never write in the main copy or in other copies; a guard hook blocks it.
- **Never Claude Code's built-in worktrees** (`EnterWorktree`, `--worktree`, worktree isolation), neither in
  the main copy nor in a worktree copy.
- The main copy keeps every rule above unchanged.

## Repository rules

- Code, identifiers, comments and technical docs in English; CLI messages and PRDs in Spanish.
