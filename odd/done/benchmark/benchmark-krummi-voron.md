# Krummi and Voron only benchmark, on the performance cores — ODD execution document

Objective: implement `docs/prd/krummi/benchmark/benchmark-krummi-voron.prd.md` (decisions B1–B6, AC-B1 to AC-B4; parent
`benchmark.prd.md`, root `krummi.prd.md`). Branch: `feat/table-row-reads` (the user keeps one branch until 7.1 is
merged). Delivery: `ask-on-risk`, left unmerged until the end of the batch. Test runner: `cargo test --workspace`
(Windows; Linux in WSL 2 Ubuntu). Estimate for the whole batch (this PRD, W25, W23, runs, cleanup, merge): 6 h – 10 h.
Real start: 2026-11-12 17:12:58 +0000.

## Specs

- **S1** — B1, verbatim: «**La opción `--solo-krummi-voron`** (nombre elegido por el usuario) añade a la orden de K10 un
  modo que mide solo Krummi y Voron: `cargo run --release -p krummi-bench -- --ravendb <carpeta> [--carpeta <carpeta>]
  [--informe <archivo>] [--reducido] [--solo-krummi-voron]`. Como `--reducido`, no lleva valor; se combina con todas
  las demás opciones.» Validations, verbatim: «repetida, el error nombra su segunda aparición; escrita como
  `--solo-krummi-voron=<algo>`, nombra ese argumento entero; y seguida de un argumento que no es una opción, nombra ese
  argumento.» Message `Opción no válida: <opción>.`, code `2` (AC-B4).
- **S2** — B2, verbatim: «lo mismo que una ejecución de los seis motores, salvo que LMDB, redb, canopydb y fjall no se
  preparan ni se ejecutan: Krummi y después Voron (el orden de K3 sin los otros cuatro), las 29 cargas con sus
  cantidades, repeticiones y datos, y los 29 umbrales.»
- **S3** — B3, verbatim: «tiene la estructura de *Salida* del banco con solo dos columnas de motores, Krummi y Voron, en
  las dos tablas, seguidas en la de operaciones por *Krummi / Voron* y *Umbral*; [...] La línea *Motores* nombra solo
  Voron con su versión de .NET; la línea *Ejecución* dice `completa, solo Krummi y Voron` o `reducida, solo Krummi y
  Voron`; *Opciones cambiadas* solo tiene líneas de esos dos motores (con las opciones del banco, `Ninguna.`).» Example
  line: `- Motores: Voron 7.2.7 (.NET 10.0.5)`.
- **S4** — B4: in every run, the processes the benchmark launches for an engine (prepare and run), and everything they
  launch, run only on the logical cores of the fastest class when the OS gives its logical cores more than one
  efficiency or capacity class; otherwise not limited; a failure to ask or to limit one process measures that process
  (or all) without limiting, and the run goes on. The benchmark process itself and the rest of the run are not limited.
- **S5** — B5, the three lines, verbatim: `Núcleos: <n> lógicos de rendimiento de <total> (los motores se miden en
  ellos)`, `Núcleos: sin fijar (el sistema no distingue núcleos de rendimiento)`, `Núcleos: desconocido`; right after
  *Procesador*, in every report. No message of its own on standard error.
- **S6** — Acceptance: AC-B1 (full `--solo-krummi-voron` runs on Windows and Linux), AC-B2 (reports byte for byte),
  AC-B3 (classes injected; real affinity of a launched process and of one it launches; injected failures), AC-B4 (CLI).

Left to the implementation (PRD): how the classes are found and how a process is limited. Decisions (D#):

- **D1** — No unsafe Rust (K14): Windows limits each engine process with a job object with an affinity limit
  (`win32job`, a safe API), assigned right after the spawn; processes it launches join the job. Linux spawns each
  engine process from a thread pinned with `sched_setaffinity` (`nix`), so the process inherits the mask from its
  creation. The classes come from the Voron program's new `cores` command (`GetSystemCpuSetInformation`'s
  `EfficiencyClass` on Windows, `cpu_capacity` under `/sys` on Linux), which the benchmark builds before measuring.
