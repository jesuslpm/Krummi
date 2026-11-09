# Margin over the thresholds: W3, W12, W17 and W11

Objective: the five proposals of the margin diagnosis of 2026-11-09 (profiles in `work/prof/margin`). Branch
`perf/margin` from `main` `22d045a`. Delivery: single branch, unmerged. Test runner: `cargo test`. Phase log:
[margin-phases.tsv](margin-phases.tsv).

## Specs

- **S1** (user, verbatim): "Implementa todas las propuestas y cambia los PRD's sin preguntar con tus
  recomendaciones". The proposals, as given to the user:
  - "W3 a) Con 0 huecos, usar la comprobación anterior, sin mapa." (+4 %, recovers the Linux threshold.)
  - "W3 b) No volver a comprobar los nodos que escribió la propia transacción", with the recommendation "Comprobar
    cada nodo cambiado una sola vez, al confirmar" (needs a PRD: it changes S1 of the writer checks).
  - "W12: Búsqueda de versiones sin cerrojo para lectores: un índice publicado en cada commit que los lectores leen
    sin tomar nada." (+30–60 % in W12; concurrency, risk item 4.)
  - "W17: Reutilizar o preasignar esos trozos también con tamaño máximo, o ubicar los valores en trozos compartidos
    más grandes." (up to +25–30 %.)
  - "W11: Unir esa consulta con la memoria de organización ya comprobada." (+8–10 %.)
  - W10 on Linux: "Antes de tocar código, lo vuelvo a medir solo."
- **S2** (invariants): every existing test passes, except those a PRD of this run replaces; checks of `CLAUDE.md`.
- **S3** (acceptance, as in the earlier runs): single-load rotations before and after each step on Windows and WSL 2
  with the bench power plan; a load is worse when lower in both rounds and confirmed in new rounds; complete runs at
  the end, compared with `docs/benchmark/2026-11-08-*-cheap-gaps.md`.

## Tasks

| ID | Specs | Route | Status | Evidence |
|---|---|---|---|---|
| M1 | S1 (W3 a), S2 | inline: the tiling pass for nodes without holes | done | `da18e52` |
| M2 | S1 (W3 b) | inline: PRD, reviewed, translated, annotations | done (Codex pending) | `e16d218`, L3, L6 |
| M3 | S1 (W3 b), S2 | inline: a write never checks the nodes it changed | done | `a583aab`, `c5093a5`, L4, L6 |
| M4 | S1 (W12), S2 | inline: readers skip the lock for pages without versions | done | `77b57e7`, L7 |
| M5 | S1 (W17), S2 | inline: shared chunks grow over few scattered slots | done | `f6c8f6a`, L5 |
| M6 | S1 (W11), S2 | inline: measured and dropped | dropped | L8 |
| M7 | S1 (W10), S3 | inline: W10 alone, rotations, complete runs, RDD, closing report | pending | |

## Log

- **L1** (2026-11-09): user's request, verbatim: "Implementa todas las propuestas y cambia los PRD's sin preguntar con
  tus recomendaciones". Start: 2026-11-09 06:07:27 UTC. No estimate was asked for ("sin preguntar"). W21 (two copies
  of each page, about 10 %) was proposed for later, not in this run: default kept.
- **L2** (2026-11-09): M1. With no holes, the layout check walks the cells from `top` and matches the offsets with
  their starts, the check before holes; only a node with holes builds the map of covered bytes. W3's profile on WSL 2
  put the check at 35.5 % of a shorter run at `e0ad729` and 38.9 % at `22d045a`. The differential test now also
  checks the coverage path against the reference on nodes without holes. Node tests pass.
- **L3** (2026-11-09): M2. PRD `pager-commit-checks` (H1–H4) with the `prd` skill: an explorer mapped the four hot
  paths; the reviewer (sonnet, high) ran twice and its findings were resolved by the agent's defaults, authorized by
  L1 (T11 precedence, place in E2, overrides of U1, U2, G6 and S4 annotated, deterministic ACs). Translation by a
  subagent (sonnet, medium), which kept "layout" for "organización" as the earlier translations do. The margin
  diagnosis went into the plan so that the PRD can cite it. Codex's PRD review: pending (usage limit until 2026-11-11
  06:49). `e16d218`.
- **L4** (2026-11-09): M3, first design (`a583aab`): each node a write changed checked once, at commit, through
  `WriteTransaction::node_mut` and a recorder of the commit's checks. Tests that committed damaged nodes through the
  tree moved their damage to a write of its own (`an_inconsistent_value_reference_is_damage_and_not_a_panic`).
