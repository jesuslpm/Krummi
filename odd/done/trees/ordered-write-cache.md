# Ordered-write cache

Objective: bring back the +9–12 % of W1 (in-order sets) that the reverted recent-leaf cache (T4 of the write
optimizations execution) gave, without its 5–6 % cost on W3 (random inserts), by remembering a leaf only for writes
whose keys come in order. Branch `feat/ordered-write-cache` from `0cb816c`; delivery `ask-on-risk`; tests
`cargo test --workspace`. No PRD: no product behavior changes (CLAUDE.md, *Workflow*), as S4 of
[the write optimizations execution](../pager/write-optimizations.md).

## Specs

- **S1** — The pending item of [the write optimizations closing report](../pager/write-optimizations-report.md):
  «**Caché de escrituras en orden** (W1, +9–12 % medido con T4): sin probar; sería un cambio aparte, con su A/B.»
  A write transaction remembers a leaf only for keys that come in order, so that a write whose key falls in it
  changes it without the walk from the root. No change of behavior or of stored bytes.
- **S2** — Acceptance, as S4 of the write optimizations execution: «keep only if an A/B shows a gain and nothing
  worse». The A/B compares Krummi alone, `main` (`0cb816c`) against the head, with the rule of Q7 of
  `pager-written-pages.prd.md` (a load improves when its median is higher and it is faster in most rounds; it worsens
  when its median is lower and it is slower in most rounds), at least ten rounds for loads under one second per
  repetition or with several threads and three for the others, over W1 and the loads Q7 names (W1–W6, W12–W15, W17,
  W19, W21, W23–W29, W7, W22, W26). W1 must improve. If a load worsens, the closing report says so and the user
  decides.
- **S3** — Complete runs of the bench (Krummi and Voron) on Windows and WSL 2 on the final code, in the closing report.

## Tasks

| ID | Specs | Route | Status | Evidence |
|---|---|---|---|---|
| T1 | S1 | inline | reverted by the user's decision (L14) | `aef6348`, `77be581`, `7d434e4`, `b12e08b`; revert `bbefd28` |
| T2 | S2 | inline (background A/B) | done: W1 better, W3, W5 and W7 worse (L6–L13) | `work/owc/*.json` |
| T3 | S3 | inline (background runs) | stopped by the user's decision (L14): Windows 29 of 29, WSL not finished | `work/owc/full/windows.md` |

## Log

- **L1** (2026-11-17) — User, after the proposal to start with the cache of ordered writes: «Adelante». Estimate
  given: 4–7 h; confirmed «Sí»; start 14:06:57 (`date`).
- **L2** (2026-11-17) — Design, from the T4 code (`3f35631`..`3c11fba`, reverted in `8b54d0b`) and W1's workload
  (`crates/krummi-bench/src/worker.rs`: in-order keys into an empty tree, in batches): every W1 write goes to the
  tree's rightmost leaf. T4 remembered the leaf of every walk, so a random insert paid a lock, a bounds check and a
  copy of the leaf's bounding keys on every write. This run remembers only a leaf on the tree's right edge, bounded
  below by its first key and open above, and forgets it on the first write that misses it: a random insert reaches
  the right edge rarely, and pays one relaxed load otherwise. Reads (`descend`) do not use it.
