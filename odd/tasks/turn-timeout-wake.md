# Turn timeout wake — ODD execution document

Objective: remove the line that wakes the next request when a request leaves the write-turn queue by its time
limit; it is never needed (refactor, no product change, no PRD).
Branch: `refactor/turn-timeout-wake`. Delivery: `ask-on-risk`. Test runner: `cargo test --workspace` (Windows).
Estimate: 20 – 40 min. Real start: 2026-10-24 07:03:20 +0100.

## Specs

- **S1 — Remove the redundant wake.** "Borrar esas tres líneas" and leave a comment with the reason: a request that
  leaves never has to wake another, because if it were first with the turn free it would have taken it, and
  otherwise the first one is woken by whoever releases the turn.
- **S2 — Correct the report.** Fix "inalcanzable" in `turn-handoff-report.md`: the line can run (while
  `release_turn` runs the `Handoff` hook) but never does anything useful.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1, S2 | inline | done | see `git log` |

Route evidence: three lines and a comment in `crates/krummi-tx/src/lib.rs`; understood. Risk: item 4 (concurrency:
wake-up of the write turn): a bounded independent verifier (user preference: short verifier runs).

Test-first exception: no rule changes, so there is no meaningful RED. The only observable effect of the line is one
extra wakeup during the `Handoff` hook window. The existing queue tests cover the behavior that must stay: AC-T6 and
AC-T7 (limits that expire), AC-T18 (handoff order) and `releasing_the_turn_wakes_only_the_request_that_gets_it`.

## Log

- **L1** (2026-10-24, user, verbatim): "Qué ocurre con la línea defensiva. ¿Se puede arreglar?" — answer: the line
  is not unreachable as the turn-handoff report said; it is unnecessary; proposal to remove it answered "Sí";
  estimate 20 – 40 min, answered "Sí".
- **L2** (T1 evidence): fmt and clippy clean; queue tests 5 runs, 9 passed each; suite 198 passed in both journal
  write modes. Report corrected (S2) in `turn-handoff-report.md` and its execution document.
- **L3** (independent verifier, bounded to about 10 min): S1 and S2 pass, no blockers. No interleaving leaves a
  waiter asleep with the turn free (timeout in the `Handoff` window, first waiter leaving while owned, woken but not
  yet running, new arrivals, `begin_write` failing). Suite 198 passed; queue tests 10 runs and 8 × 5 parallel
  copies, all passed; probe of 48 threads with 0–5 ms limits, a 3 ms sleep in the `Handoff` hook on half the
  rounds, 3 runs × 30 rounds on disk and in memory: no hang. Advisory: no committed test makes the `Handoff`-window
  timeout deterministic.
