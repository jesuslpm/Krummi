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
| T2 | S4, S5 | inline | pending | |

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
