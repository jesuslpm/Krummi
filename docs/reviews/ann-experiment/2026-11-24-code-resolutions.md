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

## 2026-11-24 (verification)

Follow-up report: [2026-11-24-code-02.md](2026-11-24-code-02.md), wrapper status `findings` (R2 open), thread
`01a20fd9-e59c-74f0-9c56-ebb1703ee8eb`. Telemetry:
- coordinator `gpt-6.1-sol` / `high`;
- reviewer `krummi-review` (Meitner) `gpt-6.1-sol` / `high`.

Outcomes, quoted:
- R1: "verified resolved".
- R3: "verified resolved".
- R2: "partially corrected, still open". Verdict: "**Changes required.** R2 remains open (major); R1 and R3 are
  verified resolved."

## 2026-11-25

Artifact versions:
- `fe18410`: code, `Ivf::allocated_bytes` and the IVF output line;
- the commit that adds this entry: the report's *Memoria*, the raw output `linux-memory-2.md` and design document 12.

### R2 — major: Complete the required memory measurement and distinguish payload estimates

- Disposition: fixed (second correction).
- Fix: the three measures are now kept apart, in the output and in the report.
  - **Payload** is the sum of buffer lengths. The output labels it "carga útil".
  - **Allocated** is new. `Ivf::allocated_bytes` sums every buffer's capacity: ids, codes, vectors and norms of each
    list, the list headers, the centroids and their norms, and the quantizer. The test is
    `allocated_bytes_count_capacities_and_norms_beyond_the_payload`.
  - **Observed RSS growth** is labelled as including the construction temporaries.
- Claims withdrawn:
  - "El índice que queda es el del tamaño calculado" (the report) and "keeps its computed size afterwards" (design
    document 12). The IVF structure keeps 1.4–1.5 times its payload allocated: 90.2 against 61.9 MiB for SIFT at
    100,000 rows, and 5,252.2 against 3,675.6 MiB for Cohere at 1 M. This agrees with Codex's capacity
    reproduction.
  - The graph-only figure per vector and its extrapolation to 1 M. The report now gives only the observed RSS growth
    of `hnsw_rs`, and states that graph and temporaries cannot be separated from outside the crate.
- Fix, 1 M coverage: the reference case of document 12 (Cohere 1 M × 768, L2) was measured on Linux, each structure
  in its own process (phase log `Linux memory run 2`, 23:23:49 to 00:00:08 UTC):
  - SQ8 codes: 732.4 MiB payload, RSS +1486.3 MiB;
  - IVF structure: 3675.6 MiB payload, 5252.2 MiB allocated, RSS +7695.9 MiB;
  - `hnsw_rs`: 3051.8 MiB compact estimate, RSS +4618.3 MiB.

  The 100,000-row figures were measured again in fresh processes, one per set and metric.
- Kept as stated limits, with their evidence in *Lo que no se midió*:
  - Windows RSS: no safe std API, and `unsafe_code = "forbid"`. Payload and allocated bytes are the same on both
    platforms, so only the allocator's overhead differs.
  - RSS at 1 M for SIFT, GloVe and the dot product.
- Status: claimed fixed, unverified until a follow-up review.

Checks:
- `cargo fmt --all --check`: passed.
- `cargo clippy -p krummi-ann-lab --all-targets -- -D warnings`: passed.
- `cargo test -p krummi-ann-lab`: 24 and 2 tests passed.
- The full workspace run is recorded in the execution document.

## 2026-11-25 (verification)

Follow-up report: [2026-11-25-code.md](2026-11-25-code.md), wrapper status `findings` (R2 open), thread
`01a21008-331f-7c31-b8e2-11d24d3673b1`. Telemetry:
- coordinator `gpt-6.1-sol` / `high`;
- reviewer `krummi-review` (Socrates) `gpt-6.1-sol` / `high`.

Outcome, quoted: R2 "partially corrected, still open". The allocation accounting, the withdrawn claims and the 1 M
reference case are verified. Open:
- "Remaining coverage gap": Windows RSS, and RSS at 1 M for SIFT, GloVe and the Cohere dot product.
- "Incorrect process-isolation claim introduced by the correction".

### R2 — major (third round)

- Process-isolation claim: fixed in the commit that adds this entry. The report and design document 12 now say that
  `medir` builds SQ8 and then IVF in one process, so IVF inherits what the allocator retained from step 1, and that
  HNSW runs in another process.
- Coverage gap: pending, a decision for the user. It costs another dependency for a safe Windows memory API (unsafe
  code is forbidden), or external process sampling, plus about 2 to 3 hours of runs.
- Status: the wording fix is claimed, unverified until a follow-up review.

## 2026-11-25 (user decision)

The user, asked whether to measure the remaining RSS coverage or to accept it as a limit, answered verbatim:
"Aceptar el límite (Recomendado)". The remaining coverage is therefore an agreed scope limit of this experiment,
recorded in the execution document's Log (L18) and stated in the report's *Lo que no se midió*:
- Windows RSS;
- RSS at 1 M for SIFT, GloVe and the Cohere dot product.

Codex's verdict is not changed.
