# Benchmark with only Krummi and Voron, on the performance cores — PRD

**Version**: 1.0
**Date**: 2026-11-12
**Parent document**: [Krummi Comparative Benchmark — PRD](benchmark.prd.en.md) (v1.0)

> English translation of [benchmark-krummi-voron.prd.md](benchmark-krummi-voron.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Two changes in how the module 6 benchmark runs, without changing its workloads, its measurement or its thresholds:

1. **An optional mode that measures only Krummi and Voron.** A full run with the six engines takes about 4 hours on
   the maintainer's machine, and almost all of that time goes to redb, canopydb and fjall: in the first attempt at a
   full run on 2026-11-11, the benchmark had compiled Voron and measured Krummi, Voron and LMDB about 44 minutes after
   starting. The thresholds (K7) only compare Krummi with Voron, so those two engines are enough to know whether
   Krummi meets them. The user's request: «El banco tarda demasiado, hagamos opcional un banco que compare solo
   Krummi y Voron.»
2. **Measure on the performance cores, in every run.** On a processor with performance and efficiency cores, Windows
   can move a measurement process to the efficiency ones, and its result drops to half or a quarter: in the
   measurements of 2026-11-11 and 2026-11-12, one batch of Voron came out at half its usual figures while the Krummi
   one was normal, and restricting the processes to the performance cores brought back the usual figures. A Krummi /
   Voron ratio computed with one of the two engines on efficiency cores does not measure the engines, but the system's
   scheduling. This change applies with and without the new mode, and adds a line to every report. When asked to
   include it in the same PRD, the user answered: «Mételo».

This PRD inherits everything from the [benchmark](benchmark.prd.en.md) and from its parent, the
[root PRD](krummi.prd.en.md). Only what is specific to these two changes is added here; what changes or refines rules
of the parent is declared in its decisions and annotated in the parent (*Annotations to other PRDs*).

### Product decisions

| ID | Decision |
|---|---|
| B1 | **The `--solo-krummi-voron` option** (name chosen by the user) adds to the command of K10 a mode that measures only Krummi and Voron: `cargo run --release -p krummi-bench -- --ravendb <carpeta> [--carpeta <carpeta>] [--informe <archivo>] [--reducido] [--solo-krummi-voron]`. Like `--reducido`, it takes no value; it combines with all the other options. |
| B2 | **What the mode measures**: the same as a run of the six engines, except that LMDB, redb, canopydb and fjall are neither prepared nor run: Krummi and then Voron (the order of K3 without the other four), the 29 workloads with their quantities, repetitions and data, and the 29 thresholds. |
| B3 | **The mode's report** has the structure of the benchmark's *Output* with only two engine columns, Krummi and Voron, in both tables, followed in the operations table by *Krummi / Voron* and *Umbral*; it has the 29 operations rows and those of the sizes of K9, without any `no aplica` cell (both engines offer all the structures, K6). The *Motores* line names only Voron with its .NET version; the *Ejecución* line says `completa, solo Krummi y Voron` or `reducida, solo Krummi y Voron`; *Opciones cambiadas* only has lines of those two engines (with the benchmark's options, `Ninguna.`). The standard output, the exit code and the rules of the reduced report (`1 medida`, `sin evaluar`) are those of the benchmark. |
| B4 | **Performance cores**: in every run of the benchmark, with or without `--solo-krummi-voron`, full or reduced, the processes that the benchmark launches for an engine (those that prepare its stores and those that measure its workloads), and all the threads and processes that these launch, run only on the performance logical cores when the operating system distinguishes them. It distinguishes them when it gives its logical cores more than one efficiency or capacity class; the performance ones are those of the fastest class. Nothing else counts as a class: neither the frequency of each core nor the cores the system prefers. The number of threads of each workload does not change (K11), even if it is greater than that of those cores. When the system gives all its cores the same class, or none (like WSL 2 or a virtual machine that does not expose it), the processes are not restricted. If the benchmark cannot ask the system for the classes, or cannot restrict a process, the run continues: that process, or all of them if the system could not be asked, measure unrestricted (user decision: «Medir sin fijar»). The rest of the benchmark, such as compiling Voron or writing the report, is not restricted. |
| B5 | **The report says where it measured**: a new line, *Núcleos*, right after *Procesador*, with or without `--solo-krummi-voron`, also when *Procesador* says `desconocido`. It says `Núcleos: <n> lógicos de rendimiento de <total> (los motores se miden en ellos)` when all the engine processes were restricted (B4), with `<n>` the performance logical cores and `<total>` all the logical cores the system gives the machine; `Núcleos: sin fijar (el sistema no distingue núcleos de rendimiento)` when the system gives all its cores the same class or none; and `Núcleos: desconocido` when the benchmark could not ask for the classes or could not restrict some process, like any machine datum it cannot obtain (*Output* of the benchmark). The benchmark writes no message of its own for this: the error output only carries its progress, in the format that the benchmark leaves to the implementation. |
| B6 | **AC19 and pre-release runs**: a full run with `--solo-krummi-voron` on Windows and on Linux is valid for checking the thresholds of AC19 of the root PRD, because the thresholds only compare Krummi with Voron (user decision: «Sí, vale»). The comparison with the other engines only comes from a run of the six. |

### Starting state

Verified on 2026-11-12 in the repository:

| Aspect | Situation |
|---|---|
| Benchmark command | It accepts `--ravendb`, `--carpeta`, `--informe` and `--reducido`; any other argument gives `Opción no válida: <opción>.` with code `2`. `--reducido` followed by a loose argument gives the error with that argument. |
| Engines | The benchmark always measures the six, in the order of K3, and the report always has six engine columns. The parent leaves out of scope «Elegir desde la orden qué motores o qué cargas ejecutar». |
| Cores | The benchmark does not restrict the cores of any process, and the parent leaves the preparation of the machine («antivirus, modo de energía, procesos en segundo plano») to the maintainer. The measurements of the tables of 2026-11-11 and 2026-11-12 restricted the processes by hand to the performance cores of the maintainer's machine (an i9-13980HX: 16 performance logical cores of 32). |
| Duration | Full run of the six engines on Windows: about 4 hours; compiling Voron and the workloads of Krummi, Voron and LMDB, about 44 minutes. |
| Continuous integration | It runs the benchmark with `--reducido` and the six engines on Windows and on Linux (K12). |

## Actors

- **Maintainer**: runs the benchmark with `--solo-krummi-voron` to check the thresholds after a change or before a
  release (B6), and without it when they want the comparison with all the engines.
- **Continuous integration**: no command changes (K12); its report gains the *Núcleos* line.

## Flow

That of the benchmark. Before measuring, the benchmark finds out which cores are performance cores (B4); with
`--solo-krummi-voron`, it only measures Krummi and Voron (B2).

## Validations and messages

Those of the benchmark, with `--solo-krummi-voron` as one more option of its table, without a value like `--reducido`:
repeated, the error names its second occurrence; written as `--solo-krummi-voron=<algo>`, it names that whole argument;
and followed by an argument that is not an option, it names that argument. With `--solo-krummi-voron` the requirements
of RavenDB and the .NET 10 SDK still hold, because Voron is measured.

## Output

The standard output does not change. Example of a report with `--solo-krummi-voron` (illustrative figures, only some
rows):

```markdown
# Banco comparativo de Krummi

- Fecha: 2026-11-12 16:00 (UTC+00:00)
- Ejecución: completa, solo Krummi y Voron
- Sistema: Windows 11 Pro 10.0.26200
- Procesador: 13th Gen Intel(R) Core(TM) i9-13980HX, 32 núcleos lógicos
- Núcleos: 16 lógicos de rendimiento de 32 (los motores se miden en ellos)
- Memoria: 64 GiB
- Disco: NVMe SAMSUNG MZVL22T0HBLB-00B00, NTFS
- Krummi: 0123456789ab
- Motores: Voron 7.2.7 (.NET 10.0.5)
- Rust: 1.99.0
- Repeticiones: 1 previa y 3 medidas, mediana

## Resultado

Krummi cumple 29 de 29 umbrales.

## Operaciones por segundo

| Carga | Krummi | Voron | Krummi / Voron | Umbral |
|---|---|---|---|---|
| carga-en-orden | 943.198 | 666.145 | 1,41 | cumple |
| tabla-actualizacion | 114.865 | 129.932 | 0,88 | cumple |

## Tamaño del almacén (bytes)

| Carga | Krummi | Voron |
|---|---|---|
| carga-en-orden | 150.994.944 | 160.432.128 |

## Opciones cambiadas

Ninguna.
```

A run without `--solo-krummi-voron` has the usual report, with the *Núcleos* line added.

## Reuse

- The command, its validations, the measurement, the report and its format rules of the benchmark: the mode only leaves
  four engines out.
- The processes that each workload already launches (K3): B4 restricts them to the performance cores.

## Out of scope

- Choosing other engines or workloads than the Krummi and Voron pair: the rest of that exclusion of the parent holds.
- Making `--solo-krummi-voron` the default mode, or giving it an alias.
- Choosing the cores by hand, the priority of the processes, one logical core per physical core, or pinning each thread
  to a specific core.
- Adjusting the number of threads of a workload to that of performance cores.
- Restricting processes foreign to the benchmark or the rest of the benchmark; configuring the power mode, which stays
  the maintainer's responsibility.
- Distinguishing cores where the operating system does not distinguish them: in a virtual machine, and in WSL 2 when it
  does not expose them, the processes are not restricted and the Linux measurements remain exposed to the system's
  scheduling (user decision: «Aceptarlo»); telling by hand which cores to use.
- Checking after measuring that a process did not leave its cores, or measuring its CPU usage.
- Re-measuring or relabelling earlier reports: they are told apart from the new ones because they lack the *Núcleos*
  line.
- Changing continuous integration (K12), resuming a run or combining reports of one mode and the other.

## Left to the implementation

- How the performance cores are found out on each system and how a process is restricted to them.

## Acceptance criteria

1. **AC-B1** — Given a full run with `--solo-krummi-voron` on Windows and another on Linux, when they finish, then each
   writes a report with the structure of B3, the 29 operations rows, the 12 of sizes (K9 with its annotation of 7.1),
   no `no aplica` cell and the *Núcleos* line of B5; and, when observing their processes, they only measured Krummi and
   then Voron.
2. **AC-B2** — Given some results per workload and engine and some machine data injected for the test, when the
   benchmark writes the report with and without `--solo-krummi-voron`, full and reduced, with the thresholds met and
   with one missed, with performance cores, unrestricted and unknown, and with an unknown *Procesador*, then the report
   and the standard output line are identical byte for byte to the expected ones, and the exit code is the benchmark's.
3. **AC-B3** — Given some core classes injected for the test (all equal, none, two and three classes), when the
   benchmark decides where to measure, then it chooses the logical cores of the fastest class when there is more than
   one, and none in the other cases, with the *Núcleos* line of B5 of each case. Given a processor with performance
   cores that the system distinguishes, when the benchmark launches the processes of an engine, then the affinity the
   system gives each one, and a process that this one launches, is those cores, and that of the benchmark's process does
   not change. Given an injected failure when asking for the classes, or when restricting just one of the processes,
   with the same results per workload and engine and the same machine data injected in the run with the failure and in
   the run without it, then both end with the same code and the same report, except that the first says
   `Núcleos: desconocido`; in a real run, the thresholds and the exit code still come from what was measured (K7, K13).
4. **AC-B4** — Given `--solo-krummi-voron` repeated, written with `=` or followed by a loose argument, when the command
   is run, then it shows `Opción no válida: <opción>.` with the `<opción>` of *Validations and messages*, ends with
   code `2` and does nothing. With `--solo-krummi-voron`, AC-K5 and AC-K9 of the benchmark still hold.

## Annotations to other PRDs

They are added to the [benchmark](benchmark.prd.en.md), to the [root PRD](krummi.prd.en.md) and to their translations:

- [benchmark.prd.md](benchmark.prd.md), K10 and *Validations and messages*: *"Annotation of 2026-11-12 from the Krummi
  and Voron only benchmark ([benchmark-krummi-voron.prd.md](benchmark-krummi-voron.prd.md)): the command also accepts
  `--solo-krummi-voron`, without a value, which measures only Krummi and Voron (B1, B2); the validations table counts
  five options."*
- [benchmark.prd.md](benchmark.prd.md), K3 and AC-K11: *"Annotation of 2026-11-12 from the Krummi and Voron only
  benchmark: with `--solo-krummi-voron`, the benchmark measures only Krummi and then Voron (B2); in every run, each
  process that measures a workload is restricted to the performance cores when the system distinguishes them (B4)."*
- [benchmark.prd.md](benchmark.prd.md), *Output*, AC-K1 and AC-K2: *"Annotation of 2026-11-12 from the Krummi and
  Voron only benchmark: every report carries the *Núcleos* line after *Procesador* (B5), also in the expected texts of
  AC-K2; exception to «Las columnas de motores son siempre las seis»: with `--solo-krummi-voron`, the report has only
  the Krummi and Voron columns (B3); AC-K1 holds for the runs without that option."*
- [benchmark.prd.md](benchmark.prd.md), *Actors* and *Flow*: *"Annotation of 2026-11-12 from the Krummi and Voron only
  benchmark: before each release, the maintainer may run the benchmark with `--solo-krummi-voron` (B6); the benchmark
  finds out the performance cores before measuring, and with that option it only measures Krummi and Voron (B2, B4)."*
- [benchmark.prd.md](benchmark.prd.md), *Out of scope*: *"Annotation of 2026-11-12 from the Krummi and Voron only
  benchmark: exceptions to «Elegir desde la orden qué motores o qué cargas ejecutar», because `--solo-krummi-voron`
  chooses the Krummi and Voron pair (B1), and to «Preparar el equipo», because the benchmark restricts its measurement
  processes to the performance cores (B4)."*
- [krummi.prd.md](krummi.prd.md), AC19: *"Annotation of 2026-11-12 from the Krummi and Voron only benchmark: a full run
  with `--solo-krummi-voron` on Windows and on Linux, with only Krummi and Voron, is valid for checking the thresholds
  (B6)."*

## References

- Parent: [Krummi Comparative Benchmark — PRD](benchmark.prd.en.md) (K3, K6, K7, K10, K11, K12, K13, *Validations and
  messages*, *Output*, *Out of scope*, AC-K1, AC-K2, AC-K5, AC-K9, AC-K11).
- Root: [Krummi, an embedded storage engine inspired by Voron — PRD](krummi.prd.en.md) (*Output*, AC19).
- Measurements that motivate B4: L13, L31 and L40 of [odd/tasks/table-row-reads.md](../../../odd/tasks/table-row-reads.md).
