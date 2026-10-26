---
name: codex-review
description: "Trigger: revisión con Codex, revisión cruzada, que lo revise Codex, Codex review, cross review. Run the Claude–Codex cross review of a PRD or a finished change: Codex reviews, Claude fixes or dismisses with evidence, Codex verifies, up to three rounds."
license: MIT
metadata:
  author: inercya
  version: "1.0"
---

# Codex review

Claude drives the cross review; Codex only reviews. Codex's side (role, skills, report format, models, delegation)
is `AGENTS.md` and `.agents/skills/review-*`; this skill does not restate or change it. Each Codex run goes
through `scripts/Invoke-CodexReview.ps1`, which checks the setup, runs `codex exec`, verifies that Codex changed
nothing but its new report, and prints a JSON result.

Use it when the user asks for a Codex review: of a PRD once it is written, or of a change once its checks pass
(before the closing report). It adds to RDD; it does not replace it.

## Run one review

Run it in the background (`run_in_background`; a critical review can take 15 minutes or more), chained with the
phase log when a run keeps one:

```text
python -I .claude/skills/closing-report/scripts/phase_times.py bg-start <log> review "codex-R<round>" ;
pwsh -NoProfile -File .claude/skills/codex-review/scripts/Invoke-CodexReview.ps1 <target> ;
python -I .claude/skills/closing-report/scripts/phase_times.py bg-end <log> review "codex-R<round>"
```

| Target | Arguments |
|---|---|
| PRD | `-Prd docs/prd/krummi/<name>.prd.md -Feature <slug>` |
| Code | `-Range <base>..<head> -Feature <slug>` |
| Follow-up of a PRD review | `-FollowUp docs/reviews/<slug>/<date>-prd.md` (always the chain's original report), plus `-FixRange <base>..<head>` when the fixes are committed |
| Follow-up of a code review | `-FollowUp docs/reviews/<slug>/<date>-code.md -FixRange <last reviewed head>..HEAD` |

`<slug>` is the feature's PRD or task slug. Every run uses the `workspace-write` sandbox: Codex needs it to run
the cargo checks and to save its report.

Read the printed JSON:

| `status` | Meaning | Next |
|---|---|---|
| `approved` | No finding open in scope | Record it; the loop ends |
| `findings` | `openFindings` lists open IDs; `openQuestions` lists product questions | Handle them (below) |
| `error` | Codex failed, timed out, gave no valid result or saved no report. **Never an approval** | Stop; report `message` and `logDir` |
| `violation` | The repository changed beyond the new report (`unexpectedChanges`); nothing was reverted | Stop; report the paths, revert nothing |

A preflight `error` says what to fix: update the CLI (`npm install -g @openai/codex`), or trust the folder in
Codex when it does not load `.codex/config.toml` (worktree copies are not trusted by default).

## The loop

At most three rounds (one review and two follow-ups) unless the user sets another limit.

1. **Round 1, independent.** Run the review before reading or discussing anything with Codex. Pass only the
   target; never add context, rationale or summaries, and do not edit the prompt the wrapper builds.
2. **Handle each open finding.** Verify it against the code or the PRD first: reproduce it with a failing test,
   or check the cited text. Then exactly one of:
   - **Demonstrated defect: fix it.** Code: test first, the checks from `CLAUDE.md`, one commit per fix (RDD as
     usual). PRD: fix only consistency, evidence and wording.
   - **Technically wrong: do not change anything.** Record `dismissed with evidence` with a justification Codex
     can check: a command and its output, `path:line`, a test name.
   - **Product decision: leave it `pending`.** In a PRD, anything that changes behavior, messages or scope; any
     open question. Do not edit the requirement; keep it for the user.
3. **Record the round** in the chain's resolutions file (below).
4. **Follow-up.** Run the follow-up target. Codex checks only the pending findings, the dismissals and the
   regressions of the fixes. Append its outcome to the resolutions file.
5. **Repeat** from step 2 while findings remain and rounds are left.

Stop and report as **Needs your decision**:

- The round limit was reached with open findings: list them with their reports.
- A dismissal Codex still rejects after verifying it, when the point matters: do not dismiss it again. Give the
  user both sides: Codex's finding and rebuttal, quoted from the reports, and your evidence.
- `pending` product questions: ask them one at a time, after the loop.

Never edit a report or Codex's verdict, change a product requirement without the user, merge or push.

## Traceability

All rounds of a chain share the original report's `<report-stem>-resolutions.md`, append-only, in the format of
the existing ones (`docs/reviews/trees-scan/2026-10-26-prd-resolutions.md`):

- `## <date>`: artifact versions (fix commits, or SHA-256 of uncommitted PRD bytes), then one `### R<n> —
  <severity>: <title>` per finding with *Disposition* (`fixed`, `dismissed with evidence`, `pending`), *Fix* or
  *Evidence*, and *Status: claimed, unverified until a follow-up review*.
- A *Review record* line per Codex run: report path, wrapper status, `threadId`, and the real model/effort from
  `telemetry` (coordinator and each reviewer role). When telemetry is unavailable, say so; never state the
  requested settings as the executed ones.
- `## <date> (verification)` after each follow-up: its report and the per-ID outcome, quoted.

In an ODD run, add one line per round to the execution document's `## Log` (report, status, findings → dispositions,
fix commits). The closing report links the reports and the resolutions.
