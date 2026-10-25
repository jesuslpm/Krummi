---
name: review-code
description: "Trigger: review the code, revisa el código, review this branch, review these commits, revisión de código. Review a Krummi change written by Claude, read-only, against its PRD, its ODD specs and the project's engineering principles."
---

# Review code

Read-only review of a change: the working diff, a branch against its base, or a commit range. You return
findings. You never edit files, never commit and never change git state.

## Before reviewing

1. Read `AGENTS.md` and `CLAUDE.md`, including the engineering principles.
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
  it, with its messages verbatim. Nothing outside *Out of scope* is implemented. Silently ignoring an explicit
  option, or changing existing output nobody asked to change, is a blocker.
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
- **Checks.** You may run the three `cargo` checks from `CLAUDE.md`. They do not modify the tree. Report the
  observed result.

## Output

List the findings, most severe first. Each one has:

- `path:line`;
- the problem;
- a concrete failure scenario: the inputs or state, and the wrong result;
- the linked `S#` or acceptance criterion, when there is one;
- the severity: **blocker** (a defect or unmet spec caused by the change that reproduces with realistic input),
  **major** or **minor**.

A defect that also exists at the base is an advisory and does not block. End with the check results and a
one-line verdict.
