# Turn handoff — ODD execution document

Objective: releasing the write turn wakes only the request that gets it, so commits from many threads stop paying
for wakeups that cannot proceed (performance, no product change, no PRD).
Branch: `perf/turn-handoff`. Delivery: `ask-on-risk`. Test runner: `cargo test --workspace` (Windows).
Estimate: 40 min – 1 h 20 min. Real start: 2026-10-23 22:21:55 +0100.

## Specs

- **S1 — Wake only the next one.** "despertar solo al siguiente de la cola, con un aviso por hilo en vez de
  `notify_all`". A request that leaves the queue by its time limit passes the turn on when the turn is free; when
  the store starts refusing writes, every waiting request still fails at once (T12).
- **S2 — Unchanged rules.** Order of arrival (T3), time limits (T4), the misuse and I/O classes and their
  precedence (T5, T12, T13) stay as they are; the existing queue tests keep passing.
- **S3 — Measured.** `commit_rate` before and after; expected: 16 threads close to 4 threads (about 30,000
  commits/s), not more, because the exclusive turn still limits a group to about 2 commits.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1, S2, S3 | inline | done | see `git log` |

Route evidence: one function pair in `crates/krummi-tx/src/lib.rs` (`wait_turn`, `release_turn`) and its tests;
understood; no Writer trigger reason. Risk: item 4 (concurrency: condvar handoff of the write turn): independent
verifier.

## Log

- **L1** (2026-10-23, user, verbatim): "Por qué con 16 hilos es peor que con 1?" — measured in a scratch clone
  (3 runs of `commit_rate`, 32 cores): 1 / 4 / 16 threads gave about 15,300 / 29,900 / 15,300 commits/s, 15,300 /
  15,000 / 7,100 journal writes/s, 1.00 / 2.00 / 2.15 commits per write, 0 / 0.6 / 18–24 turn wakeups per commit,
  1.00 / 2.0 / 1.6 durable wakeups per commit. Proposal "despertar solo al siguiente de la cola" answered "Sí";
  estimate 40 min – 1 h 20 min, answered "Sí".
- **L2** (T1 evidence): RED: `releasing_the_turn_wakes_only_the_request_that_gets_it` counted 36, 29 and 24
  wakeups for 8 handoffs; GREEN after the change. One run of the queue tests during the first loop ended with
  "error: test failed" and no panic or result line printed (the process probably ended abnormally); it did not
  reproduce in 8 sequential runs, 60 runs of the test binary, and 20 rounds of 4 parallel copies. Recorded as
  unexplained; handed to the verifier. Suite 198 passed in both journal write modes; fmt and clippy clean.
- **L3** (S3 measurement, release `commit_rate`, base binary built from `8704e27` and fixed binary run alternately,
  3 s per thread count). Direct journal writes, 3 rounds, commits/s base / fixed: 1 thread 13,425 / 16,166,
  14,528 / 14,942, 15,007 / 14,641 (means 14,320 / 15,250); 4 threads 25,017 / 29,122, 30,241 / 30,105, 30,844 /
  30,620 (means 28,701 / 29,949); 16 threads 12,201 / 45,745, 14,377 / 44,914, 13,476 / 48,135 (means 13,351 /
  46,265, ×3.5). The plan expected 16 threads near 4 threads; the result is higher: the 2 commits per journal write
  were a symptom of the slow handoff, not a limit of the exclusive turn. Buffered journal writes
  (`KRUMMI_JOURNAL_BUFFERED=1`), 6 rounds, means base / fixed: 352 / 345, 754 / 720, 3,140 / 3,079 (samples
  overlap: within noise).
- **L4** (independent verifier, `b0e3066` over `8704e27`; asked to wrap up after about 30 min, unfinished items
  listed): no blockers; S1–S3 met. Code review found no lost wakeup or stuck request (release against timeout,
  arrival during the handoff hook, refusal while waiters wake, `begin_write` error path, poisoned mutex). Probes on
  branch and baseline: 12 waiters all fail with the I/O class within 500 ms of a refusal (20 rounds); arrival
  order with expiring limits (40 iterations, 3 × 8 parallel copies); 24 threads with 0–3 ms limits, no overlap or
  hang. Queue tests: 30 cargo runs, 10 × 8 and 15 × 40 parallel copies on the branch, 15 × 40 on the baseline, no
  failure. Suite 198 passed in both modes. `commit_rate` with 16 threads, rounds 2–6, base / branch: 10,145 /
  47,474; 13,991 / 43,144; 11,142 / 40,266; 13,620 / 47,009; 13,152 / 47,950. The L2 event was not reproduced
  (about 700 branch and 600 baseline runs). Not done: a probe for a failure inside `begin_write`, a buffered
  measurement (done in L3), more L2 hunting. Advisory: the new test's limit of 12 wakeups counts spurious ones too.
- **L5** (RDD review, 2026-10-23): commits `b0e3066`, `6b0a2c3` against `8704e27`; lenses resilience, reliability;
  approved, acknowledged (lineage `review-21b8329fa80ad2c4`). Findings: R3-timeout-handoff-untested (WARNING): no
  test exercises the timeout path that passes the turn on. Analysis: the branch is never needed (corrected on 2026-10-24: it can run, see turn-timeout-wake.md); under the same
  lock a waiter checks whether it can take the turn before its deadline, so it leaves only when the turn has an
  owner (whose release wakes the next one) or when it is not first (the first was woken by the release). Kept as
  a defensive line; pending note. R4-001 and R3-wakeup-bound-counts-spurious (SUGGESTION): the test's limit of 12
  counts spurious wakeups; margin 4 over 8 handoffs, kept.

## Closing

Closing report: [turn-handoff-report.md](turn-handoff-report.md).
