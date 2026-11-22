# Temporal values in microseconds PRD review resolutions

Resolutions of [2026-11-22-prd.md](2026-11-22-prd.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-22

Artifact versions after the fixes (uncommitted working tree, SHA-256 of the bytes), under `docs/prd/krummi/tables/`:

- `tables-temporal-micros.prd.md`: `236e346a42660123ee74a22195420cce030359a3e7cde6d65530b5d94c5a75e6`
- `tables.prd.md`: `f0dcae2df40fbd8e78dcaa76cdb8d2eec04080fd34979a6add5d9b4ed5efe36d`
- `tables.prd.en.md`: `da086fa509c7ed3d5929e5e9e8cae1396a8130bcd477bdbc7df9388b7153058c`
- `tables-schema.prd.md`: `46f2aeac9d0482b930182536e6a7e7988011a13395ebeb4738843253c19e283c`
- `tables-schema.prd.en.md`: `e47cdb8baf4eef1f9a163d82d186ee774cd0b29ac07c072651b696b4807a852e`
- `tables-indexes.prd.md`: `c70251aaca727f58fd4abc829d2dffd1bdcec411f6da077b83d468f819b41fa6`
- `tables-indexes.prd.en.md`: `5ff5704add58c0b02958cbf98b364c879b2fb97aaecfcfe540ba5d942ada9c83`
- `tables-statistics.prd.md`: `0949357e2293aed1b2027a6d4df8ca5c3158bc921514f6861293970ee218720f`
- `tables-statistics.prd.en.md`: `c4f7de3945319cd2f4cf5028a8f2f7ac567e02d6224417a1073a8a7fbf1da7b7`
- `tables-json.prd.md`: `3816bafe7c9ec530ae07031116de72db1cd8e70a6440cc1d75a8136663823f1c`
- `tables-json.prd.en.md`: `24594afbdf59e61432d8484885ef4cba706f3843d7139c431b88c28659c4b7dc`

### R1 — major: Migration by integer division can change previously stored values

- Disposition: fixed.
- Fix: *Flujo* now separates the two sources. A value read from Krummi before the change is a multiple of 1,000 ns,
  so dividing it by 1,000 is exact. An original nanosecond input of the application is converted as Krummi stored it,
  to the nearest microsecond with ties toward the greater value; the paragraph gives the 1,500 ns example (stored
  2 µs, truncated 1 µs) and says that a default converted by truncation differs from the stored one and gives
  different schema (E6). The statement on unchanged nanosecond defaults now excepts values equal in both units,
  such as 0.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — minor: The declared annotations in earlier PRDs are absent

- Disposition: fixed.
- Fix: dated 2026-11-22 annotation blocks, linking to `tables-temporal-micros.prd.md`, before *Estado de partida* /
  *Starting state* of each affected PRD and its translation: `tables` (N4), `tables-schema` (E8, the −500 ns
  annotation, AC6), `tables-indexes` (IX9), `tables-statistics` (ST2) and `tables-json` (JS7).
- Status: claimed fixed, unverified until a follow-up review.

### R3 — minor: AC-TU6 mixes incompatible types in its duplicate-key setup

- Disposition: fixed.
- Fix: AC-TU6 now keeps the text form for the six values, and for the duplicate key inserts each value twice in a
  table whose primary key is a column of its own type (`time`, `timestamp`, `duration`); the key shows the same text
  in parentheses, `(09:00:00.5)`, as `key_text` writes it (`crates/krummi/src/table/value.rs:690-693`).
- Status: claimed fixed, unverified until a follow-up review.

Review record: [2026-11-22-prd.md](2026-11-22-prd.md), wrapper status `findings` (R1, R2, R3), threadId
`01a201ff-9c93-7b32-83af-2d2afe7e08ff`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer `gpt-6-astra` /
`xhigh`.

## 2026-11-22 (verification)

Follow-up report: [2026-11-22-prd-02.md](2026-11-22-prd-02.md), wrapper status `approved`, threadId
`01a20205-9b53-70f1-94c7-ffd7dd6a4e7d`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer `gpt-6-astra` /
`xhigh`. Verdict, quoted: «Ready for implementation within the requested follow-up scope: R1–R3 are verified
resolved; no open findings.»