- **D2** (replaced by the launcher after R1 of the Codex review, L9) — An assignment that fails on Windows leaves that process unlimited for its whole life; the window between the
  spawn and the assignment is not measured (the workers prepare their stores first).

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1–S3, AC-B2, AC-B4 | inline | done | `e05d783` |
| T2 | S4–S5, AC-B2, AC-B3 | inline | done | `e95287d`, `eb24cdf` |
| T3 | S6 (AC-B1), docs | inline | done | `93ba937` |
| T4 | K10 (relative `--carpeta`), fix | inline | done | `47f3e8d` |

## Log

- **L1** (2026-11-12) — User requests, verbatim: «El banco tarda demasiado, hagamos opcional un banco que compare solo
  Krummi y Voron.», «Mételo», «Hazlo todo», and «Sí» to the estimate of 6 h – 10 h for the batch.
- **L2** (2026-11-12) — T1: `--solo-krummi-voron` in `cli::parse` (shares the flag handling of `--reducido`),
  `Engine::measured` for the plan and the report, two engine columns, *Ejecución* and *Motores* lines, *Opciones
  cambiadas* of the measured engines only; LMDB's version is asked only when LMDB runs. Tests: unit (CLI, report byte
  for byte, threshold missed, reduced), integration (`tests/cli.rs` AC-B4 rows; `tests/run.rs` two ignored runs).
- **L3** (2026-11-12) — T2 design (D1, D2): `orchestrator::cores` (placement from the classes, `Pin` with a
  `win32job` job on Windows and a pinned spawning thread with `nix` on Linux), `child::run_with`, the Voron program's
  `cores` command (`Cores.cs`), `report::Cores` and the *Núcleos* line. The real-affinity test pins logical core 0 so
  that it runs on any machine; a mutation that skips the job assignment makes it fail (`left: ["4294967295",
  "4294967295"]`, `right: ["1", "1"]`), then reverted. WSL's default distribution is Docker's: WSL commands need
  `-d Ubuntu`.
