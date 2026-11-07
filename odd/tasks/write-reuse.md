# Reused write resources: candidates 3, 4 and 5 of the writes diagnosis

Objective: candidates 3, 4 and 5 of [the performance plan](../../docs/performance/plan.md), section *Writes after
O5–O7*. Branch `perf/write-reuse` from `perf/writes-diagnosis` `ca19201`. Delivery: single branch, unmerged.
Test runner: `cargo test`. No PRD: none of the three changes anything observable (K13, annotation of 2026-11-04).

## Specs

- **S1** (plan, candidate 3, verbatim): "Roll the journal file off the commit (create the next file and close the old
  one on the journal thread): 5–12 % on Windows. No observable change, if the new file is still durable before its
  first write."
- **S2** (plan, candidate 4, verbatim): "Reuse the commit's buffers: compress straight into a pooled aligned record
  buffer, stop trimming at 8 MiB, and pool page payloads instead of zeroing them. Estimated 8–15 % on Windows, 3–7 %
  on WSL 2. No observable change."
- **S3** (plan, candidate 5, verbatim): "Reuse own temporary-area chunks for value runs: about 45 % of W17 on Windows
  and 18 % on WSL 2. No observable change."
- **S4** (invariants, from O4 and O5): the journal record bytes and the data file bytes stay the same; O4's golden
  record hashes pin the record bytes. A failure at any stage leaves the store as today; the fault points the tests
  use stay where they are.
- **S5** (acceptance, K13 annotations of 2026-11-04 and 2026-11-06): single-load runs before and after, on Windows
  and WSL 2, measure what improves; no other load gets worse (S5 rule: lower in both rounds).

## Tasks

| ID | Specs | Route | Status | Evidence |
|---|---|---|---|---|
| T1 | S1, S4 | inline: the journal thread closes the old file | pending | |
| T2 | S2, S4 | inline: pooled record buffers, compression straight into them, larger retention | pending | |
| T3 | S2, S4 | inline: pooled page payloads in the write transaction | pending | |
| T4 | S3, S4 | inline: released own chunks kept for reuse | pending | |
| T5 | S5 | inline: single-load runs before and after, Windows and WSL 2 | pending | |
| T6 | all | RDD per slice, Codex code review, plan update, closing report | pending | |

## Log

- **L1** (2026-11-07): user's request, verbatim. To "¿Arrancamos por estas tres?" (candidates 3, 4 and 5 of the
  diagnosis) the user answered "Sí"; to the estimate "Entre 4 h y 7 h. ¿Arranco?", "Sí". Start: 2026-11-07
  11:28:14 UTC.
- **L2** (2026-11-07): routes. All inline: the four units share no writer reason (they are sequential, each well
  under several minutes of writing, and this session is not at the backstop). No unit names a high-risk item a
  priori: no format change (S4), no public API change; item 4 (concurrency) is checked per unit, since T1 and T2
  move work to the journal thread.
- **L3** (2026-11-07): defaults taken without asking (CLAUDE.md, *Questions first*):
  - T1 moves only the close of the old file to the journal thread. The next file is still created, sized and made
    durable on the committing thread: that is 1–2 % on Windows, inside the noise, and a file created ahead of time
    would show up in the store's folder and in recovery before any record needs it.
  - T2 and T3 keep retained memory bounded: buffers above 32 MiB are given back, as those above 8 MiB were. 32 MiB
    covers W17's commits (256 values of 64 KiB, about 17 MiB of record), which today allocate and free every time.
  - T4 reuses a released own chunk only for a run of exactly its size, and keeps at most 32 MiB of them.
