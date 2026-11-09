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
| M8 | S2 | inline: debug builds check the nodes a write changed (user's decision, L11) | done | `11829c6`, L11 |

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
- **L10** (2026-11-09): RDD. Review of `22d045a..5096ba0`, assessed high (escalated with item 4, the lock-free
  filter), lenses risk, resilience and reliability, approved, lineage `review-84f63d463f2430fb`. Findings:
  R4-temp-grow-max-size and R3-shared-chunk-under-max-size (WARNING, the same point): with a maximum size a shared
  chunk could fail as full where a run's exact chunk fit; fixed, the new rule applies only without a maximum size and
  with one the area behaves as at `22d045a` (`with_a_maximum_size_scattered_free_slots_give_a_run_its_own_chunk`).
  R3-filter-unconditional-decrement (WARNING): `flush_all` now lowers a count only for a page that had versions.
  R3-ach2-scratch-path-unpinned (SUGGESTION): AC-H2 holds a snapshot so its first pass reads the temporary area. Fixes
  in `43e6f2a`. R4-filter-fixed-footprint and R3-stress-test-timing (SUGGESTION): accepted as documented (the 256 KiB
  are in the filter's doc; the deterministic filter test is the proof, the stress test adds to it). Review of
  `5096ba0..43e6f2a`, assessed high, lenses resilience and reliability, approved, lineage `review-08b5d34b424a9250`.
  R4-bounded-own-chunk-latency (WARNING): dismissed with evidence, a store with a maximum size keeps the rule of
  `22d045a` (`let own = pages > MAX_CHUNK_PAGES || state.free.values().sum::<usize>() >= pages;`, temp.rs:235 at
  `22d045a`), so its W17-like behavior is the baseline's. R3-001 (WARNING): dismissed with evidence, `flush_all` runs
  only in `Scratch::close` and `Drop` (`scratch.rs:565`, `:641`) with exclusive access, over pages taken from
  `versions.keys()`, so no reader sees the filter afterwards. R3-002 (WARNING): fixed in `a5e7c41`, AC-H2 now covers
  the cases with and without a snapshot keeping the version. Workspace: 593 tests.
- **L11** (2026-11-09): M8. The user approved ("sí") checking, after the measurements, the nodes a write changed
  only in debug and test-hooks builds, like Voron's `VALIDATE`. `krummi-tree` has no `test-hooks` feature and the
  tests build in debug, so the check runs under `debug_assertions` only (a default taken without asking). RED:
  `debug_builds_check_the_layout_of_the_nodes_a_write_changed` panicked with an index out of bounds at `node.rs:77`
  reading the damaged node; GREEN after the `PageCheck::Changed` arm of `node_at_with` checks the layout in debug
  builds, outside the record of layout checks, so AC-H1's count still holds. H1 of `pager-commit-checks` annotated
  in both languages. Checks: fmt and clippy clean, 594 tests, 0 failed.
- **L12** (2026-11-09): M7, single-load rotations of `main` (`22d045a`) against the branch (`908b65e`), two
  alternating rounds each, bench power plan; difference of the branch, first / second round. Windows: W1 +39.9 % /
  +38.5 %, W3 +75.9 % / +55.7 %, W12 +25.5 % / +55.2 %, W17 +58.5 % / +58.0 %, W2 +5.1 % / +3.3 %, W4 +15.4 % /
  +5.1 %, W13 +2.9 % / +31.1 %, W19 +4.3 % / +4.0 %, W21 +0.5 % / +2.8 %, W14 +103.7 % / +0.0 %; mixed W7, W10, W15;
  lower in both W11 (−0.1 % / −4.5 %) and W18 (−0.5 % / −2.2 %). WSL 2: W3 +53.4 % / +47.4 %, W12 +32.0 % / +25.7 %,
  W1 +10.3 % / +0.9 %, W2 +3.7 % / +2.5 %, W15 +1.9 % / +1.6 %, W13 +0.3 % / +0.6 %, W19 +5.8 % / +4.8 %; mixed W4,
  W7, W11, W17 (−4.0 % / +21.9 %), W21 (−20.4 % / +23.8 %); lower in both W10 (−10.0 % / −11.9 %), W14 (−0.1 % /
  −1.6 %) and W18 (−1.6 % / −3.7 %). Output: `work/margin/rot-win.out`, `rot-wsl.out`.
- **L13** (2026-11-09): M7, complete runs at `908b65e` (the Linux run from an archive identical to that commit),
  same power plan as the 2026-11-08 runs: Krummi meets 22 of 22 thresholds on Windows and on Linux (was 21 of 22 on
  each). Ratio Krummi / Voron, Windows: W1 1.04 → 1.43, W3 0.82 → 1.17, W12 0.87 → 1.16, W17 0.63 → 0.91; Linux: W3
  0.78 → 1.21, W10 0.80 → 1.90 (Voron 20.5 M → 9.4 M op/s), W12 0.85 → 1.10, W17 1.20 → 1.49, W21 0.82 → 0.96. Falls
  beyond 0.05 where Krummi itself falls: Windows W9 (1.30 → 1.23, Krummi −33 % and Voron −30 %), W18 (1.58 → 1.27,
  Krummi −13 %) and W22 (1.19 → 1.12, Krummi −7 %); Linux W9 (1.13 → 0.98, Krummi −20 %) and W16 (105 → 90, Krummi
  −11 %); Windows W20 falls 1.50 → 1.13 with Krummi −1 % and Voron +32 %. Confirmation rounds, two alternating
  rounds of `main` and the branch, difference of the branch: Windows W9 +2.2 % / +3.5 %, W11 −2.4 % / −0.6 %, W18
  −13.9 % / −6.3 %, W22 +3.1 % / +2.7 %; WSL 2 W9 +1.4 % / +5.3 %, W10 +1.3 % / −1.0 %, W14 +1.0 % / +0.2 %, W18
  +2.0 % / −0.0 %, W16 +105 % / −24 %. Confirmed cost: W11 on Windows, lower in all four rounds, −1.9 % on average
  (ratio 0.86, threshold met). W18 on Windows, lower in four rounds, is the store and not the binary: a cross test
  on the two stores of the confirmation gave medians, binary main / head on the store of main, 157,704 / 150,611 and
  152,566 / 154,765, and on the store of the branch 144,848 / 150,867 and 144,075 / 143,963, so the binary is mixed,
  while the binary of main is faster on the store of main in both rounds and the branch's in the second; the two 512
  MiB data files differ in 18 bytes, all in pages 0 and 1 (the two header copies). Not confirmed: W10 and W16 on WSL
  2, W22 on Windows. Reports: [Windows](../../docs/benchmark/2026-11-09-windows-margin.md),
  [Linux](../../docs/benchmark/2026-11-09-linux-margin.md); output in `work/margin/conf-*.out` and `conf3-*.out`.
- **L14** (2026-11-09): RDD of the last slice, `43e6f2a..8d6576d` (`a5e7c41`, `908b65e`, `11829c6`, `8d6576d`),
  assessed medium (`under_budget`, executable change in `crates/krummi-tree/src/lib.rs`) and reviewed as the closing
  rule asks; lenses readability and reliability; approved, lineage `review-eef9ca39c4f007f2`. The untracked tables
  design and PRD and the phase log were moved out of the tree during the review and put back. Findings: R2-001 and
  R3-margin-log-truncated-percentages (WARNING, the same defect): L12 and L13 had lost the digit before each `%`
  when rewrapped; fixed, rebuilt from the original texts. R2-002 (SUGGESTION): the plan row's rounded figures did
  not match the garbled log; resolved by the same fix (W1 +39.9 % / +38.5 % rounds to +40 % / +39 %). R2-003 and
  R3-linux-report-unknown-revision (SUGGESTION): dismissed with evidence; the bench writes `Krummi: desconocido`
  when it runs outside a git checkout, as the 2026-11-08 Linux report also does, and the report is the bench's own
  output, not edited; L13 records that the archive is identical to `908b65e` (`diff -rq` of the archive against `git
  archive 908b65e` printed nothing). R2-004 (SUGGESTION): fixed, `keeper` is declared mutable where it is made.
  R3-ach2-unheld-first-pass-path-unpinned (SUGGESTION): fixed in the doc comment, which now says the reads before
  the transfer come from the scratch area only where a snapshot keeps the version.
