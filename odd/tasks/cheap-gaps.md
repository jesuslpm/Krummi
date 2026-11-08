# Cheap gaps against Voron: the byte sum, W11, W21, W17 and W1

Objective: the bench's read checksum and four cheap internal changes found by comparing Krummi with Voron's source.
Branch `perf/cheap-gaps` from `perf/write-reuse` `e0ad729`. Delivery: single branch, unmerged. Test runner:
`cargo test`. No PRD: the byte sum is a bench bug fix (same result, K8 unchanged) and the rest are internal.

## Specs

- **S1** (user, verbatim): "Vale, haz el arreglo de byte_sum y vuelvo a medir W18." The fix, as proposed: "usar en
  `byte_sum` la misma suma `psadbw` que `Data.cs`, con la suma escalar como respaldo en procesadores que no sean
  x86-64. El resultado no cambia". Measure W18 afterwards.
- **S2** (user: "Suma las baratas", item 6 of the proposal, verbatim): "Valores grandes: quitar las 2 copias de más y
  marcar los archivos temporales como temporales en Windows" (W17).
- **S3** (item 3, with the user's "Sí" to "¿Vamos solo por dentro?"): "Abaratar la creación de un `Range`: sin
  reservas de memoria y con chequeos más livianos al bajar. No cambia la interfaz pública ni el banco" (W11).
- **S4** (item 5, verbatim): "Tamaño fijo: llegar a la hoja 2 veces en vez de 4" (W21).
- **S5** (item 7, verbatim): "Caché de la última hoja encontrada" (W1).
- **S6** (invariants): no public API change in `krummi` and no change to the bench's engine interface; the data and
  journal bytes stay the same; every existing test passes.
- **S7** (acceptance, as in the write reuse run): single-load runs before and after, on Windows and WSL 2, with the
  bench power plan, measure what improves; no other load gets worse (S5 rule: lower in both rounds).

## Tasks

| ID | Specs | Route | Status | Evidence |
|---|---|---|---|---|
| T1 | S1, S6 | inline: vectorized `byte_sum` in the bench | done | `71c6aa8`, L3 |
| T2 | S3, S6 | inline: cheaper scans without prefix (allocations kept, L6) | done | `3d6695f`, L6 |
| T3 | S4, S6 | inline: one leaf read per fixed-size insert | done | `8f09c51`, L7 |
| T4 | S2, S6 | inline: temporary files marked temporary on Windows (copies moved to O8, L5) | done | `fafb5bf`, L5 |
| T5 | S5, S6 | inline: faster layout check of slotted nodes instead of a last-leaf cache (L8) | done | `d96a92b`, L8 |
| T6 | S7 | inline: single-load runs before and after, Windows and WSL 2 | running | |
| T7 | all | RDD per slice, Codex code review, plan update, closing report | pending | |

## Log

- **L1** (2026-11-08): user's requests, verbatim: "Vale, haz el arreglo de byte_sum y vuelvo a medir W18. Pero antes
  ¿Qué otras optimizaciones podrías introducir? y cuanto sería el beneficio?"; then "Suma las baratas"; to
  "¿Vamos solo por dentro?", "Sí"; to "Estimación para toda la tanda: entre 3 y 5 horas. ¿Arranco?", "sí". Estimate
  3–5 h. Start: 2026-11-08 06:26:27 UTC.
- **L2** (2026-11-08): routes. All inline: the units are small and sequential, with no writer reason. One read-only
  explorer maps T2–T5 (mapping trigger: four unfamiliar paths). Microbenchmark behind S1 (`work/sumbench`, bench
  power plan): scalar sum 60,300 op/s per 64 KiB value, SSE2 `psadbw` 198,700; W18 on Windows was Krummi 51,625,
  Voron 109,852, LMDB 60,534.
- **L3** (2026-11-08): T1. A safe sum in 16 `u16` lanes, folded every 256 chunks, instead of `psadbw` intrinsics:
  `krummi-bench` denies unsafe code (D12) and the intrinsics need it. Microbenchmark under the bench power plan, 4,096
  values of 64 KiB in random order: old 60,820 op/s, `psadbw` 203,950, the new `byte_sum` 193,491. RED does not apply
  (same result by design); `byte_sum_adds_every_byte_at_any_length_and_offset` pins it against the byte-by-byte sum.
  Commit `71c6aa8`.
- **L4** (2026-11-08): user, verbatim, while T1 ran: "Cuando termines haz el 1 y 2 sin preguntar y cambia el PRD con
  tus recomendaciones sin preguntar también todo de seguido. Voy a estar fuera todo el día." Items 1 and 2 of the
  proposal: "Diario con diferencias de página (O8) + hojas con huecos al borrar" and "Lecturas sin el mutex del área
  de trabajo + caché de páginas por transacción + sin rechequear versiones". Plan: O8 needs a PRD (plan.md, O8:
  journal format and recovery); remembering the check of scratch-area versions amends C3 (plan.md, candidate 2). The
  user authorized writing both PRDs with my recommendations and implementing them without asking. They go after T1–T7,
  as a second part of this run, with their own feature document each.
