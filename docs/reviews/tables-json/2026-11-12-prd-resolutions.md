# Tables PRD (JSON annotation) review resolutions

Resolutions of [2026-11-12-prd.md](2026-11-12-prd.md), the review of `docs/prd/krummi/tables/tables.prd.md` with its
annotation of child PRD 7.5 of 2026-11-12 and the readability restructuring of its annotations. Append-only: each
update is dated and keeps the earlier ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-12

Artifact versions after the fixes (uncommitted working tree, SHA-256 of the bytes):

- `docs/prd/krummi/tables/tables.prd.md`: `894a4417b471bba40006dea7142cef2875b18ee401512ba944b3edbe4ce327d4`
- `docs/prd/krummi/krummi.prd.md`: `792939f15881399437b8c349a478aded63aaceed5a285f2a05737abc0343a9a5`

Review record: `docs/reviews/tables-json/2026-11-12-prd.md`, wrapper status `findings` (open R1, product question
Q1), `threadId` `01a1cf8c-1b54-7560-8a86-38d2d9a71bb0`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.

### R1 — major: Define the copy contract for decoded JSON strings

- Disposition: fixed, after the user's product decision on Q1.
- Verification before the fix: the annotation promised "un texto, de una cadena" while root D11 and AC20, and the
  tables PRD's *Salida*, list JSON among the values returned without copying; in the normalized text an escaped
  string such as `{"x":"a\nb"}` keeps the escape, so its decoded bytes are not contiguous in engine memory.
- Product decision (Q1): the user was offered three readings, a copy only for strings with escapes, a copy always,
  and a view without copying that decodes on the fly, and chose the first: «A».
- Fix: in the 7.5 annotation, a string read as text is returned without copying when it has no escapes and decoded
  in the application's own copy when it has them, an exception to D11 and AC20 like that of the other decoded values
  of *Salida*; the whole document is always read without copying. *Salida* carries a dated annotation with the same
  rule. *Anotaciones a otros PRD* declares the change to root D11 and AC20, and the root PRD carries its annotation of
  2026-11-12 (six added lines, nothing else changed in it).
- Status: claimed fixed, unverified until a follow-up review.

The English translations (`tables.prd.en.md`, `krummi.prd.en.md`) are updated after the review, as the PRD workflow
sets.

## 2026-11-12 (verification)

Follow-up report: [2026-11-12-prd-02.md](2026-11-12-prd-02.md), wrapper status `approved`, `threadId`
`01a1cfc3-796e-70d1-bc94-402553e5eedb`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`. Verdict, quoted: "Approved — R1 verified resolved; the PRD is
ready for implementation within the reviewed follow-up scope."

- R1: "verified resolved".

No product question remains open.
