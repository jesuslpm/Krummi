---
name: review-prd
description: "Trigger: review a PRD, revisa el PRD, revisión del PRD, check a PRD before implementation. Review a Krummi PRD written by Claude, read-only, and return findings by section with the gap sweep."
---

# Review a PRD

Read-only review of a PRD in `docs/prd/krummi/`. You return findings. Only the coordinator saves the report
under *Review reports* in `AGENTS.md`; delegated reviewers never author files. Never deliberately modify
reviewed or tracked files or decide product rules: the user does. Necessary checks follow *Verification
side effects* in `AGENTS.md`.

## Delegation

The coordinator follows *Review delegation* in `AGENTS.md` before the full review and consolidates the
reviewer's evidence into the output below. A task marked `Delegated reviewer` executes this skill directly
and never delegates again. If delegation is unavailable or the user requests an inline review, apply the
same rubric inline and report the limitation when applicable.

## Before reviewing

Read `AGENTS.md` and `CLAUDE.md`. Apply the *Codex model and reasoning effort* policy in `AGENTS.md`:
routine PRD reviews use `gpt-6.1-sol` / `high`; requirements affecting critical storage invariants use
`gpt-6-astra` / `xhigh`. Apply these settings to the delegated reviewer, not to the coordinator's running session.

## Two stages, one review

### 1. Independent analysis

Read the Spanish PRD (authoritative) and every PRD of its ancestry, following **Parent document** up to
`krummi.prd.md`. Examine its references, dependencies and existing code, checking claims such as *Starting
state* and *Reuse* against source. Identify contradictions, omissions, untestable criteria and decisions
with different observable interpretations. Form your own preliminary findings before consulting Claude's
reviewer conclusions or its PRD-production skills and template. If earlier conclusions are already in the
supplied context, treat them as unverified claims, not as the starting rubric or evidence.

### 2. Methodological check

Read `.claude/skills/prd/SKILL.md`, including the **Reviewer** role and **Gap sweep** in *Subagents*, and
`.claude/skills/prd/assets/prd-template.md`. Ignore the skill's `model` and `effort` columns. Check conformity
with those artifact requirements and the project rules using the rubric below. Any Claude reviewer
conclusions consulted at this stage must be checked independently against requirements and source.

Both stages belong to the same reviewer and produce one consolidated set of findings. Reuse evidence,
merge duplicates and perform only the additional checks needed; do not launch a second review for stage 2.

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

The coordinator saves this output with the metadata and stable finding IDs required by *Review reports*
in `AGENTS.md`, then links the report in chat. Follow-up reviews reference the original finding IDs and
verify the supplied fixes without modifying earlier reports.
