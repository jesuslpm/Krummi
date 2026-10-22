# <Outcome in one line, e.g. "Query app finished: verified and merged into main">

Closing report of `<feature or change>` (<YYYY-MM-DD>). Execution approach: <ODD | SDD | vibe coding>.
Product source: <link to the PRD>. Execution document: <link, or "none">.

<One or two lines with the delivery state: branch, merge, commit, whether anything was pushed.>

## Result

- **Specification:** <which specs or acceptance criteria were delivered; tests passing, with the count>.
- **Acceptance:** <acceptance run on real data, with its figures; or "not run" and why>.
- **Reviews and verification:** <independent verifiers, automated reviews and their outcome; RDD details in
  the RDD reviews section>.
- **Findings fixed along the way:** <defects found by tests, verifiers or reviews, and how they were fixed>.

## Acceptance criteria coverage

<Required for vibe coding; optional when the execution document already traces criteria to evidence.>

| Criterion | Test or check | Result |
|---|---|---|
| <AC id> | <test name or command> | <PASS / FAIL / not run, with the figure when there is one> |

## RDD reviews

<One row per RDD review, from the record kept during the run: Gentle AI keeps no findings once a review is
acknowledged. Assessments that started no review (passive, under budget) go in one line below the table. If
RDD was unavailable or off, write "No outside review" and the reason instead of the table. RDD reviews the diff
through its lenses; it does not check the PRD, which is the job of the acceptance criteria coverage above.>

| Commits | Lenses | Result | Findings and resolution | Lineage |
|---|---|---|---|---|
| <base..head, short SHAs> | <risk, resilience, readability, reliability> | <approved / corrected, then approved / stopped: reason code> | <each finding: severity, one line, how it was resolved; or "none"> | <review-…> |

## Reuse

<Required for vibe coding; optional otherwise. What the change reused, extended or extracted from existing
code, and what had to be written new, with file references. One line for any similar-looking code left
separate on purpose, and why.>

## Time

- Estimate: <range given before starting>. Real start: <date>. End: <date>.
- **Active duration since the start: <h min>**, against the estimate: <deviation and the phases that caused it>.
- Waits for the user: <none | each wait with start, end and reason>.

### Time by phase

<Paste the output of `scripts/phase_times.py report <log> --commits <base>..HEAD`: the phase table, the
start, waits and pauses lines, the background table (acceptance runs and other tasks that overlapped the
foreground) and the time by commit (message, task labels, active time and its phases). If the run kept no
phase log, write "Not measured by phase" and why.>

<One or two lines reading the table: which phases took most of the time and why (for example, a third round
of acceptance runs after a blocker).>

## Change report (`<base>..<head>`)

<Paste the table produced by `scripts/numstat_by_area.py` (with its Code + / Comments + columns and their
legend), then the lockfiles/generated line.>

<One line comparing with the forecast, if a line forecast existed.>

## Summary

<One paragraph: what the change does for the user, in product terms, and the key technical choices.>

## Defaults chosen without asking

<Decisions the agent took with a reasonable default instead of interrupting the user; or "none".>

## Pending (non-blocking)

<Known limits, advisory findings left open, follow-up work. Each item states why it is not blocking.>

**Risk:** <item numbers and reason, or "none">.

## Manual cleanup

<Temporary folders or files the user may delete, with sizes; or remove this section if there are none.>
