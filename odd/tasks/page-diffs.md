# Journal the changed bytes of each page, and holes in the nodes (O8)

Objective: implement [journal-page-diffs.prd.md](../../docs/prd/krummi/journal-page-diffs.prd.md) (Y1–Y7), item 1 of
the investigation of 2026-11-08. Branch `perf/cheap-gaps`, after the version checks
([version-checks.md](version-checks.md)). Delivery: single branch, unmerged. Test runner: `cargo test`. Phase log and
closing report shared with the cheap gaps run.

## Specs

- **S1** (Y1, verbatim): "De una página que la confirmación obtuvo de su última versión confirmada para modificarla,
  el diario guarda el valor nuevo de cada byte que difiere de esa versión, nunca su cambio respecto del valor
  anterior. [...] De una página que la confirmación creó [...] el diario guarda siempre la página entera, sin escribir
  sus partes que son ceros; una página creada toda de ceros también consta."
- **S2** (Y2, Y3): recovery applies in commit order, created pages over zeros, the others over `krummi.data` (zeros
  past its end); checks every applied page that was not last applied as a value-run page at the end; a failure is the
  damage class with the lowest such page, after applying the rest.
- **S3** (Y4): holes in the nodes of the variable-size trees; reorganize only when a new entry fits only with the
  holes; used bytes as today, so no tree has more or fewer pages; the layout check accepts holes and rejects overlaps,
  cells out of the page or over their sizes, and a hole record that does not match.
- **S4** (Y5): format version `10`; a store of format `9` is of an earlier format.
- **S5** (acceptance criteria AC-Y1–AC-Y7 of the PRD).
- **S6** (Y7, AC-Y8): single-load pairs before and after, Windows and WSL 2; W2, W14, W15 and W19 improve on both;
  none of the measured loads gets worse.

## Tasks

| ID | Specs | Route | Status | Evidence |
|---|---|---|---|---|
| P1 | S1, S2, S4, S5 | inline: diff records, their base, recovery, format 10 | done | `ebbf6ee`, `7192dc7`, L3, L4, L6 |
| P2 | S3, S5 | inline: holes in slotted nodes | done | `fe2f1b8`, L5, L6 |
| P3 | S6 | inline: single-load rotations with the other parts | done | L8 |

## Log

- **L1** (2026-11-08): user's request, verbatim: "Cuando termines haz el 1 y 2 sin preguntar y cambia el PRD con tus
  recomendaciones sin preguntar también todo de seguido. Voy a estar fuera todo el día." PRD written with both
  reviewer rounds resolved by the agent's defaults (authorized by that message): check at the end of recovery with
  the override of J8 declared, holes not zeroed, a page's entry never much larger than today, holes in every
  variable-size tree and in temporary mode. Codex's PRD review: pending (usage limit until 2026-11-11 06:49).
- **L2** (2026-11-08): routes. P1 and P2 inline, sequential (P2's node changes are independent of the journal, but
  both are needed for the format bump and are under 1,000 lines). Risk: item 1 (stored format, recovery) and item 3
  (journal format): each gets an independent verifier and its own RDD review.
