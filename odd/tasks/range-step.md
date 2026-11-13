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
| T2 | S2, S4 | inline | done (L5) | (this commit) |
| T3 | S3, S4 | inline | pending | |
| T4 | S5 (complete run, closing report) | inline | pending | |

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
