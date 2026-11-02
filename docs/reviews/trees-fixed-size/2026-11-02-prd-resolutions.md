# Fixed-size trees PRD review resolutions

Resolutions of [2026-11-02-prd.md](2026-11-02-prd.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-02

Review record: report `2026-11-02-prd.md`, wrapper status `findings`, thread `01a19b2d-df68-72b2-8cdc-73244f1ad919`.
Executed settings from the wrapper telemetry: coordinator `gpt-6.1-sol` / `high`; one `krummi-review-critical`
reviewer `gpt-6-astra` / `xhigh`.

Artifact versions after the fixes (uncommitted working tree, SHA-256 of the bytes):

- `docs/prd/krummi/trees-fixed-size.prd.md`: `670d78a06d99fbcb119cbdb64f225111e9eb962a700741c934fc1e3223e88a6a`
- `docs/prd/krummi/krummi.prd.md`: `ee3ca142ab4759b897361444695977c4671f13f1bd25fa2b3e99845a880c214a`
- `docs/prd/krummi/krummi.prd.en.md`: `c6275fc362f51452611c9ab3cb980fd06ba4f74902a48746074787fb4d1939c5`
- `docs/prd/krummi/pager.prd.md`: `4d2c235fdce7c38a36053fd2481157eef4e3faf31678a7ee54f8afc9999191ff`
- `docs/prd/krummi/pager.prd.en.md`: `9673d674c26515f49dcb96fdc3ca8ccca3624f9bae59bdfbbe248d3b034406fc`
- `docs/prd/krummi/journal.prd.md`: `ef3a48888d5d4ace713ee930e42840e34fce95e7bebe3dbc8b69afc27e14fa8a`
- `docs/prd/krummi/journal.prd.en.md`: `d4b59bb89758adbacc4ee08ae45190e72e8f5838625cb6cefab3946d194e2c96`
- `docs/prd/krummi/journal-group-commit.prd.md`: `8809d4ed46619ecdcddc8d08e16c1b6c73964d52305d893d8f473951007fcdaa`
- `docs/prd/krummi/journal-group-commit.prd.en.md`: `554b7664349dd0a754357fa082af26afb13b64bd68a22f578f3d7b93cc4736c6`
- `docs/prd/krummi/free-space.prd.md`: `71800e6e21906219c31f82e86589310c77a61e64e422b029257edb551f3215bf`
- `docs/prd/krummi/free-space.prd.en.md`: `65f8f72d56977a5e5b31c0040d1bd86efecc9f96f35cf29bf9495bbac52ec196`
- `docs/prd/krummi/transactions.prd.md`: `6808e31c6158d468d57cb9031ccdd10efb13f678a4593c358cbd3dee8cef378a`
- `docs/prd/krummi/transactions.prd.en.md`: `f8afccc1411e4a0ca551a2851baa2b8f268fdc6fc8f47683c9860aaedfd5c416`
- `docs/prd/krummi/trees.prd.md`: `ac5f73355214f6a3a287f72c58b0a3ba2242796fdf034d68c4d77d2bce3c4747`
- `docs/prd/krummi/trees.prd.en.md`: `56aa1ca104ac80a27a1e7631a98f24dd3035c3923f102333dfcf3b41e1a1abfb`
- `docs/prd/krummi/trees-keys.prd.md`: `a533fb9990faee09adf26dedd01a436fa938a628384ec3a74086ff7f2e589a0a`
- `docs/prd/krummi/trees-keys.prd.en.md`: `69b5b9bf3876f8da2e913f374fa7a347cf8ad42a6407793ac915c2bfac01ad1b`
- `docs/prd/krummi/trees-scan.prd.md`: `7929f41e22dd5ff2d8acf5fd1f2f7951f4e692be6c8b955c4d99da1f4784f3b2`
- `docs/prd/krummi/trees-scan.prd.en.md`: `284ba98c7974579e1174f7d39b96d703dc5f7ea548a0217295c468fb97f39910`
- `docs/prd/krummi/trees-large-values.prd.md`: `19379ad8a72a1cd67696164699be18f95b7ca693ab22c3125c2f23fa8b321c8e`
- `docs/prd/krummi/trees-large-values.prd.en.md`: `6487291a1b21341cd64693aeb7afb85f93a557e41aa0455d37c587e293fd33b6`
- `docs/prd/krummi/trees-multivalue.prd.md`: `07c7ffb6edb4bdbe9a04e9ee6d438cad69a87c95f957cc0995fbec6a79ae9669`
- `docs/prd/krummi/trees-multivalue.prd.en.md`: `4d2637b64c9d63ea57eaac6db8aac119ba336cd4d0f010dd3037338765b16a06`
- `docs/prd/krummi/trees-node-check-cache.prd.md`: `2513e75b2cc3ad9f9283d86a105ac32d78c99f8c9d51fbbe617467b22e6aea0c`
- `docs/prd/krummi/trees-node-check-cache.prd.en.md`: `4ad304cc634d097a1ec960a0dcfcea22d07668116bc857a0dddd76897695fc79`

### R1 — minor: AC-X10 excludes errors that the opening rules require

- Disposition: fixed.
- Fix: AC-X10 now says that the open modes, Existe, Tipo, Listar, Renombrar sin sobrescritura and the entry count
  "nunca fallan con daño en las páginas del propio árbol, y dan su resultado o su error de siempre (por ejemplo,
  Crear nuevo sobre ese árbol falla con árbol ya existente)", which matches X10 and A1.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — minor: claimed annotations in affected PRDs are missing

- Disposition: fixed.
- Fix: every document listed in *Anotaciones a otros PRD* now carries a dated 2026-11-02 annotation that links to
  `trees-fixed-size.prd.md` and names the X decisions behind each note, inserted before its starting state, in the
  Spanish PRD and in its English translation, as 5.4 did:
  - `krummi.prd.md` (D7), `pager.prd.md` (P5), `journal.prd.md` (J3), `journal-group-commit.prd.md` (E15),
    `free-space.prd.md` (F11) and `transactions.prd.md` (whole document): format `9` (X12).
  - `trees.prd.md`: B3, B6, B7 and B8 (X1–X3, X11).
  - `trees-keys.prd.md`: A1, A3, A5, A7, A9, A10, A13 and *Fuera de alcance* (X1–X15).
  - `trees-scan.prd.md`: R3, R6–R8, R10 with *Salida* and AC-R14, and *Fuera de alcance* (X7, X8, X10, X12).
  - `trees-large-values.prd.md`: V9 with *Salida* and AC-V12, and *Fuera de alcance* (X1, X12).
  - `trees-multivalue.prd.md`: M4, and M14 with *Salida* and AC-M18 (X4, X12).
  - `trees-node-check-cache.prd.md`: C1 with *Fuera de alcance*, and *Salida* (X12, X14).
  The English annotations link to `trees-fixed-size.prd.en.md`, which the translation step after this loop creates.
- Status: claimed fixed, unverified until a follow-up review.

## 2026-11-02 (verification)

Review record: report [`2026-11-02-prd-02.md`](2026-11-02-prd-02.md) (follow-up), wrapper status `approved`, thread
`01a19b37-f231-7152-9816-7d8cfabc7497`. Executed settings from the wrapper telemetry: coordinator `gpt-6.1-sol` /
`high`; one `krummi-review-critical` reviewer `gpt-6-astra` / `xhigh`.

- R1 and R2: verified resolved.
- Verdict, quoted: "Ready for implementation within the requested follow-up scope; R1 and R2 are verified resolved."
