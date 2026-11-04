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
| T2 | S1, S2 | inline: decide at snapshot open whether any version can apply; skip the lock in `page_and_code` and `run` | pending | — |
| T3 | S2, S3 | inline: single-load runs after T2, compared with T1 | pending | — |
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
