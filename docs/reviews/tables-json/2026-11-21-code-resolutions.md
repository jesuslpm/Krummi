# JSON columns code review resolutions

Resolutions of [2026-11-21-code.md](2026-11-21-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-21

Artifact versions: no fix commits; HEAD reviewed `ba15c45`.

Review record: [2026-11-21-code.md](2026-11-21-code.md), wrapper status `findings` (R1), `threadId`
`01a1fdf8-d6d0-7bb2-80a7-5265573fee4e`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.

### R1 — minor: Sampling rate targets are described as hard limits

- Disposition: dismissed with evidence (outside the scope of this run, not a judgment on the finding's merit).
- Evidence: the cited files, `docs/design/tables/15-query-profile-and-tuning.md` and its `.html` companion, are the
  query-profile design written by another session that commits to the same branch; none of this run's commits touches
  them. `git log --format='%h %s' cf73c36..HEAD -- docs/design/tables/15-query-profile-and-tuning.md
  docs/design/tables/15-query-profile-and-tuning.html` lists only that session's commits: `b9196a0`, `5d7d588`,
  `59190d5`, `5397965`, `47f2d02`, `c28784b`, `31c2f6a`. The run implements `docs/prd/krummi/tables/tables-json.prd.md`
  (spec S1 of `odd/done/tables/tables-json.md`), whose scope does not include document 15; the execution log records these
  commits as kept apart (L6). Editing the document here would also race the session that is writing it. The finding is
  passed on to the user for that design's owner, unchanged.
- Status: claimed out of scope, unverified until a follow-up review.

## 2026-11-21 (verification)

Review record: [2026-11-21-code-02.md](2026-11-21-code-02.md), wrapper status `findings` (R1), `threadId`
`01a1fe20-46cf-79b2-9fe1-ea1acd6d6c5b`. Verdict: "Changes requested: R1 remains open (minor)." No new findings or
correction-induced regressions; no functional JSON defect in either round.

- R1: "still open; dismissal evidence verified in part, but does not justify closing the finding": the separate
  ownership is confirmed, but the reviewed range includes the design commits and the contradiction stands.

The dismissal is not repeated. R1 stays open and goes to the user as a decision in the closing report: it concerns
the query-profile design of the other session (document 15), not the JSON columns.

## 2026-11-23

Artifact version: fix commit `1f38591` (`docs(design): describe the sampling rate limits as expected-rate targets`),
on branch `docs/query-profile-rates` from `main` at `1399d32`. The commits between the last reviewed head (`ba15c45`)
and `1399d32` are two other runs, each with its own Codex code review (`docs/reviews/usage-guides/` and
`docs/reviews/tables-temporal-micros/`); none touches document 15, so the fix range of this follow-up is
`1399d32..1f38591` plus the commit that adds this entry.

### R1 — minor: Sampling rate targets are described as hard limits

- Disposition: fixed (the user decided to correct the text, 2026-11-23; the earlier out-of-scope dismissal is
  withdrawn).
- Fix: `docs/design/tables/15-query-profile-and-tuning.md`:
  - *Costs* no longer says "never more than `R` … nor `B`". It now calls them targets of the expected rate, not hard
    quotas, and names the two overshoots: up to one tick of rows with `p = 1` for an entry not yet in the snapshot
    (about 1,000 at 10,000 executions per second), and `g` following the demand one second late. The memory bound of
    the buffers is the only hard limit.
  - *A cap per entry and second* says that the cap bounds the expected rate, not the count, and that one tick passes
    before it acts.
  - *The global factor `g`* says that it brakes a spike one second late, so within that second the expected demand
    may pass `B`.
  - The companion `15-query-profile-and-tuning.html` names the clipped series "(idealized)". Its legend says that
    `Math.min(p[1], B)` is an idealization and that `g` lets the first second of a spike pass `B`. The cap-R bullet
    says "on average".
- Checks: documentation only; no code changed, so the cargo checks do not apply. `git diff --stat 1399d32..1f38591`:
  2 files, 15 insertions, 7 deletions.
- Status: claimed, unverified until a follow-up review.

## 2026-11-23 (verification)

Review record: [2026-11-23-code.md](2026-11-23-code.md), wrapper status `findings` (R1), `threadId`
`01a206f1-8a36-7413-a8e3-889cd6dc0526`, telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer `krummi-review`
`gpt-6.1-sol` / `high`.

- R1: "partially fixed; still open": the companion page still said that an entry "cannot keep more than R rows per
  second" and that its start is "flattened to R", while its own chart model gives 100 rows/s at `t = 0.01 s` with
  `λ = 100` and `R = 10`.

## 2026-11-23 (second fix)

Artifact version: fix commit `834fb9e`.

### R1 — minor: Sampling rate targets are described as hard limits

- Disposition: fixed (the remaining part, in the companion page).
- Fix: `docs/design/tables/15-query-profile-and-tuning.html`:
  - the `R / c` and `g` bullets call `R` and `B` targets of the expected rate, not hard quotas: within an entry's first
    second it keeps more than `R`, and `g` follows the demand one second late;
  - the green-curve bullet and *What that means in rows* say that the rate falls to about `R` after the first second,
    with about `R(1 + ln(rate/R))` rows within it;
  - *What that means in rows* also says that the engine's one-tick delay with `p = 1` is left out of the charts;
  - the fleet bullets say "falls to about `R` … after its first second" and "stays near the budget".

  A search of both files for "never more", "no more than", "cannot keep", "within budget" and "keeps the whole store
  under" finds none left.
- Checks: documentation only; no code changed.
- Status: claimed, unverified until a follow-up review.

## 2026-11-23 (verification 2)

Review record: [2026-11-23-code-02.md](2026-11-23-code-02.md), wrapper status `violation` (`unexpectedChanges`:
`.atl/skill-registry.md`), `threadId` `01a206f5-6cd6-75e3-b74d-d24a2d35f8ef`, telemetry: coordinator
`gpt-6.1-sol` / `high`, reviewer `krummi-review` `gpt-6.1-sol` / `high`. The user confirmed that the registry change
came from their upgrade of Gentle AI to 5.0.0 during the review (binary installed 05:51:24Z, registry rewritten
05:53:31Z), not from Codex; nothing was reverted.

- R1: "partially fixed; still open because the correction introduces a documentation regression": the new HTML
  statements (more than `R` rows in the first second, `R(1 + ln(rate/R))`, about `R` rows per second after it)
  do not hold for an entry slower than `R`; with a rate of 1 per second and `R = 10` the formula gives −13 rows and
  the chart model keeps 1 row per second.

## 2026-11-23 (third fix)

Artifact version: fix commit `a935565`.

### R1 — minor: Sampling rate targets are described as hard limits

- Disposition: fixed (the startup statements limited to entries that the cap brakes).
- Fix: `docs/design/tables/15-query-profile-and-tuning.html`: the `R / c` bullet says the cap brakes only an
  entry that runs more than `R` times per second, and an entry at `R` or fewer has `R / c ≥ 1`. The green-curve
  bullet gives `min(R, rate·p)` rows per second after the first second and "at most about `R(1 + ln(rate/R))`"
  within it, for those entries only, and says a slower entry follows the orange curve. *What that means in rows* and
  the fleet bullet say the same. In `15-query-profile-and-tuning.md`, *A cap per entry and second* limits its formula
  the same way. The bound follows from the chart model, where the rate is `λ·min(R/c, p(n))` and `c = max(1, λt)`
  within the first second: `∫₀¹ min(λ, R/t) dt = R(1 + ln(λ/R))` for `λ > R`, and `R/c ≥ 1` for `λ ≤ R`.
- Checks: documentation only; no code changed.
- Status: claimed, unverified until a follow-up review.

## 2026-11-23 (verification 3)

Review record: [2026-11-23-code-03.md](2026-11-23-code-03.md), wrapper status `approved`, `threadId`
`01a206fa-5341-7d21-bb5c-88152dcaaeb8`, telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer `krummi-review`
`gpt-6.1-sol` / `high`. Verdict: "Approved in the requested scope: R1 is verified resolved; no open findings or
correction-induced regressions were established."
