# Ordered-write cache

Objective: bring back the +9–12 % of W1 (in-order sets) that the reverted recent-leaf cache (T4 of the write
optimizations execution) gave, without its 5–6 % cost on W3 (random inserts), by remembering a leaf only for writes
whose keys come in order. Branch `feat/ordered-write-cache` from `0cb816c`; delivery `ask-on-risk`; tests
`cargo test --workspace`. No PRD: no product behavior changes (CLAUDE.md, *Workflow*), as S4 of
[the write optimizations execution](../done/pager/write-optimizations.md).

## Specs

- **S1** — The pending item of [the write optimizations closing report](../done/pager/write-optimizations-report.md):
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
| T1 | S1 | inline | pending | |
| T2 | S2 | inline (background A/B) | pending | |
| T3 | S3 | inline (background runs) | pending | |

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
