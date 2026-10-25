---
name: review-prd
description: "Trigger: review a PRD, revisa el PRD, revisión del PRD, check a PRD before implementation. Review a Krummi PRD written by Claude, read-only, and return findings by section with the gap sweep."
---

# Review a PRD

Read-only review of a PRD in `docs/prd/krummi/`. You return findings. You never edit the PRD and never decide
product rules: the user does.

## Before reviewing

1. Read `AGENTS.md` and `CLAUDE.md`.
2. Read `.claude/skills/prd/SKILL.md`. It is how Claude writes the PRD. Its *Subagents* section defines the
   **Reviewer** role and the **Gap sweep**, and that is the rubric you apply. Ignore its `model` and `effort`
   columns.
3. Read `.claude/skills/prd/assets/prd-template.md`. It is the required sections and what belongs in each.
4. Read the PRD and every PRD of its ancestry, following **Parent document** up to `krummi.prd.md`. Review the
   Spanish file: it is authoritative.
5. Read the project docs the PRD relies on (its *References*). When the PRD claims something about the code, for
   example in *Starting state* or *Reuse*, check it against the code.

## What to check

- **Everything the Reviewer role lists.** That means:
  - acceptance criteria that cannot become a test;
  - contradictions with any PRD of the ancestry that the draft does not declare as an override;
  - rules the docs already state;
  - implementation details that leaked in;
  - gaps in *Out of scope*.
- **The gap sweep.** Imagine two people implementing the PRD independently. List every point where their results
  could differ in something the user can observe, each with the readings it allows. Points already listed in
  *Left to the implementation* are not gaps.
- **Starting state against the code.** Every claim must hold, with `path:line` evidence.
- **The translation**, when `.prd.en.md` exists. It must have the same headings, decision and criterion IDs, and
  CLI messages as the Spanish file.

## Output

List the findings, most severe first. Each one has:

- its section and ID (for example `Acceptance criteria / AC-B5`);
- the problem;
- the evidence (a quote, or `path:line`);
- the severity: **blocker** (it cannot be implemented or tested as written), **major** (two readings with
  different observable results) or **minor** (clarity);
- for gaps, the readings it allows.

End with the open product questions for the user and a one-line verdict: whether the PRD is ready for
implementation.