- **L3** (2026-11-17) — T1 `aef6348`: `git revert --no-commit 8b54d0b`, then the transaction's remembered leaf keeps
  only a first key (`remember_leaf(root, leaf, lower)`), the back-off of `197d41c` is gone, `recent_leaf` forgets the
  leaf on a key of the same tree below that key, and the tree layer remembers in `change` only a leaf on the right
  edge (`edge.is_some()`); `descend` and the `PageSource` hooks are back as at `0cb816c`. Tests restored from T4: the
  transaction unit test rewritten for the new rules; the multi-value equivalence test's in-order runs now climb the
  last tenth of its keys (the old runs wrapped through the whole range and reached the last leaf 140 times, under the
  2,000 its threshold asked for T4's any-leaf cache) and its threshold is 150 hits (198 observed, seeded). Checks:
  `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo test
  --workspace` 873 passed, 0 failed, 36 ignored. Micro (`recent_leaf_measurement`, release, in-process on/off): W1
  224.5 against 311.4 ns/op (−27.9 %), W2 2655.9 against 2682.6 (−1.0 %), W3 1133.9 against 1169.5 (−3.0 %).
  Risk: item 1 (a stale remembered leaf would put a key in the wrong leaf and corrupt stored trees); RDD assessment
  `0cb816c..aef6348`: high, review due.
- **L4** (2026-11-17) — Verifier of T1 (sonnet, high): S1 pass, no blocker. Every shape change (split, root growth
  and collapse, merge, child drop, range-removal frees, freeing a tree) reserves or frees pages; `merge_child` always
  frees the right page of the pair, so a merged last leaf is forgotten; the hit path calls the same `change_leaf` with
  the same search, and `stays_in_leaf`'s fit and underfull tests are the exact complements of `change_leaf`'s. Probes
  in a scratch copy: 144 digests of results and every node byte (120 slotted runs over three trees with three key
  styles, five value profiles and 30–95 % in-order bias; 24 multi-value runs) identical between `0cb816c` and
  `aef6348`, in release and debug, with 69,202 hits; removing `forget_leaf()` from `free` made the probe fail.
  `cargo test -q -p krummi-tree -p krummi-tx`: 0 failed. Advisories: (1) a hit does not read the leaf's ancestors, so
  damage to one of them after the walk that remembered the leaf, in the same transaction, shows only on the next
  walk: kept, as in T4 — the walk that remembered the leaf read and checked them in this transaction, and a change
  of their bytes since is the hardware-fault case the PRDs accept (P16); (2) a test comment still named T4's back-off:
  fixed in `77be581`; (3) `remember_leaf` is public and trusts its caller: kept, as T4's R1-001 (another crate calls
  it; hidden from the docs). Scratch removed.
- **L5** (2026-11-17) — RDD of `0cb816c..41a3703` (T1, the comment fix and this document), lenses risk and
  reliability, item 1 named, lineage `review-7fb7fdf34f3669c3`: approved, acknowledged (authority burned). Advisory
  findings: R1-001 and R3-001 (WARNING): the leaf's first key stays right only while every move of entries between
  leaves reserves or frees pages; a redistribution between siblings would break it silently, and no test mixed
  `try_add_at_last` with the remembered leaf. Checked: `merge_child` returns `Done` without moving anything when the
  pair does not fit, so no redistribution exists; hardened in `7d434e4` with a comment on that branch naming the
  invariant and `try_add_at_last` in the three-tree equivalence test. R1-002 (SUGGESTION, `remember_leaf` public):
  kept, as L4's advisory 3. Checks after `7d434e4`: fmt and clippy clean, `cargo test --workspace` 873 passed, 0
  failed, 36 ignored. Reviewed boundary: `41a3703`.
- **L6** (2026-11-17) — T2 A/B on Windows, `main` (`0cb816c`, `work/wm/main-target`) against T1 (`aef6348`,
  `work/wm/owc-target`; later commits change only comments and tests), bench power plan, performance cores, digests
  equal (`work/wm/abq.py`, `AB_BUILDS=main,owc`, `work/owc/ab-long.json`), 3 rounds, head/main op/s and rounds slower:
  W1 1.110 (0 of 3), W2 0.995 (2), **W3 0.969 (3 of 3)**, W4 1.015 (1), **W14 0.970 (3 of 3)**, **W15 0.974 (3 of
  3)**, W17 1.000 (1), W19 0.994 (2), W21 1.010 (2), W23 0.999 (2), W24 0.998 (2), W25 0.995 (2), W26 1.000 (1),
  W28 1.002 (1), W29 0.996 (2). W1 gains 11 %, but three loads worsen by the rule of S2. The short loads stopped at
  W13, which the worker does not run alone (W12's run reports it): rerun without it. The micro measurement, which
  switches the cache in one build, saw no cost on W3, so the cost is in the code the walk now carries (the `walk`
  argument of `change` and the remembering on every right-edge leaf), not in the hits. Variant 2 (uncommitted):
  `change` back as at `0cb816c`; the leaf is remembered from the `LastLeaf` that `change_leaf` already fills when an
  add goes to the end of the tree's last leaf and nothing above it changes (and that `set`, `try_add` and `remove`
  discarded), with the key added as its lower bound, after `fix_root`. A/B of `main`, variant 2 and T1 on W1, W3,
  W14 and W15 (`work/owc/ab-v2.json`).
