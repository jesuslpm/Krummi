---
name: prd
description: "Trigger: PRD, write a PRD, nuevo PRD, product requirements, documento de producto, start a feature from a PRD. Write the product PRD that a feature's execution (ODD, vibe coding or SDD) starts from."
model: opus
effort: high
license: MIT
metadata:
  author: inercya
  version: "1.3"
---

## Activation Contract

Load before writing or revising any `*.prd.md`, or before starting a change that alters product behavior.
The PRD comes first; the execution that implements it (ODD, vibe coding or SDD) takes it as input. This skill
ends at the PRD and never creates the execution's documents.
Run inline in the main session so the PRD is agreed with the user in conversation — do not delegate the writing.
Subagents run only for the three roles in *Subagents*, always in the foreground, so `prd_time.py` counts
their time as agent time.

## Hard Rules

- Product document: the WHAT and WHY in business language. No class names, endpoints, file paths or line
  numbers — those belong to the execution and the code.
- Language: match the project's existing PRDs; with no precedent, ask the user once.
- Location: reuse the project's PRD convention (search `**/*.prd.md`). With none, use
  `docs/prd/<domain>/<domain>.prd.md` (root) and `docs/prd/<domain>/<name>.prd.md` (module or change).
- PRDs form a hierarchy: the root PRD of the product has no parent; every other PRD (a module, a change to a
  module, a change to a change) names its direct parent in the `Parent document` header field. The number of
  levels is the project's choice; do not force one.
- The ancestry of a PRD is the chain of `Parent document` links from it up to the root. It inherits every rule
  of its ancestry; a lower PRD states only what it adds or overrides, and when it overrides an ancestor's rule
  it says so and the ancestor is annotated (*Decision Gates*).
- Before declaring the ancestry or project docs silent on a rule, re-read them. Rules that feel tacit are
  usually written down.
- `Starting state` reflects what already exists in code and data, verified by search — never assumed.
- `Out of scope` is mandatory and explicit — it is the line the next change starts from.
- No gaps: every point where two reasonable implementations could behave differently in a way the user can
  observe (output keys and shape, messages, exit codes, stored data) is either fixed in the PRD or listed in
  `Left to the implementation` with the user's agreement. Public output is product, not implementation detail.
- Ask one question at a time when a product decision is genuinely the user's; never invent it.
- Writing a PRD does not authorize implementation.
- The `Elaboration time` section is written only by `scripts/prd_time.py`, never by hand and never estimated.

## Decision Gates

| Situation | Action |
|---|---|
| New product or domain | Root PRD, no parent |
| New module, or change inside a module or domain | PRD whose `Parent document` is the closest PRD that covers it |
| Correcting a rule of an earlier PRD | Annotate the earlier PRD; do not rewrite its history |
| Bug fix, refactor or tooling with no product behavior change | No PRD; go straight to ODD |

## Execution Steps

1. Read the whole ancestry, from the direct parent up to the root (follow `Parent document` until it says
   none), and the relevant project docs. Stop with an error if a link is broken or loops back.
2. Verify the starting state in code and data; delegate the explorer (*Subagents*) when it exceeds the inline
   evidence budget.
3. Draft from `assets/prd-template.md`; translate headings consistently when the PRD is not in English.
4. Launch the reviewer (*Subagents*) on the draft, including its gap sweep. Fix what is a wording or
   consistency defect; a finding that needs a product decision, and every gap, becomes a question for the user
   in the next step.
5. Review the draft with the user; iterate until no product question is open. Ask about each gap on its own,
   with the options the reviewer saw; the answer goes into the PRD as a fixed rule or as a line in
   `Left to the implementation`. Relaunch the reviewer when the answers changed the draft.
6. When the PRD is final, launch the translator (*Subagents*) for each translation the project keeps (for
   example `*.prd.en.md`). Check that the translation has the same headings, criterion IDs and messages as
   the original; the original stays authoritative.
7. As the last step of the session, once the PRD and its translations are final, record the elaboration time
   in every one of them:

   ```
   python .claude/skills/prd/scripts/prd_time.py <prd.md> [<prd.en.md> ...]
   ```

   Where there is no `python` command (most Linux systems), run it with `python3`.

   It reads this session's Claude Code history (`CLAUDE_CODE_SESSION_ID`) and splits the time into agent
   work and the person's time; a wait for the person longer than 10 minutes (`--idle-minutes`) counts only
   10 and the rest is discarded as idle. Rerun it after any later edit in the same session: it replaces this
   session's row. A revision in another session adds a row and a total. To measure a past session, pass
   `--session <id>` (the file name of its history under `~/.claude/projects/`). On exit code 2, report the
   error and leave the section out.
8. When the user authorizes implementation, hand the PRD path and its ancestry to the flow the user chose; that
   flow reads every PRD of the ancestry up to the root before its first change, and creates its own documents,
   if it has any. For ODD, its feature document takes from the PRD:

   - the acceptance criteria, validations, messages, output and examples, quoted verbatim in its `S#`;
   - `Out of scope` as the limit of its specs: nothing outside it gets a task;
   - `Left to the implementation` as the only points its decisions may settle without asking;
   - `Starting state` as the first evidence its exploration checks against the code.

## Subagents

Pass `model` and `effort` on every launch; they match the project's subagent model table (in this project,
`CLAUDE.md`). Run each one in the foreground.

| Role | When | `model` | `effort` | Input | Returns |
|---|---|---|---|---|---|
| Explorer (read-only) | Step 2, only when the starting state exceeds the inline evidence budget | `opus` | `medium` | The questions the `Starting state` section must answer | At most ~2k tokens of facts about code **and data**, each with `path:line` or the query that proves it |
| Reviewer (read-only) | Step 4, on every draft before the user sees it | `sonnet` | `high` | The draft, every PRD of its ancestry up to the root and the project docs it relies on; not the conversation | Findings with the section they affect: acceptance criteria that cannot become a test, contradictions with any PRD of the ancestry that the draft does not declare as an override, rules the docs already state, implementation details that leaked in, gaps in `Out of scope`; and the gap sweep (below) |
| Translator | Step 6, once per translation | `sonnet` | `medium` | The final PRD and an earlier translation as a terminology reference | The translated file, with the original's headings, criterion IDs and CLI messages kept verbatim |

The reviewer gets a fresh context and a model other than the writer's so it does not read what the writer meant
to write. Its findings never change product rules on their own: the user decides those.

**Gap sweep.** The reviewer imagines two people implementing the PRD independently and lists every point where
their results could differ in something the user can observe, each with the two or more readings it allows.
It checks at least:

- the exact keys, shape and wording of every public output, including empty and edge cases, and whether
  existing output keeps its shape;
- the message for every input the change can receive, including existing values the PRD does not name;
- how the change combines with the options and behavior that already exist.

Points already listed in `Left to the implementation` are not gaps.

## Output Contract

Return the PRD path, the open product questions (if any), the reviewer's findings and how each was resolved,
the elaboration time (active, agent and person, discarded idle time) and whether it is ready for
implementation.

## References

- `assets/prd-template.md` — PRD skeleton with per-section guidance.
- `scripts/prd_time.py` — measures the elaboration time from the session history and writes it in the PRD.
  Tests: `python -m unittest discover -s .claude/skills/prd/scripts`.
