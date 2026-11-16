# Write margin: W19 located write, W28 version lookups

Objective: close part of Krummi's gap to Voron in W19 `multivalor-carga` and W28 `tabla-actualizacion` without
dropping any check. Branch `feat/write-margin` from `main` `6b49bea`. Delivery: one commit per task on the branch, no
merge or push until the user asks. Tests: `cargo test --workspace`; acceptance with the bench worker and
`--solo-krummi-voron` runs.

## Specs

- **S1** — Measurement before any W28 change (the user's "esa medición"): count, for every leaf and branch read of
  W28 at full scale, which check the read pays: integrity remembered, full integrity check of the page, layout check
  remembered, full layout check, a scratch-area version already checked, a version checked for the first time, and a
  page the transaction changed. The counts go in the log; they decide whether W28-A and W28-B (out of scope here)
  are worth a PRD, and which parts of S3 apply. The probe is not committed.
- **S2** — W19-A: adding a value to a key whose values are inline in its leaf writes the new entry in the leaf that
  the lookup found, without walking the tree again, when the new entry fits in that leaf and the leaf stays at least
  a quarter full or is the root; otherwise the add goes through `set` as today. Pages, roots and results are the same
  bytes as before (no change of behavior or stored format).
- **S3** — W28-C: the scratch area finds the versions of a page with a hash suited to page numbers instead of the
  default SipHash; and, when S1 shows that versions read for the first time are frequent in W28, the layout check
  of such a version is remembered without taking the scratch lock and looking the version up again. No change of
  behavior or stored format.
- **S4** — Acceptance: an A/B of Krummi alone, `main` against the head, of W19 and W28 and of the loads that share
  the changed code, shows no load slower; complete `--solo-krummi-voron` runs on Windows and Linux meet 29 of 29
  thresholds.

Out of scope (decided with the user on 2026-11-16): W28-A (remember a page written by the transfer, candidate 7 of
the performance plan) and W28-B (keep the layout check of a leaf whose value changed in place), which change
guarantees and need a PRD first; a cache of recent leaves.

## Tasks

| ID | Specs | Route | Status | Commit / evidence |
|---|---|---|---|---|
| T1 | S1 | inline (probe in a copy under `work/`) | done | L3, probe not committed |
| T2 | S2 | inline | pending | |
| T3 | S3 (hash) | inline | pending | |
| T4 | S3 (first check of a version) | inline, if S1 supports it | pending | |
| T5 | S4 | inline, runs in the background | pending | |
| T6 | — | close: RDD of the last slice, Codex review, report, archive | pending | |

Routes: every task is one understood change of at most a few hundred lines, with no Writer trigger reason
(no parallel units), so the parent writes inline.

## Log

- **L1** (2026-11-16) — The user's request, verbatim: «Entonces veamos esas mejoras.» Then, to «¿Arranco con W19-A,
  W28-C y esa medición?»: «Sí»; and to the estimate: «Sí».
- **L2** (2026-11-16) — Evidence before the run: two read-only explorations of the code and the diagnostic
  `work/diag-20261115/informe-rendimiento.md`. W19: an add descends twice (`multi::entry`, then `set`), Voron's
  `MultiAdd` writes in the page it found. W28: one descent per update (1,001,000 descents for 1,000,000 lookups in
  the diagnostic's probe); the lookup costs more because about 97 % of updates reach a leaf changed since its last
  read, which is checked again (integrity of 8 KiB and layout) on a page out of the caches. Estimate 3 h – 5 h,
  start 2026-11-16 11:11:02 +0000. An untracked `docs/prd/krummi/tables/tables-indexes.prd.md` appeared in the
  tree, not part of this run: it is left untouched and never staged.
- **L3** (2026-11-16) — T1 (S1). Probe in a copy of `main` under `work/wm/probe` (not committed): counters in
  `node_at_with` by check and node kind, and in `Pager::run_with` by integrity path; W23 at full scale, then W28 at
  full scale from its store (`work/wm/w28b.err`). The bench reads every row back after each measured part, so the
  counters are taken and reset at the end of each measured part; the first repetition carries no earlier read-back
  (the later ones match it once the read-back is taken off). Per measured update (1,000,000): 2 branch reads, all
  with the integrity and the layout remembered; 1 leaf read: 54 % a scratch-area version read for the first time
  (full layout check, then the second lock and lookup to remember it), 43 % a page of the data file not remembered
  (xxh3 of 8 KiB and a full layout check), 3 % a page the transaction changed, 0.1 % remembered. So the second lock
  and lookup of S3 apply to about half of the updates: T4 goes ahead. For the PRD question left out of scope:
  W28-A would remove the 8 KiB hash from 43 % of the updates, W28-B the layout check from 97 %.