- **L7** (2026-11-17) — Variant A/B, 3 rounds, over `main` (ns/op) as op/s ratio and rounds slower: W1 1061.1 —
  variant 2 1.126 (0 of 3), T1 1.125 (0); W3 2342.6 — variant 2 **0.966 (3 of 3)**, T1 0.963 (3); W14 8076.6 —
  variant 2 1.009 (0), T1 0.973 (3); W15 8622.4 — variant 2 1.003 (1), T1 0.972 (3). Variant 2 removes the cost on
  removals and keeps W1's gain; W3 still loses. Variant 3 (uncommitted): `change` and `change_leaf` byte for byte as
  at `0cb816c` (the search back inside `change_leaf`; a hit searches the leaf twice). A/B of `main` and variant 3 on
  W1, W2, W3, W14 and W15, 5 rounds (`work/owc/ab-v3.json`).
- **L8** (2026-11-17) — Variant 3 over `main`, 5 rounds: W1 1.072 (0 of 5 slower), W2 1.001 (2), **W3 0.982 (5 of
  5)**, W14 1.005 (1), W15 0.998 (2). The second search of a hit costs about half of W1's gain, and W3 still loses
  with the walk identical to `main`'s. To place W3's cost: a build of `main` with only T1's `transaction.rs` (the
  new fields, `forget_leaf` on every reservation and free, the unused hooks; `work/wm/txo-target`), A/B of `main`,
  that build and variant 2 on W3, 5 rounds (`work/owc/ab-txo.json`).
- **L9** (2026-11-17) — W3, 5 rounds, `main` 2342.0 ns: transaction-only build 0.997 (3 of 5 slower), variant 2
  0.975 (4 of 5). The transaction's part costs nothing; the cost is in the tree's `update`. Hypothesis: `change_leaf`
  now has two callers (`change` and `change_in_leaf`), so it is no longer inlined in `change`'s leaf step and every
  change pays a call. Variant 5: variant 2 with `#[inline(always)]` on `change_leaf` and `#[inline(never)]` on
  `change_in_leaf`; A/B of `main`, variant 5 and variant 2 on W1 and W3, 5 rounds (`work/owc/ab-v5.json`).
