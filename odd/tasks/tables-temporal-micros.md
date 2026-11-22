# Temporal values in microseconds — ODD execution document

Objective: implement `docs/prd/krummi/tables/tables-temporal-micros.prd.md` (approved by the Codex PRD review on
2026-11-22). Branch: `feat/tables-json` (same branch as the PRD; base `c813083`). Delivery: `ask-on-risk` (single
branch, left unmerged). Test runner: `cargo test --workspace` (Windows; Linux in WSL 2 Ubuntu). Estimate: 6 h – 10 h.
Real start: 2026-11-22 07:08:13 +00:00.

## Specs

- **S1** — The PRD `docs/prd/krummi/tables/tables-temporal-micros.prd.md` and its ancestry up to the root: decisions
  TU1–TU8, *Validaciones y mensajes*, *Salida*, *Fuera de alcance* and *Dejado a la implementación*, verbatim there.
  Its acceptance criteria AC-TU1–AC-TU6 and AC-TU8 are tests; AC-TU4 also has an execution check (a store written by
  the base opened by the head); AC-TU7 is the measurement.
- **S2** — TU7 and AC-TU7: N11 with its 2026-11-20 annotation: a one-round pass of every bench load before and after,
  and 10 to 20 rounds of W23–W32 and of any load that moves more than 3 %, on Windows and Linux, with the bench
  data checks (K4) unchanged.
- **S3** — The project's checks (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`), RDD on each work-unit commit, the Codex code review over the whole range, and the closing
  report.
- **S4** — The user's instruction (L1): «Implementa el PRD de microsegundos con ODD».

## Tasks

| ID | Specs | Task | Route | Status | Commit |
|---|---|---|---|---|---|
| T1 | S1 (AC-TU4) | pin the stored bytes of temporal rows, keys and index entries with the base code | inline | done | `f36b0eb` |
| T2 | S1 (TU1–TU6, AC-TU1–AC-TU6) | values in microseconds as `i64` everywhere, no rounding of given values, JSON instant in µs, tests | inline | done | T2+T3 commit |
| T3 | S1 (TU8, AC-TU8) | guides and API docs with one unit | inline | done | T2+T3 commit |
| T4 | S2 | measurements, Windows and Linux (one round, L10) | inline, background runs | done | no code; results in L11 |
| T5 | S3 | checks, RDD, Codex review, closing report | inline | pending | |

Route evidence: one behavior (the unit of temporal values) with its tests; no parallel units and no writer reason, so
the parent works inline.

## Log

- **L1** (2026-11-22, user): «Implementa el PRD de microsegundos con ODD»; estimate 6 h – 10 h confirmed («Sí,
  empieza»).
- **L2** — Branch: `feat/tables-json`, where the PRD is (default chosen without asking).
- **L3** — T1: golden bytes captured from the base code (`c813083`): `crates/krummi/tests/table_temporal.rs` (rows and
  primary keys) and `temporal_entries_store_the_bytes_stored_before_the_change` in
  `crates/krummi/src/table/index_key_tests.rs` (index entries, four shapes). Their inputs are given through helpers
  that are the only lines T2 changes. A store written by the base (rows, a temporal default, an index over
  `ts DESC, d`, analyzed statistics) and its readout are kept in the scratchpad for the AC-TU4 execution check.
- **L4** — T2: RED observed: `crates/krummi/tests/table_temporal.rs` with AC-TU1–AC-TU3, AC-TU5 and AC-TU6 against the
  new API failed to compile (19 errors: `i128` against `i64`). GREEN after `Value::{Time, Timestamp, Duration}(i64)` in
  microseconds, no rounding in `normalize`, the text form in microseconds, the conversions removed from the row, key
  and projection codecs, `as_timestamp` returning `i64` microseconds (its text rounding kept, TU4) and the bench worker
  passing `ts` as it is. Existing tests moved to microseconds; the rounding cases of temporal values (E8, AC6 of 7.1)
  became domain cases. The golden bytes of T1 pass unchanged with microsecond inputs (AC-TU4). T3 in the same commit,
  so the guides' doctests stay green: one unit in `data-types.md`, `rows.md` and `json.md`.
