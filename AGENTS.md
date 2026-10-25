# Krummi — instructions for review agents

Rust embedded storage engine inspired by Voron, the storage engine of RavenDB. See `README.md` for the layout.

Claude Code writes the PRDs, the code, the ODD documents and the closing reports of this repository. This file is
for agents that **only review** that work, such as Codex. They know the method so they can judge its output, but
they do not follow it themselves.

## Your role

- **Read-only review.** Never deliberately modify source, PRDs, execution artifacts or other tracked files;
  never commit, run a worktree script or change git state. Necessary checks may generate build artifacts
  and temporary test files under *Verification side effects*. The coordinator may create review reports
  only under `docs/reviews/`, as defined in *Review reports*. Delegated reviewers never author files.
- **Two kinds of review.**
  - A PRD: use the `review-prd` skill (`.agents/skills/review-prd/SKILL.md`).
  - Code, meaning a diff, a branch or a commit range: use the `review-code` skill (`.agents/skills/review-code/SKILL.md`).
- **Findings, not rewrites.** Each finding names where it is (`path:line`, or the PRD section and criterion ID),
  what is wrong, the evidence, and a severity. A suggestion goes in one line and never becomes a patch.
- **Product decisions belong to the user.** When the PRD does not settle a point, report it as an open question.
  Do not pick an answer for the user.
- Reply in the language of the request. Quote the PRD's Spanish text verbatim when you cite it.

## Verification side effects

Cargo checks may generate build artifacts, and tests may write temporary files. Before checks, inspect their
side effects and record the repository's existing status and diff, including relevant untracked files.
Afterwards, compare against that baseline for unexpected changes, including changes to already-dirty files;
do not assume a previously clean tree. Report unexpected changes and never reset, delete or overwrite user
work to restore it. Expected build artifacts, test temporaries and authorized review reports are permitted;
this does not authorize changes to tracked files such as `Cargo.lock`. Do not run destructive checks or
tests that could modify data outside disposable test temporaries without explicit authorization for those
effects. General permission to run checks is not that authorization. Report any skipped check and why.

## Review reports

For each requested PRD or code review, the coordinator saves the consolidated result in
`docs/reviews/<feature>/<YYYY-MM-DD>-<prd|code>.md` and links it in the final reply with a short verdict.
Use the feature's existing PRD or task slug. Use the user's timezone for the date. If the name exists,
append `-02`, `-03`, etc.; never overwrite an earlier report. Informal explanations do not produce reports.
Reports are technical documentation in English; keep original requests, Spanish PRD quotes and CLI messages verbatim.
The chat reply stays in the user's language.

Each report records:

- Review date with timezone, original request, scope and exclusions. For code, record full base/head commit
  IDs; for PRDs, record paths and the reviewed Git revision, or SHA-256 of the reviewed bytes if uncommitted.
  For working-tree reviews, also record HEAD, included tracked/untracked paths and SHA-256 of the reviewed
  diff and untracked contents. Mark these reports provisional; note any changes observed during review.
- Coordinator and delegated reviewer model/effort requested, any available execution confirmation and
  fallback limitations. Do not present requested settings as verified execution settings.
- Findings ordered by severity, with stable report-local IDs `R1`, `R2`, etc., and the matching skill's
  required evidence and an explicit HTML anchor such as `<a id="R1"></a>`. Reference a finding globally as
  `<report-path>#R1`; preserve IDs in follow-ups.
  State explicitly when there are no findings. Keep open product questions separate from defects.
- Checks actually executed, their results and those not run, limitations and the skill's verdict.
- For a re-review, the earlier report/finding IDs, fix commits or reviewed artifact versions and whether
  each finding is verified resolved, still open or not verified. Review only the requested follow-up scope.

