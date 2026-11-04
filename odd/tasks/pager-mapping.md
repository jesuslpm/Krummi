# Pager mapping without a lock (O1)

Objective: O1 of [the performance plan](../../docs/performance/plan.md): readers get the pager's length and newest
mapping without its mutex. Branch `perf/pager-mapping` from `main` `7435b05`. Delivery: single branch, unmerged.
Test runner: `cargo test`. No PRD: an optimization without an observable change (K13 annotation of 2026-11-04).

## Specs

- **S1** (plan, O1, verbatim): "`raw_pages` and `len` take `FileStorage.state` on every page access only to read
  `len` and `maps.last()`. Both change only when the store grows, under the writer. Publish them so that readers get
  them without a lock (for example an atomic length and a pointer to the newest mapping, with the old mappings kept
  alive until the store closes, as today)."
- **S2** (plan, O1, invariants, verbatim): "a reference into a mapping stays valid for the life of `&Pager` (the
  SAFETY comment at `pager.rs:386-391`); a reader never sees a length beyond what the mapping it gets covers."
- **S3** (plan, O1, targets, verbatim): "W8, W9 above all; it lowers the cost of every read load."
- **S4** (acceptance, decided 2026-11-04, L2): single-load worker runs of W7–W12 before and after, at full scale, on
  Windows and WSL 2; the complete runs of the plan's *Accepting an optimization* run once after O1, O2 and O3a.
- **S5** (plan, risk): "concurrency (item 4 of the high-risk list): it gets an independent verifier and its own RDD
  review."

## Tasks

| ID | Specs | Route | Status | Evidence |
|---|---|---|---|---|
| T1 | S4 | inline: baseline single-load runs at `7435b05`, Windows and WSL | done | L5 |
| T2 | S1, S2 | inline: lock-free length and mapping, concurrency test | done | `9acf695`, L6 |
| T3 | S3, S4 | inline: single-load runs after T2, compared with T1 | done | L7 |
| T4 | S5 | independent verifier, RDD, Codex, closing | pending | — |

## Log

- **L1** (2026-11-04): user's request, verbatim: "sí" to "¿Hacemos el merge de `feat/benchmark` en `main` y
  arrancamos O1 con ODD?".
- **L2** (2026-11-04): acceptance asked ("¿Hacemos las ejecuciones completas después de cada optimización, como dice
  el plan, o una sola vez al final de O1, O2 y O3a?"); user: "Lo que recomiendes". Chosen: once after O1, O2 and
  O3a; each optimization is measured with single-load runs (S4). Saves about 8 h of machine time.
- **L3** (2026-11-04): estimate 2 h 30 min – 4 h, confirmed ("Sí"); start 12:56:24 UTC.
- **L4** (2026-11-04): map. `len` (`pager.rs:271`) and `raw_pages` (`:370`) lock `FileState` for every page; only
  `grow` (`:593`) changes it, under the writer; the store never shrinks (the only `set_len` down is a failed growth,
  before it is published); every mapping covers the whole file from offset 0. Temporary stores (`Storage::Memory`)
  keep their mutex: not on the benchmark's path.
- **L5** (2026-11-04): T1 baseline at `7435b05`, `scratchpad/o1/measure.py` (single-load worker runs at full scale
  from a base prepared once; median op/s of repetitions 1–3, repetition 0 the warm-up as in a complete run):

  | Load | Windows | WSL 2 |
  |---|---|---|
  | W7 | 435,691 | 356,734 |
  | W8 | 977,970 | 907,862 |
  | W9 | 359,611 | 372,642 |
  | W10 | 11,841,551 | 8,893,205 |
  | W11 | 11,154,322 | 8,247,194 |
  | W12 | 483,333 | 495,771 |
  | W13 | 18,226 | 6,313 |

  Two false starts of the script (a relative executable path on Windows; on WSL the executable was copied to the
  path of the base store) measured nothing.
- **L6** (2026-11-04): T2 in `9acf695`. `FileState` is gone: `FileStorage` has `maps: Mutex<Vec<Mmap>>`, `newest:
  AtomicPtr<u8>` (start of the last mapping) and `len: AtomicU64`, changed only under the `maps` lock. `grow` pushes
  the mapping, stores `newest` (Release) and then `len` (Release); readers load `len` (Acquire) and then `newest`
  (Acquire). Test `readers_see_every_page_while_the_store_grows` (`tests/pages.rs`): three readers check the first
  page of the newer half and the last page while the store grows ten times, in 20 stores. Mutation check, not
  committed: with `len` published before `newest` and a 200 µs pause between them, the test fails (`page 8 of 16`,
  three readers); a first version that checked only the last page passed under that mutation, because Windows had
  placed the newer view right after the older one, where the last page reads another blank page. Checks: fmt ok,
  workspace clippy clean, `cargo test --workspace` 520 passed, 0 failed, 30 ignored. Not a TDD RED: the change keeps
  behavior; the mutation check stands for it.
- **L7** (2026-11-04): T3, the same runs at `9acf695` (median op/s, before → after):

  | Load | Windows | WSL 2 |
  |---|---|---|
  | W7 | 435,691 → 509,832 (+17 %) | 356,734 → 500,475 (+40 %) |
  | W8 | 977,970 → 977,961 (0 %) | 907,862 → 1,095,073 (+21 %) |
  | W9 | 359,611 → 469,375 (+31 %) | 372,642 → 490,028 (+31 %) |
  | W10 | 11,841,551 → 17,632,828 (+49 %) | 8,893,205 → 19,215,754 (+116 %) |
  | W11 | 11,154,322 → 13,283,250 (+19 %) | 8,247,194 → 13,149,634 (+59 %) |
  | W12 | 483,333 → 509,161 (+5 %) | 495,771 → 712,318 (+44 %) |
  | W13 | 18,226 → 15,577 (−15 %) | 6,313 → 7,675 (+22 %) |

  W13 on Windows rerun twice more, alternating the two binaries: before 15,964 and 12,638, after 14,827 and 14,858;
  its repetitions range from 7,671 to 19,193 within one run, so the drop is noise. W12 on Windows the same: before
  450,989 and 331,021, after 406,608 and 427,721. No target threshold is met yet (Windows W9 469,375 against Voron's
  3,097,061 in the complete report): W9 still scales below W8, as the scratch lock (O2) and the checksum (O3) remain.
  The single-load runs do not match the complete reports' values (Windows W7 436 k here, 532 k there), so only the
  before/after pairs on the same machine state are compared.
- **L8** (2026-11-04): independent verifier (sonnet/high) over `7435b05..60dcdc8`: S1–S5 PASS, no blockers. It
  checked the ordering argument (growths serialized by the `maps` lock; a `len` read with Acquire makes its
  growth's `newest` visible), the failed-map path (shrunk before anything is published), the SAFETY comment and the
  bounds of `newest.add(first * PAGE_SIZE)` (`end <= len / PAGE_SIZE` is checked first; the mapped length fits in
  `usize`), and every other user of `len()`, `page_count()` and `maps` (bounds checks only). Mutation reproduced in a
  scratch copy: the test failed in 4 of 4 runs (three `page 8 of 16`, one `STATUS_ACCESS_VIOLATION`). Its checks:
  fmt ok, workspace clippy clean, `cargo test --workspace` 520 passed, 0 failed, 30 ignored. Advisories kept: the
  test's detection depends on where the system places the mappings (loom and Miri cannot model a mapping); the
  failed-map shrink is a no-op on Windows and the poisoned-lock `unwrap`s are as before. Its scratch copy removed.
