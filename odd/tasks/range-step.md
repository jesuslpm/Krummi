# Range step — execution document

Objective: make the per-entry step of a tree range (`krummi-tree` `Range`) cheaper, so that `intervalos` (W11) and
`tabla-intervalos` (W27) close their gap with Voron. Branch `feat/range-step` from `main` at `0257204`. Delivery: one
local commit per task, not merged nor pushed. Tests: `cargo test --workspace`; measurements: single loads pinned to
the performance cores under the bench power plan (`work/w11/win-ceiling.ps1` pattern).

## Specs

- **S1** — Optimization 1, as proposed to the user (L1): «Comprobar la cota superior y el cruce con el extremo trasero
  una vez por hoja, comparando con la última clave de la hoja, y no en cada entrada.» Applied to both ends: the front
  checks the upper bound against the leaf's last key, the back the lower bound against its first key. The check
  against the other end's last key stays per entry while that end has started (it is one `None` test otherwise).
- **S2** — Optimization 2 (L1): «Leer la clave y el valor con una sola lectura de la cabecera de la celda.»
- **S3** — Optimization 3 (L1): «Abaratar `Pager::run_with` en cada cambio de hoja: ~1,4 ns por entrada.» The profile
  (L3) shows that this cost is the first access to the new leaf, not the integrity check, so the approach is to
  prefetch the next leaf while the scan reads the current one. Kept only if single loads measure a gain (assumption
  taken by default, L3).
- **S4** — No behavior change: every scan returns the same entries, in the same order, with the same failures. A new
  test pins full scans with both bounds and a prefix, forward, backward and alternating, over a tree of several
  levels.
- **S5** — Each optimization is measured on its own against the previous binary (W11 and W27, Windows, pinned, under
  the bench power plan), and the final binary in a complete Krummi-and-Voron run on Windows.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1, S4 | inline | done (L4) | `fab4085` |
| T2 | S2, S4 | inline | done (L5) | `05fbba3` |
| T3 | S3, S4 | inline | done (L6) | `44b1094` |
| T4 | S5 (complete run, reviews, closing report) | inline | done (L7–L12) | `0d99b0e`, `a3527a6`, `7cf21b8`, closing report commit |

## Log