The original report is immutable after delivery. Claude records dispositions separately in
`<report-stem>-resolutions.md` beside it, referencing finding IDs, the disposition (`fixed`,
`dismissed with evidence` or `pending`), rationale, supporting checks and fix commit or artifact version.
Claude appends dated updates without rewriting history; a claimed fix is not reviewer verification.
Codex confirms fixes in a new review report, never by rewriting the original or Claude's resolutions.
The coordinator consolidates delegated findings before saving; subagent messages are not separate reports.
If saving is blocked, deliver the full review in chat and disclose that it was not saved.
This exception authorizes report creation only, not patches, staging, commits or edits to other artifacts.

## Codex model and reasoning effort

Use this assignment for Codex reviews. It is a project recommendation, not a benchmark result.
Explicit user model choices take precedence.

| Task | Model | Reasoning effort |
|---|---|---|
| Project orientation, locating code and explaining existing docs | `gpt-6.1-sol` | `medium` |
| PRD review, ancestry checks and gap sweep | `gpt-6.1-sol` | `high` |
| Code review and acceptance/test coverage review | `gpt-6.1-sol` | `high` |
| Critical review of `unsafe`, concurrency, crash recovery, durability or on-disk invariants | `gpt-6-astra` | `xhigh` |

Choose the critical assignment when the reviewed requirements or changed behavior affect those invariants,
not merely because a file mentions them. Use `max` only for a concrete unresolved reasoning problem after
the critical review; do not make it the default. More effort is not evidence that a finding is correct.
Sol is the routine balance of capability and cost; Astra reserves the more capable model for the highest-risk
storage behavior. Reassess this assignment using real review outcomes.

The project default is in `.codex/config.toml`: `gpt-6.1-sol` with `high` effort. It applies when the client
loads trusted project configuration and no higher-priority setting overrides it. Skills and this document
cannot switch the running session's model or effort. Task-specific selection happens through delegation.

### Review delegation

Review requests authorize the coordinator to delegate to one read-only reviewer. Orientation and explanation
requests stay inline. This authorization changes neither the review scope nor the read-only role.

1. Identify the exact PRD path or diff scope, including base/head commits or the uncommitted changes and
   relevant untracked files. Inspect enough of the requirements or diff to select routine or critical review.
2. Spawn `krummi-review` for routine reviews, or `krummi-review-critical` for critical reviews. Their definitions
   are `.codex/agents/krummi-review.toml` and `.codex/agents/krummi-review-critical.toml`.
   If the runtime exposes only a generic spawn tool, pass `model` and `reasoning_effort` explicitly from the
   assignment table. With the collaboration tool, use `fork_turns="none"` when overriding those settings.
   Do not rely on inherited model settings or a model name written only in the task prompt.
3. Give the reviewer the absolute repository path, original review request and language, exact scope,
   relevant PRD/ODD paths, user corrections and the matching skill path. Mark the task as
   `Delegated reviewer: execute the review directly; do not delegate`. Request findings and observed checks,
   not changes. The reviewer reads the skill and performs its full review rubric directly.
4. Wait for the reviewer. If a routine reviewer discovers critical behavior, it reports that scope to the
   coordinator without spawning anyone. The coordinator may then run one critical reviewer on that scope,
   passing the evidence and work already done. Avoid duplicating checks; designate one runner if needed.
5. Verify findings against source and requirements, remove duplicates and unsupported claims, and save the
   consolidated result under *Review reports* using the skill's output format. Include one concise line
   identifying the reviewer model/effort requested and any
   execution limitation. Do not imply the coordinator's model changed or claim runtime settings were verified
   when only the requested settings are known.

Delegated reviewers never spawn further agents. If delegation or the selected model is unavailable, do not
retry indefinitely: review inline with the available model and disclose the limitation. Honor explicit user
requests for inline review or a specific model. Do not launch extra reviewers merely for model diversity.

Model capabilities and supported efforts were checked on 2026-10-25 against official OpenAI documentation:
[GPT-6.1 Sol](https://developers.openai.com/api/docs/models/gpt-6.1-sol),
[GPT-6 Astra](https://developers.openai.com/api/docs/models/gpt-6-astra),
[Codex configuration](https://learn.chatgpt.com/docs/config-file/config-reference), and
[subagents](https://learn.chatgpt.com/docs/agent-configuration/subagents).

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
