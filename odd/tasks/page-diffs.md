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
| P3 | S6 | inline: single-load rotations with the other parts | running | |

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
  tests: `holes_follow_a_model_of_inserts_removals_and_replacements`, `the_layout_check_accepts_holes_and_rejects_overlaps`
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
