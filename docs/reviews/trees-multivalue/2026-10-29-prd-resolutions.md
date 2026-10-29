# Trees multi-value PRD review resolutions

Resolutions of [2026-10-29-prd.md](2026-10-29-prd.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-10-29

Review record (round 1): report [2026-10-29-prd.md](2026-10-29-prd.md), wrapper status `findings` (R1, R2 open, no
open questions), thread `01a18705-f0da-7c52-8642-f0c864e59c99`. Telemetry: coordinator `gpt-6.1-sol` / `high`;
reviewer `krummi-review-critical` `gpt-6-astra` / `xhigh`.

Artifact versions after the fixes (uncommitted working tree, SHA-256 of the bytes):

- `docs/prd/krummi/trees-multivalue.prd.md`: `a7f1a23af931559f7b8b5980a3b44059aec399124992a4d532f9f3caee49bd66`
- `docs/prd/krummi/krummi.prd.md`: `8bf192e547e2e7a82565898d896ae166340342e8d23e98940d34a6a1af79348c`
- `docs/prd/krummi/pager.prd.md`: `dd9332ad64c2b1ea2278408087c33bd3cc82750662459b07782589feb356fdca`
- `docs/prd/krummi/journal.prd.md`: `c34578f48fb3e2b61b3190e486fee1a77aa022d99ac27c255e000e0f8b2729b1`
- `docs/prd/krummi/journal-group-commit.prd.md`: `899833dc5d0123123cd32756ebf91c930585f8eb5a829044fc3628f71e36f95a`
- `docs/prd/krummi/free-space.prd.md`: `8778c54fe19b2e2cbe961db645a296a91c029695001a6a19bef8da5aeac57bdd`
- `docs/prd/krummi/transactions.prd.md`: `81a0860fd79afcc0469cf6fed5814aa48d90015e5b0deddcfad2c9c984792e47`
- `docs/prd/krummi/trees.prd.md`: `aa509af347a0a42946a431f7c84d7cfc6d7b298839de34f4799bcc9df2332da7`
- `docs/prd/krummi/trees-keys.prd.md`: `5369aa8b6edf88cb5e83106929a67bce15a5441367259578842d7f433f06a635`
- `docs/prd/krummi/trees-scan.prd.md`: `0ff9ee0e9134a97fb4bc2fa7670262a9b87563f051957f8f4b22cd345cd453c2`
- `docs/prd/krummi/trees-large-values.prd.md`: `b5adf03c8e831c08732077821f6a53e0fb5c17818fea884bd889d75f355e021e`

### R1 — minor: Missing annotations in the affected earlier PRDs

- Disposition: fixed.
- Fix: each of the ten affected PRDs carries a dated 2026-10-29 annotation, placed after the 5.3 ones and linked
  to `trees-multivalue.prd.md`. Root D7; Pager P5; Journal J3; group commit E15; free space F11; transactions,
  whole document (format `8`, M14). `trees.prd.md`: B3 (M2, M4), B6 (M1), B11 (M13) and *Dejado a la
  implementación* (M3). `trees-keys.prd.md`: A1 (M4), A5 (M6), A9 (M11), A10 (M9) and A13 (M14).
  `trees-scan.prd.md`: R6 and R7 (M8, M9), R8 (M11), R10, *Salida* and AC-R14 (M14), and *Fuera de alcance* (M7,
  M8). `trees-large-values.prd.md`: V9, *Salida* and AC-V12 (M14), and *Fuera de alcance* (M1). The PRD's own
  *Anotaciones a otros PRD* list now also names the two *Fuera de alcance* annotations of 5.2 and 5.3.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — minor: The format-change rationale describes an unsupported deletion path

- Disposition: fixed.
- Fix: M14 now justifies the incompatibility with the demonstrable behavior. A format `7` engine does not know the
  multi-value type and takes its catalog entry as damaged (`crates/krummi-tree/src/catalog.rs:58`, `:71`). So it
  would misread a healthy store: Listar, and opening, deleting or renaming over that tree, would fail with damage.
  The unsupported leak scenario is removed. The format `8` decision is unchanged.
- Status: claimed fixed, unverified until a follow-up review.

## 2026-10-29 (verification)

Review record (round 2): report [2026-10-29-prd-02.md](2026-10-29-prd-02.md), wrapper status `findings` (R3 open, no
open questions), thread `01a1870d-db0a-71a2-b110-a5971f627e69`. Telemetry: coordinator `gpt-6.1-sol` / `high`;
reviewer `krummi-review-critical` `gpt-6-astra` / `xhigh`.

- R1: "Missing annotations in the affected earlier PRDs — verified resolved".
- R2: "Unsupported deletion-path rationale — verified resolved".
- New R3, a regression of the R1 fix: "The new A10 annotation overgeneralizes the conditional-change rule". Verdict:
  "Ready for implementation subject to resolving R3 (minor documentation correction)."

## 2026-10-29 (round 2 fixes)

Artifact versions after the fixes (uncommitted working tree, SHA-256 of the bytes):

- `docs/prd/krummi/trees-keys.prd.md`: `fcc7362ee0d45280946420be8c322487ba071d322dfcc1e626dd1679ef2c37e0`
- `docs/prd/krummi/trees-multivalue.prd.md`: `d033f823c7e3924f3e1139e867da6879a0e852968e785ca2ca1d3db58d9cce9d`

### R3 — minor: The new A10 annotation overgeneralizes the conditional-change rule

- Disposition: fixed.
- Fix: the A10 annotation in `trees-keys.prd.md` now names the five multi-value operations M9 makes conditional:
  Añadir, Quitar valor, Quitar clave and the two Quitar intervalo. It adds that catalog operations and open modes
  follow A10 unchanged, and that emptying a multi-value tree is always a change, as A2 says. The matching line of
  the PRD's *Anotaciones a otros PRD* list is scoped the same way.
- Status: claimed fixed, unverified until a follow-up review.

## 2026-10-29 (verification, round 3)

Review record (round 3): report [2026-10-29-prd-03.md](2026-10-29-prd-03.md), wrapper status `approved` (no open
findings or questions), thread `01a18713-b7b7-7e63-bea4-10c3f7e407b6`. Telemetry: coordinator `gpt-6.1-sol` /
`high`; reviewer `krummi-review` `gpt-6.1-sol` / `high`.

- R3: "The new A10 annotation overgeneralizes the conditional-change rule — verified resolved".
- Verdict: "Ready for implementation within the requested follow-up scope."
