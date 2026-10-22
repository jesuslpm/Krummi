---
name: vibe-coding
description: "Trigger: vibe coding, usando vibe coding, con vibe coding, implement with vibe coding. Implement a PRD conversationally inside fixed bookends: map the existing code and ask every question before the first change, keep the code DRY while working, and finish with a test per acceptance criterion, verification against the PRD, the RDD review when it is on, and a closing report. No execution document, phases or subagents."
license: MIT
metadata:
  author: inercya
  version: "1.2"
---

## Activation Contract

Load when the user asks to implement something "using vibe coding". The user chose this approach explicitly,
so for this request it replaces the default execution workflow (ODD): do not open an ODD feature document or
any other execution workflow.

This skill fixes the **bookends** of the run, not the process in between: what happens before the first change
and what is delivered at the end. The work in between stays conversational and direct. That is what keeps it
vibe coding instead of a lightweight SDD.

## Hard Rules

- **No process artifacts.** No execution document (`odd/tasks/*` or similar), no task list file, no phases and
  no subagents. Work in the main session.
- **Look before you write.** Map the starting state before the first change (step 2). Never write code for a
  criterion without having searched for what already exists.
- **DRY means one place per piece of knowledge.** Before writing a new function, type, validation, constant or
  message, search for an equivalent and reuse or extend it. When new code would repeat a rule that already
  exists, extract a shared piece instead of copying. Do not merge code that only looks alike but changes for
  different reasons: that is coupling, not DRY.
- **The PRD is the product source.** Never invent a product decision it does not cover. Pick the most
  conservative default that adds no scope, keep going, and list it in the closing report.
- **Questions only before the start.** After the run starts, do not stop to ask; the user may still steer,
  and their messages are the only interruption.
- **Always verify.** Never ask whether to run tests or checks: run them.
- **Outside review with RDD.** RDD (receipt-driven development) runs through `gentle-ai review`. Check it once,
  in step 2, with `gentle-ai review mode status --cwd <repo> --json`; the Decision Gates say what each result
  means. When it is on, keep it as in ODD: after each commit run `gentle-ai review assess --cwd <repo> --agent
  claude-code --base-ref <last reviewed boundary> --committed-only --json`, and when `review_due` is true run
  the review it returns. The first boundary is the branch point; each reviewed commit becomes the next one. At closing, the last
  unreviewed slice is reviewed too (step 11).
  Follow the repository's consent rule (in this project: always granted). RDD works on the git diff and needs
  no execution document. It is the only outside check: no verifier subagents. When RDD is unavailable or off
  there is no outside review: never install or enable it, and record "no outside review" with its reason in
  the closing report. A `gentle-ai review` command that fails mid-run is recorded in the report and never
  counts as a passed review.
- **Every acceptance criterion gets a test.** When a criterion cannot be automated, record the exact manual
  check command and its output as its evidence.
- **Measured times only.** Start and end come from `date`; never write an estimate as if it were a measurement.
  The time by phase comes from the phase log (`closing-report` skill, *Phase log*), marked at each change of
  phase from the first exploration on.
- **No push.** Commits stay local; push only when the user asks.

## Decision Gates

| Situation | Action |
|---|---|
| The PRD leaves a product question open | Ask it before starting, together with every other question |
| Existing code already satisfies a criterion | Write or run its test; implement only what fails |
| Similar code exists for the same rule | Reuse or extend it; extract a shared piece if needed, keeping its tests green |
| Similar-looking code exists for a different rule | Leave it separate and say why in one line |
| A gap appears mid-run | Conservative default with no extra scope; note it for the report; continue |
| A criterion cannot be automated | Manual check command plus its output as evidence |
| The work needs several sessions or touches high risk (persisted data, security, contracts others consume, concurrency, delivery) | Say in one line that ODD or SDD fits better; continue unless the user stops you; flag it in the report |
| The user steers mid-run | Follow the user; record the change in the report |
| `gentle-ai review mode status` is not found (exit 127) or exits non-zero (no `gentle-ai`, or a version without `review`) | RDD unavailable: no outside review; say so in one line with the map |
| It reports `"effective": "off"` | RDD off: no outside review; say so in one line with the map; never enable it |
| It reports `"effective": "on"` | RDD on: assess after each commit (Hard Rules) |

## Execution Steps

### Before the first change

1. Open the phase log with `phase_times.py mark <log> explore` (`closing-report` skill, *Phase log*). Read the
   PRD and its whole ancestry up to the root PRD (`prd` skill) completely, including their **Starting state**
   and **Reuse** sections. List the
   acceptance criteria.
2. **Map the starting state.** For each criterion, search the code for its domain terms, field names, messages
   and similar functions, types or validations. Classify each criterion as *already done*, *partly done*,
   *similar code to reuse* or *new*, with file:line references. Share the map in the conversation in a few
   lines; it is not written to a file. Run the existing tests once to know the baseline, and check RDD with
   `gentle-ai review mode status --cwd <repo> --json` (Decision Gates).
3. Ask every open product question in a single message and wait for the answers. If there are none, say so
   in one line at the end of the map (step 2), so the step 4 message stays clean.
4. Send a message with only a time range for the whole run, calibrated with earlier closing reports when they
   exist, and the question whether to start; nothing else goes in it, so the estimate stands out. Mark the
   wait (`mark <log> wait`) before sending it. On the answer, record the real start time with `date` and mark
   the first phase with `--start`.
5. Create a branch for the change from the integration branch.

### During the run

6. Take one acceptance criterion, or a small slice of one, at a time, starting from what the map says can be
   reused. Write its test first when practical, see it fail, implement, and see it pass.
7. **Refactor before moving on:** remove any duplication the step introduced or exposed, with the tests green.
8. After each working step, run the focused tests and commit (Conventional Commit). A commit is the checkpoint
   to return to if the next step goes wrong. When RDD is on, run its assessment (Hard Rules) and, when a
   review is due, take it to its end before the next step.
9. Keep a running list of defaults chosen, gaps found, reuse decisions and, for each RDD review when it ends,
   its commits, lenses, result, findings and lineage; it goes into the closing report.

### Closing

10. Review the whole diff for duplicated logic, constants or messages, and fix what repeats existing knowledge.
11. **Review the last slice.** When RDD is on and commits remain after the last reviewed boundary (the last
    assessment said `under_budget`), request their review with the command and the generated-files rule in `CLAUDE.md` (Closing) and take
    it to its end, so every merged commit has been through RDD.
12. Run the project's full checks (see `CLAUDE.md`), after the last code change.
13. Check every acceptance criterion against the PRD with executed commands, and build the table criterion →
    test or check → result.
14. Record the end time with `date` and `phase_times.py end <log>`, and compare the real duration with the
    estimate.
15. Write the closing report with the `closing-report` skill, including the acceptance criteria coverage
    table, the reuse map outcome (reused, extended, extracted, new), the RDD reviews and their findings (or
    why there were none), the defaults chosen without asking and the pending items. Commit it.
16. Merge only if the repository rules say so (in this project: into `main` after full verification).

## Output Contract

Return the report path, the real-versus-estimated time, how many acceptance criteria passed, what was reused
or extracted, and the pending items or defaults the user should review.

## References

- `.claude/skills/prd/SKILL.md` — how the PRD this skill implements is written.
- `.claude/skills/closing-report/SKILL.md` — the closing report and its template.
- `docs/methodology/desarrollo-asistido-por-ia.md`, sections 3, 5.4 and 6 — the three rules and the vibe coding
  process this skill encodes.
