# Resolutions — query profile design review of 2026-11-23

Report: [2026-11-23-code.md](2026-11-23-code.md), range `51271f5..c40fd05` (design 15: forcing the last good plan,
CPU time per execution, the tuner's dependency on modules 10 and 12).

## 2026-11-23

Review record: wrapper status `findings` (R1, R2, R3), `threadId` `01a2070d-5872-7302-8516-105dd38feda7`, telemetry:
coordinator `gpt-6.1-sol` / `high`, reviewer `krummi-review` `gpt-6.1-sol` / `high`.

Artifact version: fix commit `402f125`. *Plan regressions* is rewritten as suspicion, trial, forcing, directives and
the plan cache, expiry.

### R1 — major: Directive changes do not reach cached plans

- Disposition: fixed.
- Fix: new paragraph *Directives and the plan cache*. The consumer publishes the directives with a version per shape,
  and each cached plan records the version it was planned with. Installing, changing, dropping or expiring a
  directive increments the version, so the shape's next lookup plans again even when revalidation would keep the
  entry; a running cursor keeps its plan. Estimate directives reach the cache the same way. M2 states the
  requirement.
- Status: claimed, unverified until a follow-up review.

### R2 — major: The first flush can permanently exclude a regressed plan

- Disposition: fixed.
- Fix: *Suspicion*: a plan other than the last good one is a **candidate** that stays pending from one flush to the
  next until it has enough executions, the shape stops running it, or the catalog no longer supports the last good
  plan. Codex's scenario (29 executions before a flush, 1,000 after) reaches the comparison at the second flush. The
  cost line counts one comparison per pending candidate.
- Status: claimed, unverified until a follow-up review.

### R3 — major: Different parameter mixes can validate a uniformly worse forced plan

- Disposition: fixed.
- Fix: a comparison of separate windows is now only a **suspicion**; it never forces a plan. A suspect shape runs a
  **trial**: each execution takes the last good plan or the candidate with probability ½, drawn in the query's thread
  as for sampling, so both plans run the same mix of parameter values at the same moments. The candidate regressed
  only when its mean in the trial exceeds the last good plan's by the factor; otherwise it is cleared and becomes the
  last good plan. In Codex's counterexample, Q costs 1 page against P's 10 on the A-only load (50 against 100 on B), so
  the trial clears Q under either mix. *What it never does*, the risk row, the costs and experiment 9, which now
  includes this counterexample, follow.
- Status: claimed, unverified until a follow-up review.

Checks: documentation only; no code changed.
