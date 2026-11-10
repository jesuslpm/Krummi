# Tables PRD (ANN scope) review resolutions

Resolutions of [2026-11-10-prd.md](2026-11-10-prd.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-10

Reviewed revision: `8aa74da` (child PRD 7.6 annotation). Fix commits: `883c4ab` (R1), `2353251` (R2).

Review record: `docs/reviews/tables-ann/2026-11-10-prd.md`, wrapper status `findings` (open R1, R2), `threadId`
`01a1c6b2-0584-73c0-804b-90570b0cba97`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`. No product question was raised.

### R1 — major: JSON reads conflict with the common ownership contract

- Disposition: fixed.
- Evidence: the 7.5 annotation (`tables.prd.md`, *Reparto en PRD hijos*) stores JSON in a binary format "que se lee
  sin copiar ni analizar el texto", while *Salida* and the root's D11/AC20 annotation listed only `string`, `bytes`
  and vector as values returned without a copy. The user's choice in 7.5 already fixes the behavior, so the fix
  propagates it; it decides nothing new.
- Fix: *Salida* of `tables.prd.md` and `tables.prd.en.md` adds JSON (from 7.5) to the values read without copying;
  the D11/AC20 annotation of `krummi.prd.md` and `krummi.prd.en.md` excludes JSON from the decoded copies and keeps
  it among the references without a copy.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — minor: starting state generalizes normal-tree limits to all tree kinds

- Disposition: fixed.
- Evidence: fixed-size trees take `u64` keys (`crates/krummi/src/lib.rs:633`) and values of a fixed length of 0 to
  2,048 bytes (`crates/krummi-tree/src/fixed.rs:1-2`); a multi-value tree caps each value at the key maximum
  (`crates/krummi/src/lib.rs:69`); B5 of `trees.prd.md` limits the 1 to 2,048-byte keys to normal and multi-value
  trees.
- Fix: the *Datos* row of *Estado de partida*, in both languages, now gives the key and value limits per tree kind.
- Status: claimed fixed, unverified until a follow-up review.

## 2026-11-10 (verification)

Follow-up review [2026-11-10-prd-02.md](2026-11-10-prd-02.md) over `8aa74da..ef4bc7c`, wrapper status `findings`
(open R1), `threadId` `01a1c6bb-2f81-75c2-b888-4ed4a01e51be`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`. Verdict, quoted: "Ready for implementation with a minor
documentation correction outstanding: R1 remains open; R2 is verified resolved."

- R1: "still open, partially resolved; severity reduced to minor". The edits to *Salida* and the root D11/AC20
  annotation were verified; the umbrella's own annotation summary (*Anotaciones a otros PRD*) still excluded only
  `string`, `bytes` and vector.
- R2: verified resolved.

## 2026-11-10 (round 2 fixes)

Fix commit: `0b19078`.

### R1 — minor: JSON ownership annotation still contradicts the corrected output

- Disposition: fixed.
- Fix: the "Padre raíz, D11 y AC20" item of *Anotaciones a otros PRD*, in `tables.prd.md` and `tables.prd.en.md`,
  now excludes JSON (from 7.5) from the decoded copies. A search of every PRD under `docs/prd/krummi/` for the old
  list (`` `bytes` y vector ``, `` `bytes` ni vector `` and their English forms) finds no other occurrence.
- Status: claimed fixed, unverified until a follow-up review.

## 2026-11-10 (verification, round 3)

Follow-up review [2026-11-10-prd-03.md](2026-11-10-prd-03.md) over `ef4bc7c..3ecffca`, wrapper status `approved`,
`threadId` `01a1c6c0-dcba-7cf0-9ebd-683881ecbb0d`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review` `gpt-6.1-sol` / `high`. Verdict, quoted: "Ready for implementation within the reviewed scope; no
findings remain open."

- R1: verified resolved.
- R2: verified resolved in round 2.