- **L5** — AC-TU4 execution check: a store written by the base (`c813083`) with temporal rows, a temporal default, an
  index `ts DESC, d` and analyzed statistics, opened by the head, reads every value, default, index row, column
  statistic and prefix statistic equal to the base's readout divided by 1,000; the same store written by the head
  reads identical to it.
- **L6** — Checks: fmt and clippy pass; `cargo test --workspace` 63 binaries, 1,221 passed, 0 failed, 35 ignored
  (before removing two needless `drop` calls that clippy flagged in the new test file; that test passes after).
- **L7** — RDD of `c813083..41043c7` (reliability, readability): approved, `review-3cad7342c5da613f`, four
  SUGGESTIONs, all acted on in the next commit: R2-rounded-table-misnamed (the AC6 test of 7.1 and its table renamed),
  R2-normalize-doc-wrap (the doc of `normalize` rewrapped), R2-midfile-imports (the imports of `table_temporal.rs`
  moved to the top), R3-001 (a test that a key whose integer does not fit in an `i64` does not decode as a temporal
  value).
- **L8** — Codex code review of `c813083..3a5ed50` ([report](../../docs/reviews/tables-temporal-micros/2026-11-22-code.md),
  [resolutions](../../docs/reviews/tables-temporal-micros/2026-11-22-code-resolutions.md)): approved on round 1, no
  findings.
- **L9** — RDD of `41043c7..8d749f6` (reliability): approved, `review-218b53af7edd6bf9`, two advisories, not acted on.
  R3-normalize-doc-semantics (WARNING): the earlier wording it compares with ("out of domain (of a `decimal`, also
  after rounding)") was this run's own intermediate text; a decimal that rounding takes past its precision fails with
  `DecimalPrecision`, not `OutOfDomain` (`ac6_domains_and_decimal_rounding_through_a_table`, 99.995 in a
  `decimal(4, 2)`), which is what the doc now says. R3-temporal-key-test-attribution (SUGGESTION): `u64::MAX` is
  refused both because it does not fit in an `i64` and because it is outside every temporal domain; the test pins the
  refusal, not its cause.
- **L10** (2026-11-22, user): «No hagas las diez rondas». T4 keeps only the one-round pass of W1–W32 on each system.
  The Windows ten-round run had already ended at once without measuring (`pick.py` chose W13, which runs only inside
  W12: «1 carga no válida: W13»); the Linux one was cancelled by making its picker select no load.
- **L11** — T4: one round of W1–W32 (W13 measured inside W12), base `c813083` against head `275f840`, release builds
  under the bench power plan. Windows ran from 2026-11-22T08:20:04Z to 08:42:53Z, Linux (WSL 2 Ubuntu, `~/tm`) from
  08:47:16Z to 09:47:30Z. Every load's data digests matched between base and head on both systems (K4: no «digests
  differ»), and both passes exited 0. The ratio is the base time over the head time, so above 1 means the head is
  faster. Only W23–W32 touch temporal values: they are the table loads (`crates/krummi-bench/src/worker/tables.rs`),
  and every row has the `timestamp` column `ts`. Beyond ±3 % among them: Windows W32 1.089; Linux W23 0.923, W25
  1.070, W27 1.031 and W28 1.043. The same load does not move the same way on both systems (W23 1.024 on Windows,
  0.923 on Linux), and loads that never touch a temporal value move as much or more (Windows W9 1.348, W12 1.132,
  W17 1.179; Linux W12 1.322, W13 1.219, W16 1.164, W17 0.891, W19 0.925). With one round none of these ratios is
  confirmed, and none is attributed to the change. The ten-round confirmation that N11 asks for was dropped by the
  user (L10).