- **L5** (2026-11-09): M5. Raising the kept own chunks to 64 MiB changed nothing: an instrumented W17 run (4
  repetitions, 16,384 values) took 7,425 own chunks, 4,543 kept and 2,882 new files, and the kept chunks never reached
  the cap; each new file was created with 9 to 44 scattered free slots (median 14), the tails of the shared chunks.
  Fix: a run takes a chunk of its own only when the scattered free slots hold at least a largest shared chunk
  (`MAX_CHUNK_PAGES`); fewer, and it grows the shared chunks, so the area reserves at most one more shared chunk than
  before. W17 on Windows, `main` against the change, four alternating rounds (bench power plan): 3,284, 3,325, 3,205,
  3,190 against 5,167, 5,106, 5,121, 5,217 op/s, +58 %. The R1 test now pins both sides of the rule
  (`scattered_free_slots_take_a_chunk_of_its_own_only_when_many`, which fails on the old rule). `f6c8f6a`.
- **L6** (2026-11-09): M3, second design. W2 on Windows, `main` against M1+M3+M5, four alternating rounds: 106,492,
  106,317, 104,477 against 99,462, 97,714, 99,502 op/s, −5 % to −8 % (a third round, disturbed, halved both binaries
  and is discarded): each W2 commit changes about a thousand leaves nobody reads again, which S1 never checked and the
  commit check did. Decision (default, authorized by L1): the write checks none of the nodes it changed, neither on
  read nor at commit, and later transactions check what it committed on their first read (U1, C2), as they already did
  for changed nodes nobody reread. The PRD and its annotations were rewritten to that design (the E2 annotation
  removed) and reviewed again. Windows, `main` against it, four alternating rounds: W1 ~700k → 905k–943k (+30 % to +35
  %), W3 ~279k → 390k–409k (+40 % to +47 %), W2 105k–107k → 105k–113k (no loss). Workspace: 590 tests. `c5093a5`.
- **L7** (2026-11-09): M4. A filter of 65,536 counters, one per slot of page numbers, counts the pages with versions;
  it changes under the write lock when a page's list of versions goes from empty to not and back (`publish`,
  `written`, `flush_all`), and readers skip the lock when its slot is zero. A version a snapshot can see was published
  before it registered, so its count is visible to it. Test `the_filter_of_pages_with_versions_follows_them` (fails
  without the increment). W12 on Windows, M1+M3+M5 against M4 added, four alternating rounds: 1.71M, 1.88M, 1.83M,
  1.60M against 2.28M, 2.32M, 2.34M, 2.19M reads/s, +25 % to +30 %. `77b57e7`.
- **L8** (2026-11-09): M6, dropped. The 7.7 % at `remembered_run` is the first touch of the leaf's memory (its bytes
  8–16 are read first). A single-page fast path in the remembered bitmap, W11 on Windows, four alternating rounds:
  −0.9 % to +1.6 %, within noise; merging both lookups would only move that miss to the next line, and needs a PRD
  change. Default: no change.
- **L9** (2026-11-09): verification, two independent verifiers (sonnet, high) in scratch clones, now removed. M1 and
  M3 at `2ae318a`: met, no blockers. fmt, clippy and 590 tests pass; a differential probe of the old `cells_are_valid`
  against the new check on 600,000 damaged nodes, with and without holes, found no disagreement; an audit of the 29
  `page_mut` calls of the tree found every changed node new or a copy of a node read and checked first (H3's premise);
  AC-H1–AC-H3 met. Advisories: three stale comments and no AC-H2 case for the root of a one-leaf tree, fixed in
  `7a8e012`. M4 and M5 at `2ae318a`: met, no blockers. The same checks and `ac_f16` (21.3 s) pass; every change of a
  page's list of versions between empty and not updates the filter under the lock (`collect_garbage` never empties
  one); a stress of one writer and seven readers over 80 s, 182,760 commits and 630,082 snapshots, found no snapshot
  seeing two commits, and fails within 8 s without the filter's increment (kept as
  `crates/krummi-tx/tests/version_filter.rs`, 2 s per mode, `85324fc`); M5's bound held over 40,000 random operations
  per run, at most one more shared chunk than page by page, and stores with a maximum size stop with its class without
  overshooting. Advisories, accepted: the filter takes 256 KiB per store outside the maximum size; it skips nothing
  once more than about 65,000 pages have versions; M5 can reserve one more shared chunk (8 MiB) than before, as its
  module documentation says.
