# JSON columns PRD review resolutions

Resolutions of [2026-11-21-prd.md](2026-11-21-prd.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-21

Artifact versions after the fixes (uncommitted working tree, SHA-256 of the bytes):

- `docs/prd/krummi/tables/tables-json.prd.md`: `2d51dfcfaae1c02d5e4de1f828dc742c08dc26758c207b7c42396d4a6ed105db`
- `docs/prd/krummi/tables/tables.prd.md`: `b580f8fda30609a181cdf17f7eff8dc39502985cb13f37da9f5d0adf69f52b5e`
- `docs/prd/krummi/tables/tables-schema.prd.md`: `9d8f1bd96db3828b83973da51a64f8ad363a6c817ac634341124cd3f556b2d2a`
- `docs/prd/krummi/tables/tables-projection.prd.md`: `1172e56fab368feb4f57476529ad6bdd06ba849269a18c9e0d3f4639f1d6aca8`
- `docs/prd/krummi/tables/tables-statistics.prd.md`: `cad1faf48054c6f5089eaaa93918ec30eb08af8c96c62d2cfd1e5572ca8cef89`
- The English translations of the four ancestor and sibling PRDs carry the same annotations.

Review record: [2026-11-21-prd.md](2026-11-21-prd.md), wrapper status `findings` (R1, R2), `threadId`
`01a1fce8-a460-7c00-842a-9d24a2c87e96`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.

### R1 — blocker: AC-JS7 requires incompatible backing memory for an included column

- Disposition: fixed.
- Fix: JS5 now names the memory each read route borrows from: the row, when read by its key, traversed or read with
  a projection, and the index entry, without reading the row, when read as an included column (IX8). AC-JS7 asserts
  that the document and an escape-free string point into the row read for the first three routes, and into the index
  entry, without reading the row, for the included column, checked as the tests of 7.1 and 7.2 check that `string`
  text is not copied.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — minor: the claimed cross-document annotations are absent

- Disposition: fixed.
- Fix: `tables.prd.md`, `tables-schema.prd.md`, `tables-projection.prd.md` and `tables-statistics.prd.md`, and their
  `*.prd.en.md` translations, carry a dated 2026-11-21 annotation block before their starting state, linking to
  `tables-json.prd.md` and naming the JS decision behind each note: N3, N4, N5 and the 7.5 split annotation; E4, E5,
  E6, E17 and E20; PY2, PY3 and Out of scope; ST1, ST12, ST18 and Out of scope.
- Status: claimed fixed, unverified until a follow-up review.

## 2026-11-21 (verification)

Review record: [2026-11-21-prd-02.md](2026-11-21-prd-02.md), wrapper status `approved`, `threadId`
`01a1fcef-0431-7cd3-b677-9caad9c62220`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`. Verdict: "Ready for implementation within the reviewed follow-up
scope; R1 and R2 verified resolved."

- R1: "Incompatible backing memory for included JSON — Verified resolved".
- R2: "Missing cross-document annotations — Verified resolved".

## 2026-11-21 (after approval)

Wording edit after the approved round, outside any finding: AC-JS8 named the default-value error "la regla valor
por defecto no válido"; it now uses the rule of E5 that JS9 and *Validaciones y mensajes* already cite ("un valor
por defecto que no cumple las reglas de su columna"). No rule changed. The English translation
`docs/prd/krummi/tables/tables-json.prd.en.md` was written after the approval and carries the same edit.