- **L1** (2026-11-13, the user's requests, verbatim): to «¿Querés que implemente estas optimizaciones del paso?»,
  «Sí, ¿Cuanto ganaríamos?»; to the estimate (2–4 h) and «¿Arranco?», «Arranca con las tres optimizaciones.» The three
  optimizations proposed: «1. Comprobar la cota superior y el cruce con el extremo trasero una vez por hoja,
  comparando con la última clave de la hoja, y no en cada entrada. 2. Leer la clave y el valor con una sola lectura
  de la cabecera de la celda. 3. Abaratar `Pager::run_with` en cada cambio de hoja: ~1,4 ns por entrada.»
- **L2** (2026-11-13, before the run, measured in WSL at `0257204`, scripts and profiles in `work/w11`): seek-only
  W11 (1M ranges of 1) Krummi ~850 ns, Voron ~1,200; step-only (10k ranges of 1,000) Krummi ~40 ns per entry, Voron
  ~32. Leaf layout equal (next cell in key order adjacent: Krummi 55.2 %, Voron 53.2 %; fill 69 % both). Step-only
  profile per entry: bench sum 10.3 / 9.7 ns, first touch of the cell 9.2 / ~9, step bookkeeping ~12.7 / ~2, leaf
  change 4.6 / 3.2. Ceiling of S1+S2 (checks removed, one header read; Windows, pinned, three rounds): W11 14.5 M →
  15.7 M (+8 %), W27 9.1 M → 9.63 M (+6 %).
- **L3** (2026-11-13): run started at 07:36:40 (`date`), estimate 2–4 h. In `Pager::run_with`, the hot instruction
  (9.8 % of W11) is `cmp QWORD PTR [rax+0x8], r14`, the read of the page number in the header of the run
  (`remembered_run`, `pager.rs:412`): the first access to the leaf. Default taken without asking (CLAUDE.md, no
  stops once started): S3 becomes a prefetch of the next leaf, kept only if it measures a gain; the complete run of
  S5 is Windows only, with WSL single loads of W11 and W27.
- **L4** (2026-11-13): T1. Each end of a range keeps `clear`, set when it reaches a leaf: the front's upper bound and
  prefix checked once against the leaf's last key, the back's lower bound and prefix against its first key (both
  checks pass on a contiguous part of the key order). The check against the other end's last key stays per entry.
  New test `bounded_scans_stop_inside_a_leaf_from_both_ends` (300 cases with both bounds and a prefix on a tree of
  three levels, forward, backward and alternating; passes on the base, as the change keeps behavior); mutations
  deciding with the wrong key of the leaf (front: the first; back: the last) fail it at case 0. Checks: fmt, clippy
  clean, 830 passed, 0 failed, 34 ignored. Windows, pinned, under the plan (`work/w11/t1.log`, medians of rounds 1
  and 2; round 3 dropped: both binaries fell at once, W27 `k0` 4.8 M and `kT1` 0.9 M, a disturbance of the machine):
  W11 `0257204` 13,851,262 / 13,995,254, T1 14,806,901 / 14,840,970 (**+6.5 %**); W27 8,898,024 / 9,125,206, T1
  9,360,482 / 9,369,700 (**+4.2 %**).
- **L5** (2026-11-13): T1 committed (`fab4085`); RDD assess `0257204..fab4085` medium, `under_budget` (pending in
  the slice). T2: `node::entry` gives the key and the value of a leaf cell from one read of its offset and header;
  `node::value` is now `entry(..).1`, and both ends of a range step with it. Checks: fmt, clippy clean, 830 passed,
  0 failed, 34 ignored. Windows, pinned, under the plan (`work/w11/t2.log`, medians of three rounds, T1 against
  T2): W11 15,472,810 / 15,633,952 / 15,657,563 against 15,930,359 / 15,788,156 / 15,826,977 (**about +1.5 %**);
  W27 9,487,497 / 9,665,757 / 9,608,505 against 9,797,886 / 9,857,450 / 9,777,343 (**about +2.3 %**). The machine
  ran faster than during L4 (T1's W11 at 15.5 M against 14.8 M), so each step is compared only within its own batch.
- **L6** (2026-11-13): T2 committed (`05fbba3`); RDD assess `0257204..05fbba3` medium, `under_budget`. T3: a range
  end that reaches a leaf whose keys are all within its bound asks for the neighbouring leaf of the same branch (the
  next child for the front, the previous one for the back) to be prefetched: `PageSource::prefetch` (no-op by
  default; reads through `ReadTransaction`), `krummi_tx::ReadTransaction::prefetch`, `Snapshot::prefetch` (only
  when the scratch area's filter says the page has no version, as finding the version takes the lock) and
  `Pager::prefetch` (`_mm_prefetch` on x86-64, nothing elsewhere; the one `unsafe` stays in `krummi-pager`). Three
  sizes measured against T2 (Windows, pinned, under the plan; medians of rounds): the whole page (128 lines,
  `work/w11/t3.log`) W11 −10 %, W27 −8.5 %; 64 bytes (`t3ab.log`) W11 +2.4 %, W27 −0.6 %; 256 bytes W11 +4.1 %,
  W27 −0.5 %. Confirmed with five rounds (`t3b5.log`, median of the round medians): W11 15,783,917 → 16,412,690
  (**+4.0 %**, every round higher), W27 9,758,634 → 9,670,125 (−0.9 %, ranges overlapping: 9.66–9.77 M against
  9.58–9.75 M). Kept with 256 bytes (`PREFETCH_BYTES`), as S3 asks for a measured gain and W27's change is within the
  spread of its rounds; the complete run (T4) checks the other loads. New test
  `a_prefetch_never_fails_and_changes_nothing` (header pages, pages of the store, past its end and `u64::MAX`, on
  disk and in memory). Checks: fmt, clippy clean, 831 passed, 0 failed, 34 ignored.
- **L7** (2026-11-13): T3 committed (`44b1094`); RDD assess `0257204..44b1094` medium, `under_budget`. T4: complete
  Krummi-and-Voron run on Windows at `44b1094` under the bench power plan, 08:22:43–08:46:17
  ([report](../../docs/benchmark/2026-11-13-windows-range-step.md); its header says «con cambios sin confirmar»
  because this document and the phase log were uncommitted: the code is `44b1094`'s). **29 of 29** thresholds.
  Against the previous final run at `6ffad00` (Krummi then, Krummi now, ratio then → now): `intervalos` 14,766,825 →
  16,400,071 (+11.1 %, 0.90 → **0.98**); `tabla-intervalos` 9,060,425 → 9,763,295 (+7.8 %, 0.86 → **0.91**);
  `recorrido` 17,721,447 → 22,933,518 (+29.4 %, 1.88 → 2.22). Drops over 5 %: `lecturas-al-azar-16` −19.0 % (Voron
  −10.7 % in the same run), `escrituras-sueltas` −6.5 %, `escrituras-concurrentes-4` −5.7 %,
  `escrituras-concurrentes-16` −5.0 %, `apertura` −12.5 % (64 → 56 op/s). None of them scans; they read through
  `node::value`, now `entry(..).1`. Checked with single loads, `0257204` against `44b1094`, three rounds, pinned,
  under the plan (`work/w11/w4w9.log`, round medians): W4 8,372 / 8,507 / 8,586 against 8,488 / 8,494 / 8,708; W9
  3,173,957 / 3,334,675 / 3,571,172 against 3,341,116 / 3,442,158 / 3,641,233: no loss, so the drops of the complete
  run are its variation between runs.
- **L8** (2026-11-13): RDD review of the last slice `0257204..0d99b0e` (assess medium, `under_budget`, 501 lines;
  requested as the run's last slice, CLAUDE.md), lenses `review-risk` and `review-reliability`, lineage
  `review-8fd0dd68fb12ca45`: **approved**, acknowledged (authority burned). Findings: risk none; reliability one,
  R3-prefetch-slice-bound (SUGGESTION, `pager.rs:414`): «Pager::prefetch slices the page bytes up to PREFETCH_BYTES
  (256) without a check. [...] A compile-time assertion would make the promise hold by construction. It holds today,
  so there is no current harm.» Fixed: `const _: () = assert!(PREFETCH_BYTES <= PAGE_SIZE);` (with 9,000 the build
  fails with «assertion failed: PREFETCH_BYTES <= PAGE_SIZE»; with 256 it builds). Envelopes in
  `work/w11/rdd-rs-*.json`. Checks after the fix: fmt, clippy clean, 831 passed, 0 failed, 34 ignored.
- **L9** (2026-11-13): RDD fix committed (`a3527a6`), reviewed alone from the reviewed boundary `0d99b0e` (assess
  medium, `under_budget`, 14 lines), lens `review-reliability`, lineage `review-4148cf1268b680aa`: **approved**, no
  findings, acknowledged. The branch also holds `03cf21f` (docs(tables): rename the JSON multi-valued index to
  multi-entry index), committed at 08:13 by the other session that works on the JSON design documents in this copy,
  while this branch was checked out: not part of this run, left as is; RDD and Codex reviewed it within the range.
- **L10** (2026-11-13): Codex code review of the run, round 1, `0257204..a3527a6`
  ([report](../../docs/reviews/range-step/2026-11-13-code.md)): `approved`, no findings, no product questions
  («No findings. No introduced defect or substantive unmet requirement was established in the requested range.»);
  its checks: fmt, clippy, `cargo test --locked --offline --workspace` passed. Thread
  `01a1d425-0c4c-70a1-902b-81dec4e7158f`; telemetry: coordinator `gpt-6.1-sol` / high, reviewer
  `krummi-review-critical` `gpt-6-astra` / xhigh. No resolutions file: nothing to resolve.
- **L11** (2026-11-13): WSL 2 single loads (S5, L3), `0257204` (k0) against `7cf21b8` (kh, same code as `a3527a6`)
  and Voron, alternating, three rounds, not pinned (`~/w11prof/wsl-final.out`, 09:13:58–09:18:02). Only round 1 is
  usable: in rounds 2 and 3 all three binaries fell at once (W11 under 3.4 M, Voron's W27 4.2–8.4 M) while Windows
  showed 31–41 % processor time with none of this run's work going (another workload on the machine). Round 1,
  median op/s: W11 k0 18,041,675, kh 21,499,711, Voron 21,653,710 (ratio 0.83 → **0.99**); W27 k0 13,490,968, kh
  14,533,666, Voron 12,922,036 (1.04 → 1.12). One round on a busy machine: indicative only; the Windows complete run
  (L7) is the run's measurement.
- **L12** (2026-11-13): closing report [range-step-report.md](range-step-report.md). Active time 1 h 41 min against
  the 2–4 h estimate (phase log). Not merged nor pushed: the branch waits for the user's review.