- **L3** (2026-11-08): P1. An entry is the page number with `RAW_ENTRY` and `CREATED_ENTRY`, a run count, then each run
  of 8-byte words that differ from the base as offset, length and new bytes (at most 8,206 bytes, against 8,200 of
  the old entry). The transaction marks pages it created (`Changed::created`); the commit takes the base of a copied
  page from the newest version in the scratch area, else from `krummi.data` (a temporary assertion against the
  writer's snapshot found no mismatch over AC-F16). Recovery decodes every entry and checks created sealed pages
  before writing, applies entries over zeros or over `krummi.data`, and checks every applied page that was not last a
  run page (`check_applied`) before writing the header. Magic `KrummiJ2`, format 10. Tests: the record golden hashes
  were recomputed and extended with copied pages; `bytes_tests` now pins the data pages after the meta page
  separately, unchanged (`0xef70dc69e134e0ee` at `e08b646` and after); `KrummiJ1` searches and damage at offset 100 in
  `tests/recovery.rs` moved to the first payload byte, since a record may be shorter than 100 bytes now;
  `entries_applied_over_any_mix_of_later_versions_give_the_last_one` (AC-Y3, AC-Y4: 200 rounds of byte mixes, a page
  created again, a second recovery) fails when entries are applied over zeros; `ac_y7_format_9_is_a_previous_format`.
- **L4** (2026-11-08): P1, defect found by AC-F16 (`ac_f16_sample_of_random_operations`, damage of page 2 at the
  reopening after the first simulated crash). `krummi.data` held a blank meta page while the header recorded commit
  1,490 as applied: `State::collect_garbage` drops old versions no snapshot needs without writing them, and
  `State::applied` then counted only the versions left, so the header passed commits whose page was never written.
  Harmless with whole pages (recovery rewrote them), wrong with diffs. Fix: `State::unwritten` keeps, per page, the
  commit of the oldest version dropped unwritten, cleared when the page is written in place; `applied` never passes
  it. AC-F16 passes; workspace 580 tests.
- **L5** (2026-11-08): P2. Header bytes 6–7 count the holes; `remove` leaves a hole (the lowest cell joins the gap, a
  node left empty resets); `insert` compacts only when the gap is too small; `free_space` is gap plus holes, so `used`
  and every split and merge decision are as before. The layout check marks each cell's bytes in a coverage bitmap,
  rejects overlaps, and requires cells plus holes to fill the end exactly; without holes that is the previous check
  (`the_layout_check_agrees_with_its_reference_on_damaged_nodes`, now skipping nodes whose header counts holes). New
  tests: `holes_follow_a_model_of_inserts_removals_and_replacements`,
  `the_layout_check_accepts_holes_and_rejects_overlaps`
  (AC-Y6), `trees_with_holes_have_the_pages_of_trees_without_them` (AC-Y5, against a test switch that compacts after
  every removal), `ac_y1_the_journal_keeps_little_of_a_page_a_commit_changes_little` (AC-Y1: 130, 210, 102 and 174
  bytes; with compaction on removal the removal took 1,018, under the PRD's quarter of a page, so the test adds a bound
  of 256 bytes for it). Layout check of a full leaf of 66 W1 cells: 288–329 ns (253–276 before holes, 451–561 at the
  start of the run). Workspace: fmt, clippy, 584 tests. AC-Y2 relies on the existing crash and recovery tests and
  AC-F16 (no new test).
- **L6** (2026-11-08): verification. `7192dc7` makes Y1's "Una página así que no cambió ningún byte no se guarda" true
  (an empty entry was written before; seen as `runs []` for the meta page in L4's debugging), with the record counting
  only its entries. Independent verifier (sonnet, high) of `ebbf6ee`, `7e4d872`, `fe2f1b8` and `7192dc7`: S1–S4 hold, no
  blockers; it confirmed the precondition of Y2 in the code (`collect_garbage` is the only path that drops a version
  unwritten, under `flushing`; the base read from `krummi.data` happens only for pages with no version) and by probes:
  6 seeds where the recovered `krummi.data` equals a clean close page by page, 1,056 crash points recovering a prefix,
  records with no entries, AC-Y4 through the opening, 40,000 nodes with holes against a byte-wise reference.
  Checks in its copy: fmt, clippy, 584 tests, release builds without test hooks, the 100,000-operation AC-F16 (13.45 s)
  and the five long sweeps of `krummi` pass; `ac10_every_altered_byte_with_the_journal` (about 75 minutes) was stopped
  after 25 minutes, and its stride-11 version passed in 420 s. Test gaps it found, now closed in `4aa2835`
  (`crates/krummi-tx/tests/page_diffs.rs`, from its probes): AC-Y2 was not covered by the existing crash tests and
  AC-F16, as L5 claimed (they compare contents, not page bytes); AC-Y3 and AC-Y4 were tested only below the opening.
  AC-Y3 through the opening runs one seed in the suite (about 40 cuts) and three seeds as an ignored acceptance test.
  Advisories: a node without cells that counted holes passed the layout check (now rejected, Y4); a stale doc comment
  of `apply_pages` (removed). Workspace: 589 tests. The measured `pd` binary is `7192dc7`; `4aa2835` adds tests and the
  empty-node rule only.
- **L7** (2026-11-08): RDD review of `e08b646..335abbb`, escalated with item 1 (journal format, recovery, node layout
  on disk), lenses risk, resilience and reliability, approved, lineage `review-8b7f0a4e693379ce`. Findings
  R4-commit-base-expect and R3-commit-base-expect-panic (both SUGGESTION, the same point): the commit panicked through
  `expect` if a copied page without a version were missing from `krummi.data`; it now fails with the damage class and
  that page (`fix(pager): fail with damage, not a panic, when a commit finds no base`). 589 tests pass.
- **L8** (2026-11-08): P3. Binaries: `base` = `e0ad729`, `cheap` = `d96a92b`, `vc` = `e08b646`, `pd` = `7192dc7`,
  built in release on each platform; each binary measures every load in its own work folder, from its own base store.
  Rotation (`work/cg/final.sh`, `rot-win.out`, `rot-wsl.out`): two rounds of each load over the four binaries, the
  second in reverse order, bench power plan, Windows 10:10–11:06 and WSL 2 11:06–13:40 UTC, nothing else running. Each
  load flagged as lower in both rounds was measured again in two new rounds (`work/cg/confirm.sh`, `conf-win.out`,
  `conf-wsl.out`, Windows 13:42–13:52, WSL 2 13:52–15:13 UTC); what was lower again got five alternating rounds with
  fresh stores (`alt.sh`, `alt-win.out`, 15:15–15:23; `alt-wsl.sh`, `alt-wsl.out`, 15:24–15:28 UTC). Change `pd` /
  `vc`, first round / second round (new rounds in brackets where a load was lower in both):

  | Load | Windows | WSL 2 |
  |---|---|---|
  | W2 | ×2.51 / ×2.48 | ×2.17 / ×2.24 |
  | W14 | ×2.36 / ×2.34 | ×2.00 / ×2.00 |
  | W15 | ×2.45 / ×2.47 | ×2.11 / ×2.16 |
  | W19 | ×2.08 / ×2.11 | ×1.70 / ×1.66 |
  | W13 | ×2.40 / ×2.31 | ×1.37 / ×1.44 |
  | W21 | +45 % / +43 % | +20 % / +18 % |
  | W3 | −7 % / −3 % [+5 % / −4 %] | +1 % / +5 % |
  | W7 | +3 % / +2 % | −7.5 % / −1.4 % [−4 % / +1.5 %] |
  | W12 | +25 % / +19 % | −1 % / −5 % [+10 % / −1.4 %] |
  | W18 | −3 % / −3 % [−15 % / −15 %; alternating: mixed] | −3 % / 0 % |

  W2 rises from 44,670 to 112,172 op/s on Windows and from 23,903 to 51,760 on WSL 2 (first round). S6 holds: W2, W14,
  W15 and W19 improve on both platforms, and the only load lower in both rounds twice, W18 on Windows, is inconclusive
  in the alternating rounds: `pd` lower in four of five (−0.4 % to −7.3 %), higher in one (+9 %), with the higher
  median (156,899 against 153,801) (correction of L9 and L10; the first wording said no load was lower twice). W18 on
  Windows reads in two modes, about 162,000 and 140,000 op/s, with the same store and binary; in five alternating
  rounds with fresh W17 stores (`alt-win.out`) `vc` also fell to the slow mode (142,153) and `pd` was higher in one
  round (167,675 against 153,801), so the slow mode is not the change's. The branch as a whole (`pd` / `base`): every
  load improves or is mixed on Windows; on WSL 2, W3 is lower in all four rounds (−2 %, −4 %, −3 %, −7 %) and in four
  of five alternating rounds (`alt-wsl.out`, median 265,432 → 257,230 op/s, −3.1 %), a cost of the branch on WSL 2
  with no single step lower in both rounds; W7 −3 % / −1 % then +1.5 % / +3 %.
- **L9** (2026-11-08): RDD review of the last slice, `335abbb..4a3a92e` (`ec9a9cc` and two log commits), assessment
  `medium` / `under_budget`, lenses resilience and reliability, approved, lineage `review-88827c8bc66af220`. Findings:
  R4-commit-base-damage-untested (SUGGESTION) and R3-commit-base-damage-untested (WARNING), the same point: no test
  reaches the commit's damage path for a copied page without a base. Dismissed with evidence: the path cannot be
  reached through any interface. `ScratchWriter::stage` asserts that every page is inside the store
  (`(HEADER_PAGES..page_count).contains(page_number)`, `crates/krummi-pager/src/scratch.rs`), `Pager::raw_pages`
  returns `None` only for pages past `len / PAGE_SIZE` (`crates/krummi-pager/src/pager.rs`, both storages), and the
  length only grows while the store is open (its only store is in `Pager::grow`, after mapping the larger file);
  `raw_pages(n, 1)` returns exactly one page, so the `try_into` cannot fail either. The comment now says the path is
  unreachable. R3-pd-s6-claim-contradicts-w18 (WARNING): fixed, L8's summary named no exception although W18 on
  Windows was lower in both rounds twice; it now names it and the alternating rounds that clear it.
- **L10** (2026-11-08): RDD review of `4a3a92e..f295627` (the comment of `dd378bb` and L9), lens readability,
  approved, lineage `review-bca2a488be0037db`. Finding R2-w18-alternating-claim-overstated (SUGGESTION): L8's
  corrected summary still overstated W18; it now gives the alternating rounds' counts and medians. Mechanism bound
  (estimate, not measured): W18 reads 4,096 values of 64 KiB, about 6 µs each, and the slower layout check with holes
  adds tens of nanoseconds per leaf read, under 1 %.
