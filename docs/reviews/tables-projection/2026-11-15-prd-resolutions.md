# Tables projection PRD review resolutions

Resolutions of [2026-11-15-prd.md](2026-11-15-prd.md), the review of `docs/prd/krummi/tables/tables-projection.prd.md`.
Append-only: each update is dated and keeps the earlier ones. A claimed fix stays unverified until a follow-up review
confirms it.

## 2026-11-15

Artifact version after the fixes (uncommitted working tree, SHA-256 of the bytes):

- `docs/prd/krummi/tables/tables-projection.prd.md`: `297a5f2a678f85037de82bade1cf14b353144c40319a5c460300899076366e2c`

Review record: `docs/reviews/tables-projection/2026-11-15-prd.md`, wrapper status `findings` (open R1, product question
Q1), `threadId` `01a1de9b-e5ea-70e0-a440-6f92d203afbc`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.

### R1 — major: Contrato incompatible de propiedad y préstamos para filas copiadas

- Disposition: fixed, after the user's product decision on Q1.
- Verification before the fix: PY3 promised for every row «se devuelven igual: sin copia el texto, los bytes y los
  vectores», and *Salida* «con la forma y los préstamos de leer la columna por su posición». For a copied row, reading
  by position returns an owned copy: `TableRow::into_owned` keeps owned values (`crates/krummi/src/table/rows.rs:991`)
  and `value` clones them (`rows.rs:943`), so the two promises disagree, as the report says.
- Product decision (Q1): the user was offered an owned copy as today's reads by position give, which may outlive the
  copied row, or a borrow from the copied row, and chose the first: «A».
- Fix: PY3 says that from a copied row every value is returned as reading it by position does today, in the
  application's own copy that may outlive the copied row; *Salida* separates the two cases (no copy from a row read or
  traversed, an own copy from a copied row); AC-PY10 checks that the copied row's text, bytes and vectors stay valid
  after the copied row is dropped.
- Status: claimed fixed, unverified until a follow-up review.

The annotations of *Anotaciones a otros PRD* and the English translation are applied after the review, as the PRD
workflow sets.

## 2026-11-15 (verification)

Follow-up report: [2026-11-15-prd-02.md](2026-11-15-prd-02.md), wrapper status `approved`, `threadId`
`01a1deaa-f740-7782-836c-9fd9ecdc2465`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`. Verdict, quoted: "Ready for implementation within the requested
follow-up scope; R1 is verified resolved."

- R1: "Verified resolved: copied-row output ownership and lifetime".

No product question remains open.