| Load | Windows base (ns) | Windows head (ns) | Windows ratio | Linux base (ns) | Linux head (ns) | Linux ratio |
|---|---:|---:|---:|---:|---:|---:|
| W1 | 738.5 | 717.7 | 1.029 | 9,940.2 | 9,958.3 | 0.998 |
| W2 | 8,469.7 | 8,455.1 | 1.002 | 19,348.7 | 19,517.7 | 0.991 |
| W3 | 2,416.0 | 2,494.0 | 0.969 | 2,940.2 | 2,897.7 | 1.015 |
| W4 | 119,365.9 | 119,253.2 | 1.001 | 8,943,755.0 | 9,014,510.4 | 0.992 |
| W5 | 50,586.9 | 50,930.4 | 0.993 | 4,484,814.0 | 4,429,566.1 | 1.012 |
| W6 | 43,179.1 | 43,137.5 | 1.001 | 1,054,695.0 | 1,074,408.7 | 0.982 |
| W7 | 928.1 | 936.5 | 0.991 | 1,175.4 | 1,170.3 | 1.004 |
| W8 | 229.8 | 232.9 | 0.987 | 323.3 | 299.9 | 1.078 |
| W9 | 254.4 | 188.8 | 1.348 | 459.3 | 446.4 | 1.029 |
| W10 | 30.7 | 31.0 | 0.991 | 37.3 | 38.3 | 0.974 |
| W11 | 46.7 | 46.5 | 1.004 | 50.0 | 53.5 | 0.935 |
| W12 | 560.0 | 494.8 | 1.132 | 500.9 | 378.9 | 1.322 |
| W13 | 16,589.0 | 14,705.5 | 1.128 | 97,477.2 | 79,985.0 | 1.219 |
| W14 | 8,188.4 | 8,205.9 | 0.998 | 14,898.7 | 14,406.0 | 1.034 |
| W15 | 8,717.6 | 8,787.0 | 0.992 | 14,864.8 | 15,029.2 | 0.989 |
| W16 | 15,279,532.0 | 15,279,800.0 | 1.000 | 5,269,852.5 | 4,528,355.6 | 1.164 |
| W17 | 140,563.9 | 119,193.8 | 1.179 | 280,369.0 | 314,605.5 | 0.891 |
| W18 | 6,076.9 | 6,010.0 | 1.011 | 7,231.5 | 6,562.7 | 1.102 |
| W19 | 7,555.9 | 7,503.9 | 1.007 | 14,962.4 | 16,171.1 | 0.925 |
| W20 | 121.7 | 115.6 | 1.053 | 117.2 | 120.3 | 0.974 |
| W21 | 6,397.1 | 6,384.5 | 1.002 | 18,562.0 | 18,624.1 | 0.997 |
| W22 | 459.9 | 463.1 | 0.993 | 546.0 | 561.1 | 0.973 |
| W23 | 2,110.9 | 2,062.1 | 1.024 | 8,567.6 | 9,282.7 | 0.923 |
| W24 | 10,033.1 | 10,075.9 | 0.996 | 17,272.6 | 17,756.3 | 0.973 |
| W25 | 2,132.0 | 2,106.5 | 1.012 | 2,144.3 | 2,004.4 | 1.070 |
| W26 | 1,300.1 | 1,318.6 | 0.986 | 1,451.6 | 1,429.2 | 1.016 |
| W27 | 81.8 | 81.1 | 1.009 | 81.7 | 79.3 | 1.031 |
| W28 | 8,037.9 | 8,048.4 | 0.999 | 13,923.4 | 13,345.0 | 1.043 |
| W29 | 8,345.9 | 8,370.4 | 0.997 | 14,197.5 | 14,216.2 | 0.999 |
| W30 | 18,284.5 | 18,323.7 | 0.998 | 26,522.9 | 26,441.7 | 1.003 |
| W31 | 3,410.2 | 3,386.7 | 1.007 | 3,224.0 | 3,273.6 | 0.985 |
| W32 | 1,162.0 | 1,067.4 | 1.089 | 1,227.1 | 1,227.3 | 1.000 |

- **L12** — RDD of `8d749f6..8d70b7f` (readability, docs only): approved, `review-b35b832d6dae30af`, one SUGGESTION,
  acted on: R2-001 (L11 named only three of the ratios beyond ±3 % and did not say which loads touch temporal values;
  L11 now names them).
