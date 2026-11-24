# ANN experiment code review resolutions

Resolutions of [2026-11-24-code.md](2026-11-24-code.md). Append-only: each update is dated and keeps the earlier ones.
A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-24

Review record: [2026-11-24-code.md](2026-11-24-code.md), wrapper status `findings` (R1, R2, R3 open), thread
`01a20fb3-84d6-7540-9775-2af45722e2db`. Telemetry:
- coordinator `gpt-6.1-sol` / `high`;
- reviewer `krummi-review` (Halley) `gpt-6.1-sol` / `high`.

Artifact versions: fix commits `56c24ce` (R1), `f5b4294` (R3), `96ee454` (R2, code), and the commit that adds this
file (R2, report, raw outputs and design document 12).

### R1 — blocker: Reject the remaining options that commands silently ignore

- Disposition: fixed.
- Fix (`56c24ce`): `preparar --filas` fails with "La opción --filas no vale con preparar.", and `--informe` outside
  `medir` and `grafo` fails with "La opción --informe solo vale con medir y grafo." Exit code 2 in both cases.
  `verdad --filas` stays valid, because the ground truth is computed at that scale. The test
  `options_a_command_would_ignore_are_rejected` now covers both. RED was observed before the fix: the test panicked
  at `main.rs:154`.
- Reproduction after the fix:
  - `preparar --carpeta <fixture> --conjunto sift --filas 2`: "La opción --filas no vale con preparar.", exit 2.
  - `verdad --carpeta <fixture> --conjunto sift --filas 5 --informe <fixture>/r.md`: "La opción --informe solo vale
    con medir y grafo.", exit 2.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — major: Complete the required memory measurement and distinguish payload estimates

- Disposition: fixed, within what can be measured without unsafe code.
- Fix, code (`96ee454`):
  - `memory.rs` reads `VmRSS` and `VmHWM` from `/proc/self/status`. Each structure's section now reports the RSS
    growth while it was built: the SQ8 codes, the two IVF indexes and the HNSW graph.
  - On Windows the section says "RSS no medido en esta plataforma". The workspace forbids unsafe code (`Cargo.toml`
    `unsafe_code = "forbid"`), and std has no safe API for the working set.
  - The HNSW line now calls the 4-byte link size a compact estimate, and says that `hnsw_rs` keeps shared pointers
    and its own copy of the vectors.
  - Tests: `memory::tests::status_fields_are_read_in_bytes` and
    `growth_states_the_growth_or_that_it_was_not_measured`.
- Fix, measurement: Linux (WSL 2), 100,000 rows, every structure of SIFT, GloVe and Cohere. Each figure kept in the
  report is the first section of its process: inside one process the allocator reuses memory an earlier section freed
  (Cohere dot measured after L2 gave +0.0 MiB for its SQ8 codes), so the dot product was measured again in its own
  processes. Raw outputs: `docs/benchmark/2026-11-24-ann-experiment/linux-memory.md` and `linux-memory-dot.md`.
- Fix, report: the new section *Memoria* of `docs/benchmark/2026-11-24-ann-experiment.md`.
  - The `hnsw_rs` graph takes about 2.7–2.8 KiB per vector beyond the vectors (GloVe and Cohere dot), against
    128 bytes compact.
  - IVF grows to about twice its computed size while it builds.
  - The HNSW link row of the Cohere table is labelled a compact estimate.
  - Design document 12's *Measured result* carries the memory line.
- Left as stated limits, in *Lo que no se midió*:
  - memory on Windows and at 1 M rows;
  - the page-cache working set, which only exists for the stored index of 7.6.
- Status: claimed fixed, unverified until a follow-up review.

### R3 — minor: Handle recall when fewer than k neighbours exist

- Disposition: fixed.
- Fix (`f5b4294`): `recall` divides by the neighbours the truth has, the sum of min(k, len), instead of
  queries × k. With full truth lists (100 ids), every committed recall figure is unchanged.
  `recall_counts_the_ids_found_among_the_first_k` now covers a five-row base. RED was observed before the fix
  (left 0.5, right 1.0).
- Reproduction after the fix: five rows `[1; 128]` to `[5; 128]`, query `[1; 128]`. `medir --filas 5 --sondeos 1`
  prints recall@10 and recall@100 1.0000 for the SQ8 scan, IVF-Flat and IVF-SQ8.
- Status: claimed fixed, unverified until a follow-up review.

Checks after the fixes:
- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: 1283 passed, 0 failed.

`git diff --check` (noted in the report, not a finding): the trailing blank line at the end of the execution
document was removed. The phase log's first line and the raw outputs are kept as generated.
