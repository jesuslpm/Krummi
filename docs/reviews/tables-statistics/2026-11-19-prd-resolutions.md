# Tables statistics PRD review resolutions

Resolutions of [2026-11-19-prd.md](2026-11-19-prd.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-19

Artifact versions after the fixes (uncommitted working tree, SHA-256 of the bytes):

- `docs/prd/krummi/tables/tables-statistics.prd.md`: `cfe3370863ba506bbd3e0535ba7f818f2699aa3cf7ca6a545efdf18b3cfe9855`
- `docs/prd/krummi/krummi.prd.md`: `705aa5f796f96d7521c443cf8a4d0e14da77d6e2fa1611f4032e120a10df4026`
- `docs/prd/krummi/krummi.prd.en.md`: `130ac6658f9586e8ccdd644f717a348c71c6589b41d8409199395e0ed44da7f3`
- `docs/prd/krummi/tables/tables.prd.md`: `d816bff9d2576c4f915824866a51e90f08695e6e830339fcc438774f28046c95`
- `docs/prd/krummi/tables/tables.prd.en.md`: `7f354c50cce633aec7dadcb1698fe597fb1b79f6fa3737f06c6e708edb066b46`
- `docs/prd/krummi/tables/tables-schema.prd.md`: `1920de110d033637153927dfc383f2623ad34a5d8d59284191174e0f87ef8951`
- `docs/prd/krummi/tables/tables-schema.prd.en.md`: `2e1accd71cf4287a23347c4b28406d2b3397a2d623969926b22b7fd334dcd2e0`
- `docs/prd/krummi/transactions/transactions.prd.md`: `c957a03ddf3bd0329d773a8fcea7ae0b0028495ad694d09f4f17970bb10349ee`
- `docs/prd/krummi/transactions/transactions.prd.en.md`: `8f6c2b8e6a07e3d0ffee006d5164d87234bfa5cfb6db4cc93fe386fc735d6781`
- `docs/design/tables/03-keys-and-indexes.md`: `a8ab0a56b28a85c3e033bbbef2f3fbe32c8f88f6a860d29ff6b0074f448122b0`
- `docs/design/tables/04-query-tree-and-planner.md`: `32467665666f93dee57198d0099c603be361e2eb0e950a7726237ab0ad9881c2`
- `docs/design/tables/10-agent-tooling.md`: `ee6837d5f17243376d6d67e0f1a8958046a2c8fb185fbf36078e3d5d4341b702`

### R1 — blocker: Analyze must publish statistics and also fail without effects

- Disposition: fixed, after the user's product decision. Asked which effect survives a store-level analyze that
  fails after sampling, the user chose «Se queda en memoria».
- Fix: ST7 says that when analyze on the store fails after sampling (waiting for the turn or saving), what it
  published stays in memory, as in ST9: the store does not change and the next estimate that uses those statistics
  asks for them to be saved. AC-ST9 now says each failure leaves the store unchanged, rather than "sin efecto". It
  adds that a store-level analyze failing after sampling leaves its publication in memory, and that a later estimate
  uses it without sampling.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — major: Estimate value-size limits inherited from IX9 are ambiguous

- Disposition: fixed, after the user's product decision. The user said «Los valores grandes a partir de un cierto
  umbral no deberían muestrearse. Si quieres condiciones de igualdad mejor añadir una columna de hash», then agreed
  («Sí») to the rule below.
- Fix:
  - ST2 says that, unlike IX9, estimate values have no size limit: any value valid for its column (E18) is accepted,
    and no estimate fails with tamaño no válido because of its values.
  - ST1 defines large values as `string` or `bytes` over 2,048 bytes, the key maximum of B5. Sampling does not read
    them, and statistics keep their fraction outside frequent values and the histogram.
  - ST2 states how estimates treat large values. An equality with a large value estimates their fraction, an upper
    bound. An equality with a short value does not count them. An interval with a bound always adds their fraction.
  - ST16 reads the fraction of large values.
  - AC-ST12 covers 2,049-byte values, a 4,096-byte equality that does not fail and a 2,048-byte equality. It also
    checks that sampling reads no pages of large values, through the new page-count hook in ST14.
  - Multi-column equality (ST18) applies the same per-value rule. Index prefixes never hold large values, because
    index entries are limited to 2,048 bytes.
- Status: claimed fixed, unverified until a follow-up review.

### R3 — minor: Starting state omits an existing thread and a row-count mutation path

- Disposition: fixed.
- Evidence: `crates/krummi-pager/src/journal.rs:592` names the `krummi-journal` thread.
- Fix: *Estado de partida* now says the store has two threads of its own, the journal's and the data-file transfer,
  and that closing waits for both. It also lists emptying a table as a second path that sets the row count directly,
  besides the append fast path.
- Status: claimed fixed, unverified until a follow-up review.

### R4 — minor: Claimed upstream annotations are absent

- Disposition: fixed.
- Fix: the annotations are now applied, dated 2026-11-19 and linking to `tables-statistics.prd.md`, in the
  Spanish PRDs and their English translations:
  - `krummi.prd.md` / `.en.md`, after the guarantees: exception to G3 and AC6 for the shared statistics in memory.
    The PRD's annotation list now names it too.
  - `tables.prd.md` / `.en.md`, after the decisions: N8, *Flujo* step 5, AC-N8, the 7.3 row of the split, and
    *Fuera de alcance* on several-column statistics.
  - `tables-schema.prd.md` / `.en.md`: E3, E16, E17, E20.
  - `transactions.prd.md` / `.en.md`: T4, T12.
  - Design documents 3 (*Statistics*), 4 (combining equalities) and 10 (C2 and question 7), before their *Costs
    and risks* or *Sources* sections.
- Status: claimed fixed, unverified until a follow-up review.

### R5 — minor: AC-ST3 cannot observe the truncate increment through staleness

- Disposition: fixed.
- Fix: AC-ST3 no longer checks emptying through staleness. It checks it through ST16: emptying an analyzed
  400-row table and analyzing it again gives an origin counter 400 higher than before.
- Status: claimed fixed, unverified until a follow-up review.

### Other changes in this round

- ST3 now says that the sample is read like a read transaction that blocks nobody, and that only the later short save
  asks for the write turn. An application write that estimates keeps holding its own turn while it waits for the
  sample. This follows the user's remark: «El muestreo muestrea en una transacción de lectura y cuando tiene ya los
  datos estadísticos empieza una transacción de escritura que es cortita».

Review record: [2026-11-19-prd.md](2026-11-19-prd.md), wrapper status `findings` (R1–R5 open), thread
`01a1f352-1cff-7b70-a940-1c0f77d62d30`. Telemetry: coordinator `gpt-6.1-sol`/`high`; reviewer
`krummi-review-critical` `gpt-6-astra`/`xhigh`.

## 2026-11-19 (verification)

Follow-up report: [2026-11-19-prd-02.md](2026-11-19-prd-02.md). Verdict, quoted: "Ready for implementation within the
reviewed follow-up scope." Findings, quoted: "No open findings remain in the requested scope. R1–R5 are verified
resolved at the versions above. No supported regression finding was retained."

- R1: verified resolved.
- R2: verified resolved.
- R3: verified resolved.
- R4: verified resolved.
- R5: verified resolved.

Review record: [2026-11-19-prd-02.md](2026-11-19-prd-02.md), wrapper status `approved`, thread
`01a1f35d-9182-7f43-b14e-0b7d11afd38c`. Telemetry: coordinator `gpt-6.1-sol`/`high`; reviewer
`krummi-review-critical` `gpt-6-astra`/`xhigh`.
