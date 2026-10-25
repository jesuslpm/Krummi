# Krummi — instructions for review agents

Rust embedded storage engine inspired by Voron, the storage engine of RavenDB. See `README.md` for the layout.

Claude Code writes the PRDs, the code, the ODD documents and the closing reports of this repository. This file is
for agents that **only review** that work, such as Codex. They know the method so they can judge its output, but
they do not follow it themselves.

## Your role

- **Read-only.** Never edit, create or delete files, never commit, never run a worktree script, never change git
  state. The only output is your findings.
- **Two kinds of review.**
  - A PRD: use the `review-prd` skill (`.agents/skills/review-prd/SKILL.md`).
  - Code, meaning a diff, a branch or a commit range: use the `review-code` skill (`.agents/skills/review-code/SKILL.md`).
- **Findings, not rewrites.** Each finding names where it is (`path:line`, or the PRD section and criterion ID),
  what is wrong, the evidence, and a severity. A suggestion goes in one line and never becomes a patch.
- **Product decisions belong to the user.** When the PRD does not settle a point, report it as an open question.
  Do not pick an answer for the user.
- Reply in the language of the request. Quote the PRD's Spanish text verbatim when you cite it.

## How Claude works here: the rules it follows

`CLAUDE.md` is the authority. Read it before any review. It sets:

- **PRD first.** Every product change starts with a PRD. The Spanish PRD is authoritative, and `*.prd.en.md` is a
  translation of it. The execution comes afterwards: ODD by default, or vibe coding or SDD when the user asks for
  one.
- **Engineering principles.**
  - Balance simplicity, efficiency and maintainability.
  - DRY over WET.
  - No premature abstractions.
  - Performance by design, optimization by measurement.
  - Never trade correctness for any of the above.
- **Checks.** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test --workspace`.
- **Repository rules.** Code, identifiers, comments and technical docs are in English. CLI messages and PRDs are
  in Spanish.
- **Commits.** One commit per task on a feature branch, with Conventional Commits and no AI attribution.

The skills in `.claude/skills/` are how Claude produces each artifact. Treat them as the specification of that
artifact, not as instructions for you.

| Artifact | Where it lives | What defines it |
|---|---|---|
| PRD | `docs/prd/krummi/<name>.prd.md` (Spanish, authoritative) and `.prd.en.md` | `.claude/skills/prd/SKILL.md`, template `.claude/skills/prd/assets/prd-template.md` |
| ODD execution document | `odd/tasks/<feature>.md` | `CLAUDE.md` and the ODD protocol (summary below) |
| Phase log | `odd/tasks/<feature>-phases.tsv` | `.claude/skills/closing-report/SKILL.md` (*Phase log*) and `scripts/phase_times.py` |
| Closing report | `odd/tasks/<feature>-report.md` | `.claude/skills/closing-report/SKILL.md`, template `.claude/skills/closing-report/assets/closing-report-template.md` |
| Vibe coding run | No execution document. The closing report holds the criterion → test → result table | `.claude/skills/vibe-coding/SKILL.md` |
| Worktree copies | `C:\wt\krummi-<n>` on `feat/<n>` | `.claude/skills/worktree/SKILL.md`, `docs/worktrees.md` |

### PRDs

- **The PRD tree.** Each PRD names its **Parent document**. `krummi.prd.md` is the root, and a child PRD
  inherits every rule of its ancestry unless it declares an override.
- **Sections.** These come from the template:
  - Objective and context, with a *Starting state* checked against the code;
  - Actors;
  - Flow;
  - Validations and messages;
  - Output;
  - Reuse;
  - Out of scope;
  - Left to the implementation;
  - Acceptance criteria;
  - References;
  - Elaboration time.
- **IDs.** Decisions carry IDs such as `A12` or `F3`. Acceptance criteria carry IDs such as `AC-B5` and must be
  testable.

### ODD execution documents (`odd/tasks/<feature>.md`)

- **Header.** The objective (the PRD it implements and its ancestry), the branch, the delivery strategy and the
  test runner.
- **`## Specs`.** Numbered `S1..Sn`. They quote the PRD's criteria, messages and examples verbatim, and add no
  requirement the user did not ask for.
- **`## Tasks`.** One line per task, with the linked `S#`, the route (inline or delegated), the status and the
  commit.
- **`## Log`.** `L1` is the user's original request, verbatim. After it come user corrections, evidence,
  decisions, RDD review records (commits, lenses, findings, lineage) and the next step.
- **Test-first rule.** One test per rule, through the public interface, with RED observed before GREEN. Every
  existing behavior that shares the changed code gets a test showing it still works.

### Methodology

The background is in `docs/methodology/`:

- `desarrollo-asistido-por-ia.md` covers the whole method: PRD, vibe coding, ODD and SDD, skills, hooks and
  memory.
- `fases.md` covers the ODD phases.
- `generacion-del-prd.md` covers how a PRD is produced.
- `agentes-y-modelos.md` covers the agents and models.
- `parallel-developing.md` and `docs/worktrees.md` cover parallel work in worktrees.
- RDD (receipt-driven development, through `gentle-ai review`) is the outside review Claude runs on each commit.
  Your review adds to it and does not replace it.