- **L10** (2026-11-17) — 5 rounds over `main`: W1 1069.3 ns — variant 5 1.129 (0 of 5 slower), variant 2 1.148
  (0); W3 2351.0 ns — variant 5 0.971 (5 of 5), variant 2 0.981 (5 of 5). The inlining hypothesis is wrong: forcing
  it does not help. What is known: the cache's work at run time costs W3 nothing (the in-process on/off measurement
  of L3), nor does the transaction's part (L9), and every variant that changes the tree's `update` loses 2–3.7 % on
  W3 whatever it changes, as the loop alignment of T5 of the write optimizations execution moved W6 by 3.6 %: most
  likely the layout of the compiled code, not measured. Decision taken during the run (listed in the closing
  report): keep variant 2, the best for W1 (+12.6 to +14.8 %) and neutral on W14 and W15, and finish the
  acceptance; by S2, W3 goes to the user. Variant 2 committed with the equivalence tests' in-order runs adding past
  the greatest key (an add at the end is now the only way to remember a leaf; the old runs, within the keys at
  random, found it 1 and 24 times) and the transaction's docs reworded. Checks: fmt and clippy clean, `cargo test
  --workspace` 873 passed, 0 failed, 36 ignored.
- **L11** (2026-11-17) — T2 A/B on Windows, `main` against variant 2 (`work/wm/owc2-target`, the code of `b12e08b`:
  later commits change only tests and docs), digests equal (`work/owc/ab2-long.json`, `ab2-short.json`), op/s ratio
  and rounds slower; by S2's rule: W1 **1.135** (0 of 3) better; W2 1.011 (0), W4 1.006 (0), W14 1.006 (1), W15 1.004
  (0), W17 1.009 (1), W19 1.007 (1), W24 1.001 (0), W25 1.014 (1), W26 1.007 (1), W29 1.006 (1), W27 1.025 (1 of
  10) better; W21 1.000 (2), W23 0.999 (1), W28 1.000 (1), W6 1.001 (5 of 10), W13 1.060 (6 of 10) the same; **W3
  0.985 (3 of 3), W5 0.988 (6 of 10), W7 0.989 (7 of 10), W12 0.992 (6 of 10), W22 0.996 (6 of 10) worse**. W7 and
  W22 only read, through code this change leaves as it was. Repeat, as the write optimizations execution did for
  its marked loads: W5, W7, W12 and W22 in 20 rounds, W3 in 10 (`work/owc/ab7.sh`), after the verifier of `b12e08b`.
- **L12** (2026-11-17) — Verifier of `b12e08b` (sonnet, high): S1 pass, no blocker. `change_leaf` fills `LastLeaf`
  only for a new key that becomes the tree's greatest, in its last leaf; removals and replaced keys never fill it;
  `try_add_at_last` never remembers; every merge that fits clears it, and `fix_root` runs before the remember, which
  uses the final root. Probes in a scratch copy: 375 randomized runs (300 over three slotted trees, three key styles,
  five value profiles, 30–95 % in-order adds past the greatest key, `try_add_at_last` + `append_last` bursts, aborted
  transactions; 75 multi-value runs), digests of every result and every page byte identical between `0cb816c` and
  `b12e08b` in release and debug, 529,911 hits; three mutations (no `forget_leaf()` in `free`, no reset of the
  rightmost leaf after a merge, an empty lower bound) each caught. `cargo test -q -p krummi-tree -p krummi-tx`: 226
  passed, 0 failed, 7 ignored. Advisories: the two of L4 (ancestors not read on a hit; `remember_leaf` public),
  already kept; the probe's runs with 30 % in-order adds hit less. Scratch removed.
- **L13** (2026-11-17) — Repeat on Windows (`work/owc/ab3-short.json`, `ab3-w3.json`), variant 2 over `main`: W5
  0.995 (11 of 20 slower) worse; W7 0.997 (12 of 20) worse; W12 1.002 (10 of 20) the same; W22 1.008 (5 of 20) and
  W13 1.008 (6 of 20) better; **W3 0.970 (10 of 10) worse**. W12 and W22 clear; W7 only reads, through code this
  change leaves as it was; W3 loses 3 % in every round. By S2, the closing report says so and the user decides.
  RDD of `41a3703..` the log commit after `6836f90` (variant 2 and its records), lenses risk and reliability, item 1
  named, lineage `review-af03f889b03835d4`: approved, acknowledged. Advisory R3-001 (WARNING: the equivalence tests
  never check that the remembered leaf is used): dismissed with evidence, they assert more than 1,000 and 150 hits
  (`crates/krummi-tree/src/tests.rs:1321`, `crates/krummi-tree/src/multi_tests.rs:1234`). Complete runs (T3)
  launched on the final code (`work/owc/full.sh`).
- **L14** (2026-11-17) — User, after the report of L13: «No merece la pena optimizar W1 si W3, W5 y W7 empeoran.»
  The cache is not adopted. The complete runs were stopped at 18:18: the Windows one had finished at 18:08 (Krummi
  meets 29 of 29 thresholds, `work/owc/full/windows.md`, not kept in `docs/benchmark/`: it measures code that is not
  merged), the WSL one was at W12 and was killed. `bbefd28` restores `crates/` as at `0cb816c` (checked with `git
  diff --quiet 0cb816c -- crates`). Checks: `cargo fmt --all --check` clean, `cargo clippy --workspace --all-targets
  -- -D warnings` clean, `cargo test --workspace` 868 passed, 0 failed, 35 ignored.
- **L15** (2026-11-17) — RDD of the last slice `d48f747..3092600` (the revert and the records; the assessment said
  medium, under budget), lens reliability, lineage `review-35d081d38face02c`: approved without findings,
  acknowledged. Codex code review of `0cb816c..3092600` launched.
- **L16** (2026-11-17) — Codex code review of `0cb816c..3092600`
  ([report](../../../docs/reviews/ordered-write-cache/2026-11-17-code.md),
  [resolutions](../../../docs/reviews/ordered-write-cache/2026-11-17-code-resolutions.md)): `approved`, no findings,
  one round.
- **L17** (2026-11-17) — Closing report: [ordered-write-cache-report.md](ordered-write-cache-report.md). The run
  closes on `feat/ordered-write-cache`, not merged and not pushed, with `crates/` as at `main`.