- **L5** (2026-11-08): profiles with samply on WSL 2 of W11, W1, W21 and W17 at `71c6aa8` (`work/cg/prof`; the
  root step of the plan's method was denied by the session's permissions, and `perf_event_paranoid` was already 1).
  T4: W17 takes about 330 µs per value on Windows (3,031 op/s) and most of it writes 16 MiB per commit; zeroing a
  value's buffer costs about 3 µs (1 %), so it stays. The raw payload copy of the record and its fallback copy after
  LZ4 (30 % of W17 on WSL 2) cannot be removed without either a copy of the compressed bytes for every record that
  compresses or a change of the record bytes: the plan already leaves that copy to O8. Decision (default taken while
  the user is away): T4 is only `FILE_ATTRIBUTE_TEMPORARY`; delete-on-close is left out, since the files are already
  removed explicitly. RED: `a_temporary_area_file_is_marked_temporary_on_windows` saw attributes `0x20`.
- **L6** (2026-11-08): T2. W11's profile: latency of the first read of each random leaf (`node::key`, 21 %),
  interval bound tests (`after_lower`, `before_upper` and libc `memcmp`, about 9 %) and the two allocations per
  `Range` (about 5 %). The bound tests now skip the empty prefix. The allocations stay: removing them needs a
  hand-made inline vector, much code for under 5 %. RED does not apply (same results); the interval tests pin them.
- **L7** (2026-11-08): T3. The leaf decision uses the node `node_of` just checked, and the leaf is read once more to
  change it, instead of three times. The fault sweep fixture of the fixed-size tree now reaches 579 page operations
  instead of 777, every one of them still applying nothing (`ac_x12_a_failure_at_every_...`, run in full);
  `FIXED_POINTS` went from 760 to 560. W21's profile puts 61 % of its time in the commit (copies, LZ4, xxh3), so T3
  gains a few percent at most; the commit belongs to O8.
- **L8** (2026-11-08): T5. W1's profile spends 39 % in `node::is_valid`: every insert changes the same leaf, so S1 of
  the writer checks makes the next insert check it whole. A last-leaf cache would only save the branch levels (about
  10 %); a faster check keeps S1 as it is and helps every write load. Decision (default): replace S5's cache with the
  faster check. The check now follows the cells once and then matches the offsets with them, specialized for leaves
  and branches: 253–276 ns against 451–561 ns on a full leaf of 66 W1 cells (release, three runs). It accepts exactly
  what it accepted: `the_layout_check_agrees_with_its_reference_on_damaged_nodes` compares it with the previous check
  on 60,000 damaged nodes, and fails at round 8 when the test of repeated offsets is removed. Workspace: fmt, clippy
  and 571 tests pass.
- **L9** (2026-11-08): T6, first Windows round (`work/cg/win.out`, bench power plan, `before` = `e0ad729`,
  `after` = `d96a92b`), medians of the measured repetitions, op/s:

  | Load | Before | After | Change |
  |---|---|---|---|
  | W1 | 282,645 | 331,037 | +17 % |
  | W2 | 20,773 | 42,785 | +106 % |
  | W3 | 279,800 | 281,415 | +1 % |
  | W7 | 963,231 | 922,640 | −4 % |
  | W10 | 15,458,794 | 18,522,429 | +20 % |
  | W11 | 11,841,511 | 14,402,598 | +22 % |
  | W12 | 811,359 | 492,744 | −39 % |
  | W13 | 30,858 | 15,117 | −51 % |

  W12 and W13 fall from one repetition to the next inside the process (W13 after: 29,488, 19,280, 15,117, 14,865;
  before: 30,849–31,316), as if something accumulated. Suspect: `FILE_ATTRIBUTE_TEMPORARY` (`fafb5bf`), which keeps
  the dirty pages of the temporary area in memory. The run was stopped to bisect it with a build of `d96a92b`
  without that block (`work/cg/bisect.sh`); the full pairs run again with the final binary. The stop let the script
  start its WSL part, which was killed before measuring.
- **L10** (2026-11-08): bisect (`work/cg/bisect.out`, Windows, bench power plan, rounds `notemp after before notemp`):
  W12 853,848 / 986,029 / 847,512 / 919,974 and W13 29,648 / 33,989 / 32,220 / 32,541 op/s; W2 42,760 / 42,060 /
  40,697 / 41,841. `after` is not slower, and `before` reads W2 at 40,697 (41,227 in the complete run of 2026-11-07),
  not 20,773: L9's whole round was disturbed (it ran while subagents worked on the same machine) and is discarded.
  W2 gains about 3 %, not 106 %. The pairs run again at the end with every binary, on a quiet machine. Rule kept for
  the rest of the run: no measurement while subagents or builds run.
- **L11** (2026-11-08): verification of `e0ad729..d96a92b`. Assessment `medium` / `under_budget`, escalated with
  `--escalate-item 2` (T5 rewrites the layout check, a damage guard) to `high`. Independent verifier (sonnet, high):
  S1–S6 met, no blockers; its own differential probe of 400,000 nodes (34,233 accepted) found no disagreement with the
  previous check; `cargo fmt --check`, clippy, `cargo test --workspace` and the full fixed-size sweep pass in its
  scratch copy (removed). Advisory: `is_valid` now panics on a slice shorter than a payload, where the old check could
  return false; every caller passes a whole payload. RDD review: lenses risk and reliability, approved, lineage
  `review-3be47b92a02e4060`, 07:51:13–07:51:59 (the phase log missed it: the first attempt could not detach HEAD over
  the modified phase log and closed its entry). Finding R3-fixed-leaf-split-precompute (SUGGESTION): no test pins the
  split boundary of a fixed-size leaf; fixed in `49e25ee` (`a_leaf_splits_exactly_when_it_is_full`, which fails when
  the boundary moves by one).
