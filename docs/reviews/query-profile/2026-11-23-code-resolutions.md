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

## 2026-11-23 (verification)

Review record: [2026-11-23-code-02.md](2026-11-23-code-02.md), wrapper status `findings` (R3), `threadId`
`01a20712-406c-7550-9e58-490d11b40dbd`, telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer `krummi-review`
`gpt-6.1-sol` / `high`.

- R1: "verified resolved — Directive changes reach cached plans".
- R2: "verified resolved — Insufficient first-interval evidence remains pending".
- R3: "still open; partially corrected": a finite randomized trial can still split a rare dear parameter value
  unevenly. With P costing 10 / 10,000 and Q 1 / 1,000 on A / B, and one B execution falling on Q's side, Q's mean is
  34.3 against P's 10 and the fixed-count rule forces P (about 5.13 % of assignments for that input stream).

## 2026-11-23 (second fix)

Artifact version: fix commit `385be4a`.

### R3 — major: A short randomized trial can still validate a uniformly worse plan

- Disposition: fixed.
- Fix: *Trial* keeps a count, sum and sum of squares of pages per execution for each side, and decides only with a
  one-sided confidence bound: `z = (m_cand − f·m_good) / √(s²_cand/n_cand + f²·s²_good/n_good)`, computed once each
  side has 30 executions. The trial forces at `z > 1.645` and clears at `z < −1.645`; otherwise it goes on within a
  bound on time, and a trial that reaches the bound is **inconclusive** and forces nothing. A new paragraph, *The
  limits of the guarantee*, gives about 5 % error on each side for means that behave as normal, heavy tails handled
  by the variance, and wrong decisions undone by expiry. Codex's counterexample gives `z = 0.43` (Q: 29 × 1 and
  1 × 1,000, sample variance 33,267; P: 30 × 10), so it continues or ends inconclusive and never forces P. *What it
  never does*, the risk row, experiment 9 and the thresholds of question 8 follow. The factor is named `f`, so it is
  not confused with the histogram's `k`.
- Status: claimed, unverified until a follow-up review.

## 2026-11-23 (verification 2)

Review record: [2026-11-23-code-03.md](2026-11-23-code-03.md), wrapper status `findings` (R3), `threadId`
`01a20716-934e-7ce3-8a41-a42a7589ed19`, telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer `krummi-review`
`gpt-6.1-sol` / `high`. R1 and R2: "previously verified resolved; no regression identified".

- R3: "still open; partially corrected". Two points:
  - With P costing 10 / 1,000,000 and Q 1 / 100,000, three B executions among 200 that all fall on Q's side
    (probability 1/8) give `z ≥ 1.72` and force P.
  - Repeated evaluation at a fixed ±1.645 inflates the error: 14.80 % false clearance with ten flushes and 18.64 %
    with twenty, against 5.20 % at one evaluation.

  Codex notes that "Choosing an inference mechanism or a risk budget belongs to product design".

## 2026-11-23 (third fix)

Product decision, asked after the round: the user chose "Automático, límites explícitos (Recomendado)" over "Solo
recomendación" for forcing the last good plan.

Artifact version: fix commit `ce88517`.

### R3 — major: The trial's confidence rule remains miscalibrated

- Disposition: fixed, within the risk budget the user accepted.
- Fix: *Trial* now has three rules:
  1. one evaluation, at a size fixed in advance (100 executions per side), with an unreached size inconclusive, so the
     sequential problem disappears;
  2. strata by the power of two of the rows estimated for the shape's whole filter, computed before the draw and the
     same for both plans, with post-stratified means and variances weighted by the pooled share of each stratum,
     sparse strata left out, and the trial inconclusive past 5 % of them;
  3. the one-sided bound on the stratified means.

  A trial that is not conclusive forces nothing, and the candidate is no longer promoted to last good on a negative
  `z`, which removes the false-clearance path.

  *The limits of the guarantee* states:
  - about 5 % for means that behave as normal within strata;
  - an error this rule does not bound when the estimate does not separate the dear values from the cheap ones,
    with Codex's 1/8 example;
  - a wrong forcing lasting at most the one-day time to live, visible in the profile and `explain`.

  Experiment 9, the risk row, the costs, the thresholds of question 8 and the 2026-11-23 review note follow.
- Status: claimed, unverified until a follow-up review.
