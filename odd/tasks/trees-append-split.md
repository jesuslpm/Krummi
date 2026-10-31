# Append split — ODD execution document

Objective: a node that overflows at its end keeps its cells and starts a new sibling with the new one, so keys added
in increasing order fill their nodes instead of leaving them half full (performance, no product change, no PRD).
Branch: `feat/trees-append-split`. Delivery: `ask-on-risk`. Test runner: `cargo test --workspace` (Windows).
Estimate: 45 min – 1 h 15 min. Real start: 2026-10-31 08:25:56 +0000.

## Specs

- **S1 — Split at the end.** "añadir al final si no cabe parte en dos podría optimizarse, en vez de partir en dos
  añadir una hoja", for "todos los árboles": normal trees, the keys and the subtrees of multi-value trees, and the
  catalog, all of which split through `split` in `crates/krummi-tree/src/lib.rs`. A leaf keeps its cells
  untouched and its new sibling holds the new entry; a branch moves its last key up to the parent, so that its new
  sibling has the new key and a leftmost child (as Voron's `TreePageSplitter` does).
- **S2 — Any node.** "Acepto tu recomendación": the rule applies at the end of any node, not only the last node of
  the tree.
- **S3 — Unchanged otherwise.** Any other split still divides the node by bytes into halves; the format, the
  results of every operation and the structure invariants (D4) do not change.
- **S4 — Measured.** Pages in use and height after 100,000 entries in order and at random, before and after.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1–S4 | inline | done | see `git log` |

Route evidence: one function (`split`) and its tests; understood; no Writer trigger reason. Risk: none (no format,
contract or concurrency change; the randomized model tests and the structure checks catch a wrong split).

## Log

- **L1** (2026-10-31, user, verbatim): "Pues eso que dices de que añadir al final si no cabe parte en dos podría
  optimizarse, en vez de partir en dos añadir una hoja. Sería una buena optimización para cuando añadirmos datos
  ordenados." Then: "Primero vayamos a la optimización de inserción secuencial de todos los árboles". To the
  question "¿Lo aplico al final de cualquier nodo, como Voron, o solo en el último nodo del árbol?": "Acepto tu
  recomendación." (any node). Estimate 45 min – 1 h 15 min, answered "Sí". This was the pending item of
  `odd/tasks/trees-keys-report.md` ("Una división al final para inserciones crecientes sería un cambio aparte,
  medido").
- **L2** (T1 evidence): RED: `entries_added_in_order_fill_every_node_but_the_last_of_its_level`,
  `values_added_in_order_fill_their_subtree` and `a_stream_that_resumes_before_another_fills_its_leaves` failed on
  the base (nodes half full); `entries_added_in_reverse_order_still_split_into_halves` passed on the base and
  after (S3). GREEN after the change.
- **L3** (correction of the S2 rationale): the recommendation said that any node would also fill the leaves of
  several interleaved ordered streams (`prefijo + id`). A first test showed it does not: the first keys of the
  second stream stay in the leaf where the first stream ends, so each new key of the first stream lands before
  them, in the middle, and that leaf splits into halves; Voron behaves the same. Any node helps when a stream
  resumes at the end of a leaf that is not the last, after another stream took the keys past it; the test was
  rewritten for that case, which also fails with a split only at the tree's end.
- **L4** (S4 measurement, release, temporary store, 100,000 `set`s, pages in use / height; keys big-endian
  integers padded to their length):

  | Keys → values | Order | Before | After |
  |---|---|---|---|
  | 8 → 8 bytes | increasing | 543 / 3 | 272 / 2 |
  | 8 → 8 bytes | random | 427 / 3 | 427 / 3 |
  | 8 → 0 bytes | increasing | 344 / 2 | 173 / 2 |
  | 8 → 0 bytes | random | 257 / 2 | 257 / 2 |
  | 16 → 100 bytes | increasing | 3,051 / 3 | 1,523 / 3 |
  | 16 → 100 bytes | random | 2,194 / 3 | 2,208 / 3 |

  Random order costs 0.6 % more pages in the last case: a random key that lands past a node's last cell leaves
  that node full.
- **L5** (existing tests whose reach depended on half-full nodes): the T11 sweeps of AC-B18, AC-R12 and AC-V8 fell
  under their minimum number of failure points (585 of more than 700 in AC-B18), because writes in order split
  less. Their writes now add their keys in both orders (`in_both_orders`: the first half increasing, the second
  decreasing), so that the sweeps also inject failures in the new split; the minimums are unchanged. AC-V6 needs
  the tree to lose height after its removals: in increasing order its branches keep a single key of 2,048 bytes,
  over a quarter full, so they never merge (D4); it now adds its keys in decreasing order. AC-A15 needed more
  than 40 pages: it fills 120 entries instead of 60. AC-M12 needed key 9 in another leaf than key 13; with full
  leaves of 8 keys of 1,000 bytes they share one, so its other subtree key is now 5.
- **L6** (finding, pending, activated by this change): with keys of 2,048 bytes a branch with one key is over a
  quarter full and never merges with a sibling (D4), so after removals a tree can keep more levels than it needs.
  The merge rule is the base's, and decreasing inserts leave such branches too (`B0` branches in a scratch run),
  but increasing loads now reach it: on the base, AC-V6's increasing load lost height after its removals; with the
  append split it keeps it (2, 4, 4). Corrected after RDD's R3-inorder-merge-coverage, which called the earlier
  wording ("not caused by this change") inconsistent with that base result.
- **L7** (RDD review, 2026-10-31): commit `b7db2f1` against `main`, lenses `review-risk` and `review-reliability`,
  lineage `review-b7fc82a5ee615800`; result **approved**, acknowledged (authority burned). Risk: no findings.
  Reliability, three advisory findings, none blocking:
  - R3-branch-append-left-size (WARNING, inferential): the left branch of an append split could be left with no
    key or under a quarter full. Answer: a branch always fits three cells of the longest size (D4,
    `crates/krummi-tree/src/node.rs:53`), so it overflows only with three or more keys and keeps at least two; the
    old cells fill more than a page minus the new cell, so losing a last cell of at most 2,062 bytes leaves at
    least about 4,018 bytes, over the quarter (2,044). A leaf's left side is untouched and over half full. Only
    the new sibling, with one cell, can be under a quarter, as in Voron.
  - R3-inorder-merge-coverage (WARNING, behavior-activated): see L6; pending, for the user.
  - R3-catalog-coverage (SUGGESTION): no test of its own fills the catalog in order; it splits through the shared
    `split`. Not addressed.
