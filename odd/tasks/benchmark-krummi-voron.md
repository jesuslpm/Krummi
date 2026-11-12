# Krummi and Voron only benchmark, on the performance cores — ODD execution document

Objective: implement `docs/prd/krummi/benchmark-krummi-voron.prd.md` (decisions B1–B6, AC-B1 to AC-B4; parent
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
- **D2** — An assignment that fails on Windows leaves that process unlimited for its whole life; the window between the
  spawn and the assignment is not measured (the workers prepare their stores first).

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1–S3, AC-B2, AC-B4 | inline | pending | |
| T2 | S4–S5, AC-B2, AC-B3 | inline | pending | |
| T3 | S6 (AC-B1), docs | inline | pending | |

## Log

- **L1** (2026-11-12) — User requests, verbatim: «El banco tarda demasiado, hagamos opcional un banco que compare solo
  Krummi y Voron.», «Mételo», «Hazlo todo», and «Sí» to the estimate of 6 h – 10 h for the batch.
