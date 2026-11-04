# Scratch lookup without a lock (O2)

Objective: O2 of [the performance plan](../../docs/performance/plan.md): a read snapshot skips the scratch lock when
no version can apply to it. Branch `perf/scratch-lock` from `main` `b3c0c44`. Delivery: single branch, unmerged.
Test runner: `cargo test`. No PRD: an optimization without an observable change (K13 annotation of 2026-11-04).

## Specs

- **S1** (plan, O2, verbatim): "`Snapshot::page_and_code` locks the scratch `State` and looks up `versions` on every
  page access, even when the map is empty (all the read-only loads). Decide without the lock whether any version can
  apply to this snapshot, or make the lookup lock-free."
- **S2** (plan, O2, targets, verbatim): "W7–W9, W11, W18, W22; W12 needs the lookup to stay correct while the writer
  publishes versions."
- **S3** (acceptance, as for O1, L2 of [pager-mapping.md](pager-mapping.md)): single-load worker runs of W7–W12
  before and after, at full scale, on Windows and WSL 2; the complete runs of the plan's *Accepting an optimization*
  run once after O1, O2 and O3a.
- **S4** (plan, O2, risk, verbatim): "concurrency (item 4): verifier and own review."

## Tasks

| ID | Specs | Route | Status | Evidence |
|---|---|---|---|---|
| T1 | S3 | inline: baseline single-load runs at `b3c0c44`, Windows and WSL | done | L4 |
| T2 | S1, S2 | inline: decide at snapshot open whether any version can apply; skip the lock in `page_and_code` and `run` | done | `387e758`, L5 |
| T3 | S2, S3 | inline: single-load runs after T2, compared with T1 | done | L6–L8 |
| T4 | S4 | independent verifier, RDD, Codex, closing | pending | — |

## Log

