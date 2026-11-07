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
| T1 | S1, S4 | inline: the journal thread closes the old file | done | `f4e0e8a`, `95f3ff0`, L4, L5 |
| T2 | S2, S4 | inline: pooled record buffers, compression straight into them, larger retention | done | `33a10d0`, L6 |
| T3 | S2, S4 | inline: pooled page payloads in the write transaction | done | `20ceb1e`, L7 |
| T4 | S3, S4 | inline: released own chunks kept for reuse | done | `f63507f`, L8 |
| T5 | S5 | inline: single-load runs before and after, Windows and WSL 2 | done | L11 |
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
- **L4** (2026-11-07): T1, commit `f4e0e8a`. A roll hands the old file to `State::retired`; the writer takes the
  retired files with its group and drops them outside the lock; `close` drains them. RED: the committing thread
  closed 2 files in `the_journal_thread_closes_the_file_a_commit_rolls_away_from`; GREEN after. Risk: item 4
  (concurrency: the writer's wake-up loop and where a file handle is dropped).
- **L5** (2026-11-07): RDD review of T1 (`ca19201..f4e0e8a`), lenses resilience and reliability, approved, lineage
  `review-7d830f865fc9696e`, acknowledged. Findings:
  - R4-001 (warning) and R3-retired-wakeup-dependency (warning): retired files rely on the enqueue's notify. No
    failure path follows a successful roll in `enqueue`, but the roll now notifies itself. Fixed in `95f3ff0`.
  - R4-002 (suggestion): `close` kept retired files open across the last group's write. Fixed in `95f3ff0`.
  - R3-closing-test-only-negative (warning): the test checked only the negative. It now checks that three files
    were rolled and that none is left to close. Fixed in `95f3ff0`.
- **L6** (2026-11-07): T2, commit `33a10d0`. `AlignedBuf` has a capacity and `set_len`; every byte of the capacity
  stays initialized. A commit compresses straight into a spare record buffer (smallest that fits) or a zeroed new
  one; the writer gives records back once their group is durable. The `compressed` buffer is gone. Retention:
  32 MiB for the payload buffer and for the spare records in all. O4's golden record hashes pass with records
  built in reused buffers. Test `a_record_is_built_in_the_buffer_the_writer_gave_back` (new API, so RED was a
  compile failure). Assessment: medium, under budget.
- **L7** (2026-11-07): T3, commit `20ceb1e`. The `Engine` keeps up to 32 MiB of page payloads of earlier commits
  (store on disk only), and `copy_for_write` takes one through `krummi_pager::reuse_payload`, which keeps the
  `Op::Alloc` fault point. `add_zeroed` still allocates zeroed payloads. RED: `spare_payloads` was 0 after a commit;
  GREEN after. Golden bytes of the commit (`bytes_tests`) pass. Assessment: medium, under budget.
- **L8** (2026-11-07): T4, commit `f63507f`. Released own chunks of a store on disk are kept, all of one size and at
  most 32 MiB, and a run of that size takes one without creating a file. Deviation from L3, found by the tests:
  `runs.rs` checks that the files of `krummi.temp` count against the maximum size (P10, T14), so chunks are kept
  only without a maximum size, which is the benchmark's case. The R1 test of the large values review now expects
  the last own chunk kept on disk. RED: 2 new tests failed; GREEN after; 568 workspace tests pass. Assessment:
  medium, under budget.
- **L9** (2026-11-07): RDD review of T2–T4 (`f4e0e8a..f63507f`), lenses risk, resilience and reliability,
  approved, lineage `review-e90cf5ae517e6501`, acknowledged. Findings:
  - R3-padding-tail (warning): reused buffers zero only up to `record_len()`, and `set_len` rounds up to
    `DIRECT_ALIGN`. Dismissed with evidence: `record_len()` is a multiple of `ALIGN` (4096), and
    `journal.rs:276` asserts at compile time that `ALIGN` is a multiple of `DIRECT_ALIGN` (4096,
    `sys.rs:479`), so the rounding adds nothing.
  - R1-001 (suggestion): kept own chunks hold an earlier run's bytes on disk longer. Dismissed: freed slots of
    shared chunks already do, nothing reads them, and the next run overwrites the whole chunk.
  - R4-001 (suggestion): a store on disk can now retain up to 32 MiB each of payload buffer, spare records, page
    payloads and kept own chunks, where it retained about 16 MiB. Accepted as the cost of the change; recorded in
    the plan.
  - R3-queue-capacity (suggestion): a small record can take a larger spare buffer, so a full queue may hold more
    than its 8 MiB limit in capacity, by at most the 32 MiB retained. Accepted: bounded by the retention.
- **L10** (2026-11-07): independent verifier of T1 (risk item 4), sonnet at high effort, on a scratch copy of
  `95f3ff0`: S1 pass, S4 pass, no blockers. It ran the `krummi-pager` and `krummi-tx` suites, also with
  `KRUMMI_JOURNAL_BUFFERED=1`, `journal` and `recovery` five times each, and six throwaway probes ten times: a paused
  writer still drains retired files; close while paused with queued records; a writer blocked in the sync hook
  piles retired files up and then drains them; a write fault after rolls; a journal dropped without close; removing
  journal files while retired handles are open. Advisories: `remove_files` may now delete a file whose handle the
  writer has not closed yet, harmless with `FILE_SHARE_DELETE` on Windows and unlink on Linux; the test's
  `retired == 0` holds because the writer takes retired files before the next group. Scratch copies removed.
- **L11** (2026-11-07): T5, single-load runs of `ca19201` (before) against `f63507f` (after), two alternated rounds,
  median of three measured repetitions, benchmark power plan, release builds. Change in op/s, round 1 / round 2:

  | Load | Windows | WSL 2 |
  |---|---|---|
  | W1 | +0.3 % / +1.0 % | +0.5 % / −1.8 % (mixed) |
  | W2 | +22.7 % / +25.4 % | +3.3 % / +3.6 % |
  | W3 | +2.1 % / +2.9 % | +0.2 % / +2.9 % |
  | W12 | +0.4 % / +6.0 % | −13.0 % / −9.5 %; repeat −4.1 % / +2.6 % (mixed) |
  | W13 | +2.1 % / +9.1 % | +2.8 % / +0.2 % |
  | W14 | +24.7 % / +25.3 % | −1.8 % / +2.6 % (mixed) |
  | W15 | +26.8 % / +25.8 % | +4.0 % / +3.9 % |
  | W17 | +54.2 % / +76.1 % | +0.8 % / +11.2 % |
  | W19 | +7.9 % / +8.5 % | +2.4 % / +2.6 % |
  | W21 | +16.1 % / +16.9 % | −0.1 % / +3.9 % (mixed) |

  On Windows every load improves in both rounds. On WSL 2 W12 was lower in both rounds, so it was measured again
  with the intermediate commits (`95f3ff0`, `33a10d0`, `20ceb1e`) in two alternated rounds: before 1,688,643 /
  1,675,265 reads/s, after 1,619,578 / 1,719,336, and no intermediate commit lower than before in both rounds. The
  same `before` binary gave 1.63 M to 1.75 M reads/s across the sessions, so the first pair is taken as noise, as
  with W21 in the write-path run. Incident: `wsl.exe` wrote the runs' error output over the start of `wsl.out`, which
  lost round 1 of W1–W14 on WSL 2; that round was rerun inside WSL with its output written there. The first rerun
  failed at once (`wsl.exe` split the command at `;`) and appears in the phase log as a 5-second background task.
  S2's estimate holds on WSL 2 (W2 +3 %, W15 +4 %) and is exceeded on Windows (+23 % to +27 %); S3 on Windows
  W17 +54 % / +76 %, on WSL 2 +1 % / +11 %. No load is lower in both rounds after the repeat (S5).
