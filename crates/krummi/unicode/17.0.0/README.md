# Unicode 17.0.0 data

The Unicode Character Database files the text column collation (IC2 of
`docs/prd/krummi/tables/tables-collation.prd.md`) is pinned to, downloaded from
<https://www.unicode.org/Public/17.0.0/ucd/> and kept byte for byte: do not edit them.

| File | Used for |
|---|---|
| `UnicodeData.txt` | Canonical decompositions, canonical combining classes, General_Category Mn |
| `CompositionExclusions.txt` | The composition exclusions of NFC |
| `CaseFolding.txt` | Full case folding (statuses C and F) |
| `Scripts.txt` | Script Latin, Greek and Cyrillic |
| `NormalizationTest.txt` | The canonical equivalence test of AC-IC2 |

`scripts/unicode/gen_collation_tables.py` reads them and writes `crates/krummi/src/table/collation_tables.rs`,
which is committed (run `python -I scripts/unicode/gen_collation_tables.py`). The test
`crates/krummi/tests/table_collation_fold.rs` reads them at test time to build its own reference.

The files are under the Unicode License v3, in `LICENSE.txt`.