- **L1** (2026-11-04): user's request, verbatim: "Hagamos O2 entonces".
- **L2** (2026-11-04): estimate 1 h 15 min – 2 h, confirmed ("Adelante"); start 18:45:33 UTC.
- **L3** (2026-11-04): map. `Snapshot::page_and_code` (`scratch.rs:993`) and `Snapshot::run` (`:1033`) lock `State`
  and look up `versions` for every page. A version enters `versions` only in `ScratchWriter::publish` (`:1215`),
  under the lock, with `txid = last_committed + 1`; a snapshot is registered under the same lock with a `txid` of at
  most `last_committed` (`register`, `:523`; `visible`, `:515`), and its `txid` never changes. So a version published
  after a snapshot opened is newer than it and never applies to it; versions only leave `versions` afterwards
  (transfer, retire). Decision (default taken, S1's first option): the snapshot records at open whether `versions`
  is empty; when it is, no version can ever apply to it and it reads the store without the lock. Where `versions` is
  not empty at open (W12's readers beside its writer, the writer's own snapshot) the lookup stays as today, so W12
  keeps its behavior but gains nothing here; a lock-free map is left out. W18 and W22 need the stores of W17 and W21
  and are left to the complete runs (S3).
- **L4** (2026-11-04): T1 baseline at `b3c0c44` (`scratchpad/o1/measure.py`, as in O1; median op/s of repetitions
  1–3): Windows W7 558,820, W8 1,239,325, W9 476,077, W10 19,584,687, W11 15,045,268, W12 538,159, W13 21,420; WSL 2
  W7 500,657, W8 1,078,229, W9 306,985, W10 2,354,959, W11 8,125,466, W12 777,841, W13 7,717. WSL's W9–W11 are noisy
  (W9 repetitions 178,127 to 421,288; W10 far below O1's 19,215,754 for the same code): they get reruns that
  alternate the two binaries (T3).
- **L5** (2026-11-04): T2. `Snapshot` gets `may_see_versions`, set in `register` under the lock to
  `!state.versions.is_empty()`; `page_and_code` and `run` lock `State` only when it is true (`Option` of the guard).
  The existing tests pin both sides; mutation check, not committed: with the field always `false`, 6 scratch tests
  fail (`a_snapshot_sees_what_was_committed_when_it_opened`, `a_run_is_read_only_whole_and_from_one_place`,
  `versions_nobody_can_read_are_retired` among them). No new test: the change adds no rule, and the field is written
  once, under the lock that orders it with `publish`. Checks: fmt ok, workspace clippy clean, `cargo test
  --workspace` 520 passed, 0 failed, 30 ignored.
- **L6** (2026-11-04): T3, first runs at `387e758` (median op/s; Windows run once, then two more rounds alternating
  the binaries): Windows W9 476,077 / 389,770 / 394,097 → 598,624 / 646,945 / 575,018, but W10 19,584,687 /
  19,514,289 / 14,280,349 → 2,697,755 / 2,340,125 / 1,812,117 and W11 15.0 M / 15.0 M / 13.9 M → 6.7 M / 10.6 M / 9.9 M.
  WSL 2 (two rounds): W9 306,985 / 431,963 → 709,969 / 702,301; W10 bimodal with both binaries (2,354,959 /
  10,955,741 → 16,788,218 / 3,577,543).
- **L7** (2026-11-04): the Windows W10 drop is the machine, not the code. W10 alone: before 21,164,738 / 18,895,662,
  after 20,211,125 / 21,918,385 (and the same code with the field always `true`, 10,976,334 / 20,805,679). W9 then
  W10 in one `measure.py` call: before 20.8 M / 19.5 M, after 2.5 M / 2.4 M, also with a 5 s pause between the loads.
  Crossing the binaries under one parent process: after's W9 then before's W10 gave 2,316,164, and before's W9 then
  after's W10 gave 20,828,040. A load that follows one saturating 16 threads under the same parent runs slowly on
  this machine (likely Windows power throttling or E-core scheduling of the process tree, not proven); with the
  scratch lock gone, W9 now saturates them. Comparison redone with each load in its own `measure.py` call,
  alternating the binaries (`MEASURE_PAUSE` added to the script, unused there). The complete runs may show the same
  effect on the loads after W9: noted for the closing report.
- **L8** (2026-11-04): T3, each load in its own `measure.py` call from one background shell, two rounds alternating
  the binaries (median op/s, before → after): W7 497,453 / 403,518 → 472,736 / 458,725 (+3 %); W8 1,056,017 / 988,946
  → 1,038,073 / 1,079,484 (+4 %); W9 413,069 / 404,539 → 528,994 / 640,617 (+43 %); W10 1,639,397 / 2,599,821 →
  1,573,248 / 2,645,050 (0 %: both slowed by the shared parent, L7); W11 1,269,337 / 10,545,825 → 10,927,791 /
  14,887,534 (noise); W12 428,063 / 364,371 → 412,125 / 328,361 (−7 %); W13 14,183 / 13,653 → 15,661 / 11,635. W12
  does not take the new path (its writer keeps versions in the scratch area); its alternated pairs over all Windows
  rounds give +4 %, +3 %, −4 % and −10 %, no consistent direction. Result: W9 gains on both platforms (Windows +26 %
  to +66 % per pair, WSL 2 +63 % to +131 %); the other loads are within this machine's noise. No threshold met yet:
  the checksum (O3a, O3) remains.
- **L9** (2026-11-04): independent verifier (sonnet/high) over `b3c0c44..387e758`: S1, S2 and S4 PASS, S3 left to
  the measurements (L4–L8), no blockers. It checked that `publish` is the only insertion into `versions`, under the
  lock and with `txid = last_committed + 1`; that `last_committed` only grows; that a snapshot's `txid` never changes;
  that the baseline already dropped the lock before reading the store, so the store read has the same exposure as
  before; and that a transfer selects a page only when the oldest read snapshot is at least its newest version, so
  no page that a snapshot opened without versions reads is written in place while it is open. Its checks: fmt ok,
  workspace clippy clean, `cargo test --workspace` 520 passed, 0 failed, 30 ignored; the L5 mutation fails the same 6
  tests. Advisories: the writer's snapshot is not in `live`, so a transfer can write in place a page it reads (as at
  the baseline); no committed test pinned the concurrent case. Taken: its probe, rewritten with a drop guard so a
  failing writer cannot hang the reader, is the new test
  `a_snapshot_opened_without_versions_keeps_its_page_while_newer_ones_are_written`: a reader opened on an empty
  scratch area keeps reading "a" while another thread commits, flushes and transfers 300 newer versions. With the
  transfer's oldest-snapshot condition removed it fails (`scratch_tests.rs:174`); restored, it passes. Checks: fmt
  ok, workspace clippy clean, `cargo test --workspace` 521 passed, 0 failed, 30 ignored. Verifier scratch removed.
