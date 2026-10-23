# Transfer pacing — ODD execution document

Objective: make the transfer thread cheaper and simpler without lowering performance (follow-up of
[transfer-background-thread.md](transfer-background-thread.md); refactor and performance, no PRD).
Branch: `refactor/transfer-pacing`. Delivery: `ask-on-risk`. Test runner: `cargo test --workspace` (Windows).
Estimate: 45 min – 1 h 30 min. Real start: 2026-10-23 19:55:31 +0100.

## Specs

- **S1 — Urgency by growth.** "marcar la pasada como urgente solo cuando el área creció 32 MiB desde la pasada
  anterior, no cuando ya está por encima": with a long read holding more than the threshold, commits do not start
  one pass each.
- **S2 — One lock per pass to retire pages.** "Retirarlas todas juntas al final" instead of taking the state lock
  once per page written.
- **S3 — One deadline.** "Un solo plazo en vez de `requested`, `urgent` y `retry_at`: la próxima pasada no antes
  de X", same behavior.
- **S4 — Data fsync interval.** Measure with `commit_rate` with and without the 2 s interval; user (L2): "Sí,
  quitarlo si no empeora (Recommended)" — if it gets worse beyond noise, keep it and record the figures.
- **S5 — No performance loss.** Compare `cargo run --release -p krummi-tx --example commit_rate` before and after.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1, S2, S3, S5 | inline | done | see `git log` |
| T2 | S4, S5 | inline | done | see `git log` |

Route evidence: one module (`crates/krummi-pager/src/transfer.rs`, `scratch.rs`) and its tests; understood; no
Writer trigger reason. Risk: item 4 (concurrency: the transfer thread's wake-up rules): independent verifier.

## Log

- **L1** (2026-10-23, user, verbatim): "Deberíamos reducir los 200 ms o los 32 MiB? Qué sugieres para mejorar el
  rendimiento y quitar complejidad sin reducir rendimiento?" — answer to the proposal of points 1, 2, 4 and the
  measurement of 5: "Sí".
- **L2** (user): "Sí, quitarlo si no empeora (Recommended)". Estimate 45 min – 1 h 30 min, answer "Sí".
- **L3** (T1 evidence): `commit_rate` baseline on `e3ea257` (release, direct journal writes, 3 runs of 3 s,
  commits/s with 1, 4, 16 threads): 14,186 / 29,199 / 11,925; 13,532 / 30,349 / 13,170; 14,850 / 29,264 / 13,001;
  means 14,189 / 29,604 / 12,699. RED for S1: `a_pass_starts_at_once_only_when_the_scratch_area_grew_by_the_
  threshold` gave 50 passes for 50 commits. GREEN after T1; suite 197 passed in both journal write modes; fmt and
  clippy clean. After T1: 15,914 / 31,310 / 12,329; 14,485 / 30,768 / 12,945; 14,623 / 30,826 / 13,960; means
  15,007 / 30,968 / 13,078 (+5.8 %, +4.6 %, +3.0 %).
- **L4** (T2 evidence): temporary measuring build of `commit_rate` (not committed: journal file limit from
  `KRUMMI_FILE_LIMIT`, plus a data flush column), binaries with and without the 2 s interval run alternately, 3 s
  per thread count, direct journal writes. Release file size (16 MiB), 3 rounds, means with / without: 1 thread
  14,960 / 14,309 (−4.4 %), 4 threads 30,435 / 30,273 (−0.5 %), 16 threads 12,959 / 13,713 (+5.8 %). 1 MiB files,
  6 rounds: 12,515 / 12,661 (+1.2 %), 22,078 / 21,152 (−4.2 %), 12,317 / 11,970 (−2.8 %); the samples of each pair
  overlap (spread about ±10 %). Data flushes in 3 s: 2 with the interval (one release and the close), 9 to 20
  without. Decision per L2: no worsening beyond noise, so the interval is removed (`Flushing`, the put-off release
  and the third pacing value go). Suite 197 passed in both journal write modes; fmt and clippy clean.
- **L5** (independent verifier, `0c6af85`..`e6a9fb5` over `54a9141`): no blockers; S1–S4 met, S5 not re-measured
  (consistent with L3/L4). Probes: a read holding about 49 MiB with 200 single-page commits made 1 pass on the
  branch against 200 on the baseline (3 against 196 with buffered journal writes); a failure on the 3rd of 8 page
  writes left 6 pages in the scratch area on both, and reopening gave the right values; concurrent durable and
  deferred writers with readers, close or drop, then reopen: every value equal. Suites 197 passed in both modes,
  transfer tests 10 runs. Advisories: `set_pacing` (tests only) no longer shortens a deadline already pending;
  fault probes that count writes globally can hit a journal write (same on the baseline).