- **L4** (2026-11-12) — T2 checks: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo test --workspace` (816 passed, 0 failed) on Windows; on WSL 2 Ubuntu, clippy and the `krummi-bench`
  tests pass. The ignored runs `a_krummi_and_voron_only_run_measures_only_those_two` and
  `a_krummi_and_voron_only_run_stops_on_a_failure` pass on Windows (440 s) and Linux (131 s).
- **L5** (2026-11-12) — RDD review of `cac715b..e95287d` (escalated: item 5, new `win32job` and `nix` dependencies),
  lenses risk, resilience, readability and reliability: approved, lineage `review-4db0f4a645af578f`, acknowledged
  (`authority: burned`). Advisory findings: R2-001 (SUGGESTION, the `pinned` flag means «no failure»), R2-002
  (SUGGESTION, `spawn_limited` returned `true` for a failed spawn), R2-003 (SUGGESTION, the group numbering of the
  core ids): fixed in the next commit (`limit_held`, `io::Result<(Child, bool)>`, comment). R3-core0-assumption
  (WARNING, the test assumed core 0 and read the main thread on Linux): fixed (the first allowed core,
  `/proc/thread-self/status`). R3-exact-capacity-class and R4-linux-capacity-singleton (WARNING: on a Linux kernel that
  gives slightly different `cpu_capacity` values to cores of one tier, the fastest class may hold one or two cores):
  kept as written, because B4 fixes that every distinct capacity is a class («Nada más cuenta como clase: ni la
  frecuencia de cada núcleo ni los núcleos que el sistema prefiere»); a product question for the closing report.
- **L6** (2026-11-12) — AC-B1, first runs with `eb24cdf` under the bench power plan: WSL 2 passes, **29 of 29**
  thresholds, `Núcleos: sin fijar (el sistema no distingue núcleos de rendimiento)`, 2,683 s. Windows fails twice
  (678 s and 702 s): «El motor Voron falló en la carga lecturas-al-azar: el almacén no tiene el árbol bench.»
  Reproduced by hand without the benchmark and without any limit: Voron resolves a relative store path against its
  program's folder (the base store went to `voron-d6a/base`, and the relative one held only `Temp`). The runs used
  `--carpeta work/bkv/runs`, relative; the earlier full runs used absolute folders. A defect of the benchmark since
  module 6, not of B4. T4 (fix, no PRD): the orchestrator gives the workers the stores' absolute paths; test
  `a_relative_work_folder_holds_every_store` RED (failed, 200 s) then GREEN (214 s).
- **L7** (2026-11-12) — AC-B1 met. Windows with `47f3e8d` (19:10, 1,404 s): **29 of 29** thresholds, `Núcleos: 16
  lógicos de rendimiento de 32 (los motores se miden en ellos)`, two engine columns, 29 + 12 rows, no `no aplica`;
  its *Krummi* line says «con cambios sin confirmar» because the run's phase log, a tracked file, was being written.
  WSL 2 with `eb24cdf` (L6; T4 does not change a run with an absolute folder): 29 of 29. Progress lines: Krummi's 28
  processes, then Voron's. Reports: `docs/benchmark/2026-11-12-windows-krummi-voron.md` and
  `docs/benchmark/2026-11-12-linux-krummi-voron.md`. Table loads on Windows: W23 1.18, W24 0.91, W25 0.86, W26 0.96,
  W27 0.82, W28 0.86, W29 2.17.
- **L8** (2026-11-12) — RDD review of the last slice `e95287d..93ba937`, lenses readability and reliability:
  approved, lineage `review-2bf473a71af1ed9a`, acknowledged (`authority: burned`). R2-abs-fallback (WARNING, the
  silent fallback of `std::path::absolute`): commented (it fails only without a readable current folder, which the
  Voron build already needed). R2-limit-held-comment and R2-placeholder-commit-refs (SUGGESTION): fixed. R3-001
  (SUGGESTION, the relative-folder test is ignored): dismissed, CI runs the ignored benchmark tests with RavenDB
  (`.github/workflows/ci.yml:152`, `cargo test --release -p krummi-bench -- --include-ignored`).
- **L9** (2026-11-12) — Independent verifier (Sonnet, high; item 5): S1–S6 PASS, no blockers; advisories: `Cores.cs`
  has no isolated test, cores of a second processor group give `desconocido`, the dirty-commit note of L7, the
  capacity-class question of L5. Codex round 1 over `cac715b..1e9dffc`
  ([report](../../../docs/reviews/benchmark-krummi-voron/2026-11-12-code.md)): `findings`, R1 (blocker, the Windows
  process ran before its assignment to the job) and R2 (minor, the run test rejected `Núcleos: desconocido`), both
  fixed in `a65a111`: D2 is replaced by the launcher of the resolutions
  ([resolutions](../../../docs/reviews/benchmark-krummi-voron/2026-11-12-code-resolutions.md)).
- **L10** (2026-11-12) — RDD of the R1/R2 fixes `93ba937..440f8ac` (escalated, item 4), lenses risk, resilience and
  reliability: approved, lineage `review-57046d6b0ba0caf5`, acknowledged. Warnings R1-001 (orphan engine process
  when the launcher has no job), R3-launcher-spawn-error-no-fallback and R3-no-windows-pin-spawn-coverage: fixed in
  `b02b34b` (the launcher runs nothing without its job; the benchmark starts the process itself; the Windows chain test).
  R4-launcher-ctrlc-kills-engine: kept (AC-K9 removes the stores). R3-run-accepts-desconocido-unconditionally: kept
  (B5 allows it; the chain test checks the limit). Details in the resolutions.
- **L11** (2026-11-12) — Codex round 2 (follow-up, `1e9dffc..1531bd0`,
  [report](../../../docs/reviews/benchmark-krummi-voron/2026-11-12-code-02.md)): `approved`, R1 and R2 verified resolved.
- **L12** (2026-11-12) — Closing report: [benchmark-krummi-voron-report.md](benchmark-krummi-voron-report.md). The
  batch continues with the table loads (W23, W25) in `odd/done/tables/table-row-reads.md`.
