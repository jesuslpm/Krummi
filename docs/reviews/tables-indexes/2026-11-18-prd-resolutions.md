# Tables secondary indexes PRD review resolutions

Resolutions of [2026-11-18-prd.md](2026-11-18-prd.md) and its rerun [2026-11-18-prd-02.md](2026-11-18-prd-02.md).
Append-only: each update is dated and keeps the earlier ones. A claimed fix stays unverified until a follow-up review
confirms it.

## 2026-11-18

Review records:

- [2026-11-18-prd.md](2026-11-18-prd.md): wrapper status `violation`. Another session working in the same copy
  changed `odd/tasks/night-optimizations-phases.tsv` and `odd/tasks/night-optimizations.md` during the review.
  Nothing was reverted, and the round does not count. `threadId` `01a1ed91-b250-7d70-ad01-c58bee4e3da2`; telemetry:
  coordinator `gpt-6.1-sol`/`high`, reviewer `krummi-review-critical` `gpt-6-astra`/`xhigh`.
- [2026-11-18-prd-02.md](2026-11-18-prd-02.md): round 1 rerun on a clean tree at `d7fe899`, wrapper status
  `findings`, verdict «Not ready for implementation: R1 and R2 remain open.» `threadId`
  `01a1efac-5209-7b51-8125-acb39cc16e6e`; telemetry: coordinator `gpt-6.1-sol`/`high`, reviewer
  `krummi-review-critical` (Gauss) `gpt-6-astra`/`xhigh`.

The fixes are in the commit that adds this file.

### R1 — major: included primary-key columns have conflicting ownership contracts

- Disposition: fixed, after a product decision.
- Decision: the user answered Codex's open question with «No falla, pero no hace nada, la clave primaria siempre
  está incluida».
- Fix: IX1 now says primary-key columns given among the included columns are accepted and do nothing. The stored
  definition omits them, Describir does not list them among the included columns, and Crear si no existe compares
  definitions without them. An entry returns them as primary-key columns. IX3's rules are checked first, on the
  definition as given. IX2's same-definition rule says «sin las de la clave primaria». IX8 now says the trailing
  primary-key columns are those «que no están en la clave del índice». AC-IX3 expects a text primary-key column given
  among the included columns to come back once, in its primary-key position and in a copy. AC-IX9 adds Crear si no
  existe over two definitions that differ only in included primary-key columns. The context paragraph quotes the
  decision.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — minor: announced amendments are absent from the affected PRDs

- Disposition: fixed.
- Fix: dated 2026-11-18 annotation blocks, linking to `tables-indexes.prd.md` and naming the IX decisions, now sit
  before the starting-state heading of `krummi.prd.md` (D11, AC20), `journal.prd.md` (J2, J8, J14), `pager.prd.md`
  (P3, P13), `tables.prd.md` (N5, N10, Salida), `tables-schema.prd.md` (E2, E5, E6, E16, E17, E19),
  `tables-row-reads.prd.md` (L1, L2, L3), `tables-projection.prd.md` (PY3) and `benchmark.prd.md` (loads, K4, K7,
  Salida, K9). Another block sits before `## Flujo` in `krummi.prd.md` (G6). Each `*.prd.en.md` translation carries
  the same blocks in English.
- Status: claimed fixed, unverified until a follow-up review.
