# Collation of text columns PRD review resolutions

Resolutions of [2026-11-23-prd.md](2026-11-23-prd.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-23

Artifact versions after the fixes (uncommitted working tree, SHA-256 of the bytes):

- `docs/prd/krummi/tables/tables-collation.prd.md`: `d9deea30c7410ed981f9e4b3468b3838b6ce5f9a2511270ce18a3fc210c769ce`
- `docs/prd/krummi/tables/tables.prd.md`: `0825134c12733f13061e97fc8885d0ee5fee972dabbee2ade8f165a663441d2b`
- `docs/prd/krummi/tables/tables.prd.en.md`: `107fe61f27838d119951f99ebae8c73f1767dd9f4c340a0ea5079284f6498c0d`
- `docs/prd/krummi/tables/tables-schema.prd.md`: `551fc7f9e39790638aa7efa39781ab6226969f27851bfad9ef68afefc0414a4b`
- `docs/prd/krummi/tables/tables-schema.prd.en.md`: `f79f75007d630f412878df9890a93cb811c91f3ec920a255126cab099571536d`
- `docs/prd/krummi/tables/tables-indexes.prd.md`: `9333dd7618b0a0c3be857e29bcb7d4339676defa1c6f2cb2b4176c0b9d959ae5`
- `docs/prd/krummi/tables/tables-indexes.prd.en.md`: `27aff438e53a1ae659fc43bb92959eb62e53d1f8fbc662bdfed3f4cec2a1ea01`
- `docs/prd/krummi/tables/tables-statistics.prd.md`: `9ad47955a8efe436ef8e5874adf85f61316e1a09cb02dd27ff53ffc34d943a5c`
- `docs/prd/krummi/tables/tables-statistics.prd.en.md`: `08610f23483abc0f501e9c6ab30d82e8ae42f1c0ceac14d16adc5cfac0bcccfd`
- `docs/prd/krummi/tables/tables-projection.prd.md`: `73a0cd57925d13f2ae410925a2ac6686076502719aab8d446cd768989a785278`
- `docs/prd/krummi/tables/tables-projection.prd.en.md`: `e7c0a7e12f617a675a92effcac37678acb3b3d35ac8bd534c41d4fee7947563e`
- `docs/prd/krummi/tables/tables-json.prd.md`: `c8e0bb7ed8cea4b026999344201c77a9cca985912c15540e7f26b9da6e9c4460`
- `docs/prd/krummi/tables/tables-json.prd.en.md`: `ad4da60cfe72f2385aabfb2b02d77ab33dfc16a254a87988d0dbf6143aa3db2a`
- `docs/design/tables/03-keys-and-indexes.md`: `98497b0bd710cb8228aa0a923e12d63dab2beae435f0427c23f248e7fe116ac5`
- `docs/design/tables/README.md`: `3b878e066dd68be5ff8b9c496a32874b0537122e6fb6133ffbf32efdc46e5f54`

### R1 — major: AC-IC4 contradicts IC6 about primary-key spelling in an update error

- Disposition: fixed.
- Fix: AC-IC4 now requires the texts of the colliding row as they would be stored (IC6), and adds the case the
  report describes: updating the row whose key is `Ana` through `ANA` while giving the index column a value
  equivalent to another row's fails with duplicate key whose primary key is `Ana`, the one the row keeps (IC4).
- Status: claimed fixed, unverified until a follow-up review.

### R2 — minor: The claimed cross-document annotations are absent

- Disposition: fixed.
- Fix: dated 2026-11-23 annotation blocks, linking to `tables-collation.prd.md`, before *Estado de partida* /
  *Starting state* of each affected PRD and its translation: `tables` (N2, N4, N5, split, out of scope),
  `tables-schema` (E2, E4, E5, E6, E10–E13, E17, E18, E20), `tables-indexes` (IX7–IX11, out of scope),
  `tables-statistics` (ST1, ST2, ST16–ST18), `tables-projection` (PY2) and `tables-json` (AC-JS12). Design
  document 3 carries an annotation after *Strings and collation* and on product question 5, and the design README
  one on the column types of the first version.
- Status: claimed fixed, unverified until a follow-up review.

Review record: [2026-11-23-prd.md](2026-11-23-prd.md), wrapper status `findings` (R1, R2), thread
`01a20747-4003-7801-93f5-0cbd50d7287b`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.

## 2026-11-23 (verification)

Follow-up report: [2026-11-23-prd-02.md](2026-11-23-prd-02.md), wrapper status `approved`, thread
`01a2074e-bb20-7f01-95b5-2fe239d5434a`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer `krummi-review`
`gpt-6.1-sol` / `high`.

- R1: "Major, verified resolved: AC-IC4 agrees with IC6 about primary-key spelling".
- R2: "Minor, verified resolved: the cross-document annotations are present".
- Verdict: "Ready for implementation within the requested follow-up scope: R1 and R2 are verified resolved, with no
  open findings or correction-induced regressions established."
