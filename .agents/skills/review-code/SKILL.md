---
name: review-code
description: "Trigger: review the code, revisa el código, review this branch, review these commits, revisión de código. Review a Krummi change written by Claude, read-only, against its PRD, its ODD specs and the project's engineering principles."
---

# Review code

Read-only review of a change: the working diff, a branch against its base, or a commit range. You return
findings. Only the coordinator saves the report under *Review reports* in `AGENTS.md`; delegated reviewers
never author files. Never deliberately modify reviewed or tracked files, commit or change git state.
Necessary build artifacts and temporary test files follow *Verification side effects* in `AGENTS.md`.

## Delegation

The coordinator follows *Review delegation* in `AGENTS.md` before the full review and consolidates the
reviewer's evidence into the output below. A task marked `Delegated reviewer` executes this skill directly
and never delegates again. If delegation is unavailable or the user requests an inline review, apply the
same rubric inline and report the limitation when applicable.

## Before reviewing

1. Read `AGENTS.md` and `CLAUDE.md`, including the engineering principles.
   Apply the *Codex model and reasoning effort* policy in `AGENTS.md`: routine code reviews use
   `gpt-6.1-sol` / `high`; reviews affecting critical storage invariants use `gpt-6-astra` / `xhigh`.
   Apply these settings to the delegated reviewer, not to the coordinator's running session.
2. Find what the change implements:
   - **ODD.** The execution document is `odd/tasks/<feature>.md`. Read it up to `## Log`, then read the `## Log`
     for corrections and decisions made during the run.
   - **Vibe coding.** There is no execution document. Use the PRD and, when it exists, the closing report
     `odd/tasks/<feature>-report.md`.
   - **The PRD itself and its ancestry**, following **Parent document** up to `krummi.prd.md`.
3. Find the diff. For a branch, use `git merge-base main HEAD`. For commits, use the range you are given. Read
   whole functions around each hunk, not only the changed lines.

## What to check

- **Specs.** Every `S#`, and every acceptance criterion the change claims, must be implemented as the PRD states
  it, with its messages verbatim. Nothing explicitly listed as *Out of scope* is implemented. Silently
  ignoring an explicit option, or changing existing output nobody asked to change, is a blocker when
  established by the evidence required below.
- **Tests.** Every rule has a test through the public interface that asserts every observable effect (output,
  exit code, persisted data). Every existing behavior that shares the changed code still has a test.
- **Correctness.**
  - Invariants of the storage engine: pages, transactions, journal, free space and trees.
  - Error paths and partial writes.
  - Concurrency and ordering.
  - `unsafe` code.
- **Principles.**
  - Duplicated logic, constants or messages that repeat existing code.
  - Premature abstractions.
  - Performance complexity added without a measurement or requirement.
- **Repository rules.** Code and comments are in English and CLI messages are in Spanish. Help text and docs are
  updated when a command, option or message changed.
- **Checks.** You may run the three `cargo` checks from `CLAUDE.md`, subject to *Verification side effects*
  in `AGENTS.md`. Compare repository contents before and after checks and report observed results,
  unexpected changes and checks skipped for lack of authorization.

## Output

List the findings, most severe first. Each one has:

- `path:line`;
- the problem;
- a concrete failure scenario: the inputs or state, the causal chain through the code and the wrong result;
- verifiable evidence: a reproduction or a technical argument with source references, reachable preconditions
  and the violated requirement or invariant; identify which kind of evidence supports the finding;
- the linked `S#` or acceptance criterion, when there is one;
- the severity: **blocker** (a critical defect or material unmet spec introduced by the change, established
  by reproduction or verifiable technical evidence of failure under realistic conditions), **major**
  (a demonstrated substantive defect or requirement gap with lower impact), or **minor** (a limited-impact
  defect or clarity/maintainability issue).

Lack of experimental reproduction alone does not lower severity: a rigorous causal argument can establish
memory-safety, concurrency or durability failures. Separate demonstrated defects from hypothetical risks.
Label unproven risks explicitly, state the missing evidence and do not present them as demonstrated blockers.

A defect that also exists at the base is an advisory and does not block. End with the check results and a
one-line verdict.

The coordinator saves this output with the metadata and stable finding IDs required by *Review reports*
in `AGENTS.md`, then links the report in chat. Follow-up reviews reference the original finding IDs and
verify the supplied fixes without modifying earlier reports.
