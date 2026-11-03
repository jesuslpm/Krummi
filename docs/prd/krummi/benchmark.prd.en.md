# Krummi Comparative Benchmark — PRD

**Version**: 1.0
**Date**: 2026-11-03
**Parent document**: [Krummi, an embedded storage engine inspired by Voron — PRD](krummi.prd.en.md) (v1.1)

## Objective and context

Build child PRD **6** of the [product split](krummi.prd.en.md#split-into-child-prds): a reproducible comparative
benchmark that measures Krummi against other embedded engines with the same workloads on the same machine (D5), and
that sets the performance thresholds of modules 1 to 5.

When this change closes, the maintainer can run a single command on Windows and on Linux and get a report that
compares Krummi with **Voron**, **LMDB**, **redb**, **canopydb** and **fjall** on every workload that each engine runs
(K6), and that says, workload by workload, whether Krummi meets its threshold. Objective 3 of the parent
("competitive, measured performance") gets a verifiable definition: **on every workload, Krummi delivers at least 80%
of what Voron delivers**; on the fixed-work workloads, that means taking no more than 1.25 times as long as Voron (K7).

**Why Voron as the reference**: Krummi reproduces the capabilities of Voron (objective 2 of the parent), so the
question the benchmark answers is whether it reproduces them with comparable performance. The other engines set no
thresholds: they give context on where Krummi stands among the embedded engines that a Rust application could choose.

**Why now**: modules 1 to 5 are closed and their PRDs left the thresholds for this module. Until now each one included
its own measurement of Krummi, without comparing it with anyone (F13, E16, V11, M15, X13).

This PRD inherits everything from the [root PRD](krummi.prd.en.md). Only what is specific to module 6 is added here.
What changes or refines rules of the parent is declared in its decisions and annotated in the parent (see
[Annotations to other PRDs](#annotations-to-other-prds)).

In this document, an **engine** is each of the six that the benchmark compares; a **workload** is one of the measured
operations of the [workload table](#workloads); a **repetition** is one execution of a workload for an engine; the
**base store** is the one that results from workload W2 (K4).

### Product decisions

| ID | Decision |
|---|---|
| K1 | **Engines and versions**: Krummi, with the code of the working copy from which the benchmark is run; **Voron** from RavenDB `7.2.7`; **LMDB** through heed `0.23.1`; **redb** `4.4.0`; **canopydb** `0.2.5`; and **fjall** `3.1.13`. Changing the version of any of them is a change to this PRD and is annotated here; changing the version of Voron also changes the reference of the thresholds (K7). |
| K2 | **Voron, exception to D5**: Voron is not a Rust engine. The benchmark measures it with its own .NET program that uses Voron compiled from the RavenDB `7.2.7` source code, with the .NET 10 SDK that this code requires. The RavenDB code is supplied by the maintainer (K10); the Krummi repository neither includes nor distributes it (its license without a key is AGPLv3). Voron has no package of its own published and can only be obtained this way. Krummi neither reuses nor translates Voron code (D1): the benchmark only uses it to measure it. Compiling it may leave intermediate files inside the RavenDB folder, but it changes none of its source files. |
| K3 | **Same conditions**: in each run, all the engines are measured on the same machine, on the same disk and in the same working folder (K10), in this order: Krummi, Voron, LMDB, redb, canopydb and fjall; and, within each engine, the workloads in the order of the table. Two engines are never measured at the same time. Each workload of each engine runs in its own process, which does its warm-up repetition and its three measured ones (K8) and ends; W12 and W13 are a single run with two metrics and share a process. Every commit is **durable** in all the engines: it returns only when its changes have been brought to the physical medium (G2). An engine whose commit does not guarantee this by itself achieves it with its own flush-to-disk operation right after each commit, and that time is part of the measurement. The engines with their own cache (redb, canopydb and the block cache of fjall) have it at 1 GiB; the others use the system cache. In canopydb and fjall, the benchmark uses their single-writer transactions, not the optimistic multi-writer ones. The rest of the options of each engine stay at their default value, except what it needs to meet this decision or to offer the structures of K6. Krummi and the other Rust engines are compiled with the repository's release profile; Voron, in release configuration, with the default .NET garbage collection and tiered compilation. |
| K4 | **Data**: keys are 16 bytes and values 100 bytes, except where the workload table says otherwise; both are random bytes, which do not compress. A fixed seed generates the same bytes, in the same order, for all the engines and all the repetitions. **At random** means a permutation without repetitions: each key appears only once; the **new entries** have keys distinct from one another and from those of the base store. **In key order** means the same random keys sorted by their bytes. The **base store** of an engine is the one W2 leaves with 1,000,000 entries (10,000 in the reduced run, K11), closed without error. In the workloads that only read (W7–W11, W18, W20 and W22), the workload's process opens its starting store only once: the warm-up repetition fills the caches, the system one and the engine's own, and the three measured repetitions read with all of them warm (in the reduced run, with no warm-up, the single measured repetition reads with the engine's own cache cold, K11). W12 modifies the store, so its readers start each repetition with the engine's own cache cold. In the ones that modify, each repetition starts with its starting store freshly prepared, with exactly the starting content, freshly opened and without emptying the system cache, and closes it when it ends. W16 starts each repetition with the base store closed, because it measures opening it. |
| K5 | **Common workloads**: W1 to W18 of the [workload table](#workloads). All six engines run them. |
| K6 | **Workloads for own structures** (W19 to W22): multi-value trees and fixed-size trees. They are run only by the engines that offer that structure natively: Krummi, Voron, LMDB (sorted duplicate keys and integer keys) and redb (multi-value tables and integer keys). canopydb and fjall do not run them and their cells say `no aplica`, also in the size table; the benchmark does not emulate them. |
| K7 | **Thresholds** (those of modules 1 to 5, D5): in each workload, the metric is the number of operations per second of K8. Krummi **meets** the threshold of a workload when its metric is at least 80% of that of Voron (Krummi / Voron ratio ≥ 0.80, with the unrounded values). There is one threshold for each of the 22 workloads, and all of them hold on Windows and on Linux, each report with its own. The other engines have no threshold, nor does the store size (K9). |
| K8 | **Measurement**: each workload is run, for each engine, once without measuring and then three measured times; its result is the **median** of the three. The time of a repetition goes from its first operation until it returns its last commit or its last read; it does not include generating the data, preparing the store, opening it or closing it, except in W16, nor the work the engine does in the background afterwards. In the workloads with several threads, the threads are created beforehand and start at the same time. The metric of a repetition is its number of operations (*Operations* column) divided by its time in seconds, and that of the workload is the median of the metrics of its three measured repetitions, computed separately for W12 and for W13. The work an engine leaves in the background after its last commit (compacting, moving what was committed) does not count in any engine; the close pays for it, and it is not measured. **Reading a value** means going through all its bytes and accumulating them in a checksum; when the workload also reads the key (W10, W11 and, once per key, W20), its bytes go into the same checksum. If the engine's interface only returns a copy, the copy is part of the time. A read that does not find its key, or a checksum that does not match that of the written data, is a failure of the engine (*Validations and messages*). |
| K9 | **Store size**: after the last measured repetition of W1, W2, W3, W14, W15, W17, W19 and W21, with the store closed without error, the benchmark measures the sum of the lengths of all the files in the store folder, including its subfolders, the same for all the engines; if an engine leaves preallocated files larger than its data, the figure reflects it without correcting it. It is informative and has no threshold. |
| K10 | **The command** (that of AC19 of the parent): `cargo run --release -p krummi-bench -- --ravendb <carpeta> [--carpeta <carpeta>] [--informe <archivo>] [--reducido]`. The options go in any order, each one only once and with its value as the next argument. `--ravendb` is the folder with the RavenDB `7.2.7` code; `--carpeta`, an existing folder where the benchmark creates the stores (the system temporary folder if not given); `--informe`, the report file (`banco-<fecha>-<hora>.md` in the current folder if not given, with the local start date and time as `AAAAMMDD` and `HHMMSS`). The benchmark creates its stores inside a new subfolder of its own in the working folder and deletes it when it ends, also if it fails or is interrupted. |
| K11 | **Reduced run**: with `--reducido`, the quantities are those of the *Reduced* column of the workload table, with a single measured repetition and without the warm-up one. The sizes of keys, values and writes and the number of threads do not change. It serves to check that the benchmark works; the thresholds are not evaluated and the report says so. |
| K12 | **Continuous integration**: in every continuous integration run, on Windows and on Linux, the benchmark is run with `--reducido` and must end with exit code `0`. To do so it obtains the .NET 10 SDK and the RavenDB `7.2.7` code; if it cannot download them, the run fails for that external cause. The thresholds are evaluated only in full runs on the maintainer's machine. |
| K13 | **Missing a threshold** (refines the exit codes of the parent): the report marks each threshold as met or missed, and the command ends with code `1` if any is missed, even if the run has finished and written its report, like the verification that finds damage. This module closes with the benchmark and its first report even if Krummi misses thresholds: each miss becomes a separate change of the affected module, with its own PRD. AC19 of the parent is required before each version of the product, not to close this module (annotation to AC19). |
| K14 | **The benchmark is not part of the product** (refines D2 and D12): it is a maintainer tool, not part of the product's public surface, and its dependencies (the other engines, the .NET SDK) do not reach the Krummi library. It may use unsafe Rust code only where another engine's public interface requires it. The check of AC21 still holds for the engine. |

### Workloads

*Operations* is what the metric counts (K8) in a full run; *Reduced* gives the quantities of the reduced run (K11).
The names are those the report uses.

| ID | Name | Starts at | What it does | Operations | Reduced |
|---|---|---|---|---|---|
| W1 | `carga-en-orden` | Empty store | Stores 1,000,000 entries in key order, in writes of 1,000. | 1,000,000 | 10,000 entries |
| W2 | `carga-al-azar` | Empty store | Stores 1,000,000 entries at random, in writes of 1,000. | 1,000,000 | 10,000 entries |
| W3 | `carga-masiva` | Empty store | Stores 1,000,000 entries at random in a single write. | 1,000,000 | 10,000 entries |
| W4 | `escrituras-sueltas` | Base store | One thread stores 10,000 new entries at random, one per write. | 10,000 | 160 entries (a multiple of 16, like W5 and W6) |
| W5 | `escrituras-concurrentes-4` | Base store | 4 threads store 10,000 new entries in total at the same time, 2,500 each, one per write. | 10,000 | 160 entries, 40 per thread |
| W6 | `escrituras-concurrentes-16` | Base store | 16 threads store 10,000 new entries in total at the same time, 625 each, one per write. | 10,000 | 160 entries, 10 per thread |
| W7 | `lecturas-al-azar` | Base store | One thread reads all the keys at random in a single read transaction. | 1,000,000 | 10,000 reads |
| W8 | `lecturas-al-azar-4` | Base store | 4 threads, each in its own read transaction, each read all the keys in its own random order. | 4,000,000 | 40,000 reads |
| W9 | `lecturas-al-azar-16` | Base store | Like W8, with 16 threads. | 16,000,000 | 160,000 reads |
| W10 | `recorrido` | Base store | Scans all the entries forward in a read transaction, reading the key and the value of each one. | 1,000,000 | 10,000 entries |
| W11 | `intervalos` | Base store | In a read transaction, 100,000 forward scans, each from an existing key chosen at random, that read up to 100 entries (fewer if they reach the end). Because of the fixed seed, the number of entries read is the same in all the engines. | Entries read | 1,000 scans |
| W12 | `lecturas-con-escritor` | Base store | 4 threads read random keys, in read transactions of 1,000 reads each, while another thread makes 1,000 writes of 100 new entries. Each reader goes through its own random order of the keys of the base store and starts it again if it exhausts it. The time is the writer's, from its first operation to its last commit; it counts each individual read that finishes in that time. | Reads finished | 10 writes of 100 |
| W13 | `escritor-con-lectores` | The same repetition as W12 | The writer of W12; it counts the entries stored, not the commits. | 100,000 | 1,000 entries |
| W14 | `borrado` | Base store | Removes all the entries at random, in writes of 1,000. | 1,000,000 | 10,000 entries |
| W15 | `reutilizacion` | Base store | Removes half of the entries at random and then stores as many new ones at random, in writes of 1,000. | 1,000,000 | 5,000 and 5,000 entries |
| W16 | `apertura` | Closed base store | 100 times: opens the store, reads one random key and closes it. The time includes opening and closing. | 100 | 10 opens |
| W17 | `valores-grandes-carga` | Empty store | Stores 4,096 values of 64 KiB (256 MiB) with random keys, in writes of 256. | 4,096 | 64 values |
| W18 | `valores-grandes-lectura` | The store of W17 | Reads all the values at random in a read transaction. | 4,096 | 64 reads |
| W19 | `multivalor-carga` | Empty store | Adds 1,000,000 values of 16 bytes to 100,000 keys, 10 per key, in a random order that interleaves the keys, in writes of 1,000. | 1,000,000 | 10,000 values in 1,000 keys |
| W20 | `multivalor-lectura` | The store of W19 | In a read transaction, goes through the values of each key, with the keys at random. | 1,000,000 | 10,000 values |
| W21 | `tamano-fijo-carga` | Empty store | Stores 1,000,000 distinct 64-bit integer keys, at random, with values of 8 bytes, in writes of 1,000. | 1,000,000 | 10,000 entries |
| W22 | `tamano-fijo-lectura` | The store of W21 | Reads all the keys at random in a read transaction. | 1,000,000 | 10,000 reads |

Each workload covers at least one of modules 1 to 5: the Pager (W1–W3 and W16, and the sizes of K9), transactions (W8,
W9, W12 and W13), the journal and group commit (W4–W6 and W13), free space (W14 and W15, and their sizes) and the trees
(all of them; W10 and W11 for the scan, W17 and W18 for large values, W19 and W20 for multi-value ones, W21 and W22 for
fixed-size ones). In W5 and W6, the engines with a single writer queue the threads' writes (G4); Krummi in addition
brings them to disk grouped (E4).

### Starting state

Verified on 2026-11-03 in the repository and in the public sources of each engine:

| Aspect | Situation |
|---|---|
| Benchmark | It does not exist. There is no benchmark package nor comparison with other engines. |
| Measurements of Krummi | There are example programs that measure only Krummi, without a threshold: those of F13, E16, V11, M15 and X13, and those of the tree workloads, the scan and the [remembered node layout check](trees-node-check-cache.prd.en.md). They share a seeded random key generator and a Markdown table output. |
| Continuous integration | It runs formatting, clippy, tests and the acceptance sweeps on Windows and on Linux. It runs no measurement and has no .NET. |
| Voron | It has no package published on NuGet; it is obtained only by compiling the RavenDB code. The latest stable version is `7.2.7`, whose Voron requires .NET 10. License without a key: AGPLv3. |
| LMDB (heed) | heed `0.23.1`, published on 2026-10-26; it compiles LMDB from C. Opening an environment is an unsafe function of its interface. |
| redb | `4.4.0`, published on 2026-10-13. B+ tree in Rust, one writer, with multi-value tables and integer keys. |
| canopydb | `0.2.5`, published on 2025-11-22. B+ tree in Rust; its default write transaction is single-writer and it offers optimistic multi-writer ones separately; the durable commit needs its journal enabled. Its continuous integration tests on Windows and on Linux. Its author declares it to be at an early stage. |
| fjall | `3.1.13`, published on 2026-10-25. LSM tree in Rust, with single-writer and optimistic transactions. |

## Actors

- **Maintainer**: runs the full benchmark on Windows and on Linux before each version, reads the report and, when a
  threshold is missed, opens the change of the affected module (K13).
- **Continuous integration**: runs the reduced benchmark on every change (K12).

## Flow

1. The maintainer has the .NET 10 SDK and a folder with the RavenDB `7.2.7` code.
2. They run the command of K10. The benchmark checks its options and requirements in the order of
   [Validations and messages](#validations-and-messages) before measuring anything, and compiles the Voron program.
3. For each engine, in turn (K3), it prepares the stores it needs and runs each workload with its warm-up repetition
   and its three measured repetitions (K8), and measures the size of the stores (K9). Meanwhile it writes its progress
   to the error output.
4. When it finishes, it writes the report, deletes its working subfolder and writes the summary to the standard
   output.
5. It ends with code `0` if Krummi meets all the thresholds or the run is reduced, and with `1` if it misses any
   (K13).

## Validations and messages

They are checked in this order; the first one that applies wins. Each message goes on its own line in the error
output. `<carpeta>`, `<archivo>` and `<destino>` are shown as the maintainer wrote them, or as the benchmark forms
them when they are the defaults. The arguments are checked from left to right and the first invalid one wins;
`<opción>` is that argument as received, the second occurrence of a repeated option, or the name of the option that
lacks its value (with `--carpeta --informe`, `--carpeta`). The folder and destination messages are the parent's; each
one is used where the parent uses it, which is why a folder that does not exist gives different messages depending on
whether it is the working one (source of the stores), the report's (destination) or the `--ravendb` one
(requirement).

| Situation | Rule | Message | Code |
|---|---|---|---|
| An argument that is not one of the four options (also `--opción=valor` or a loose argument), a repeated option, or one with a value that lacks it, or whose value starts with `--` | The command does nothing. | `Opción no válida: <opción>.` | `2` |
| `--ravendb` is missing | The command does nothing. | `Falta la opción --ravendb con la carpeta del código de RavenDB 7.2.7.` | `2` |
| The report file already exists, also as a folder | The command does nothing. | `El destino <destino> ya existe.` | `1` |
| The folder where the report would go does not exist or cannot be used | The command does nothing. | `No se puede usar la carpeta <carpeta>: <causa>.` | `1` |
| The `--ravendb` folder does not exist or does not contain the RavenDB `7.2.7` code | The command does nothing. | `La carpeta <carpeta> no contiene el código de RavenDB 7.2.7.` | `1` |
| There is no .NET SDK of version 10 | The command does nothing. | `No se encuentra el SDK de .NET 10, necesario para medir Voron.` | `1` |
| The working folder does not exist | The command does nothing. | `La carpeta <carpeta> no existe.` | `1` |
| The working folder cannot be used | The command does nothing. | `No se puede usar la carpeta <carpeta>: <causa>.` | `1` |
| The compilation of the Voron program fails | The run stops before measuring and deletes its working subfolder. | `No se pudo compilar Voron: <causa>.` | `1` |
| An engine fails when preparing or running a workload (including a full disk, a read that does not find its key, a checksum that does not match and a process that ends without finishing the workload) | The run stops, deletes its working subfolder and writes no report. | `El motor <motor> falló en la carga <carga>: <causa>.` | `1` |
| The report cannot be written at the end, also because it already exists | It deletes its working subfolder and leaves no half-written report. | `No se pudo escribir el informe <archivo>: <causa>.` | `1` |
| The run is interrupted with Ctrl+C | Same as a failure: it ends the engines' processes, deletes its subfolder and writes no report. | `Banco interrumpido.` | `1` |

`<motor>` is the name of the engine as it appears in the report; `<carga>`, the name of the workload in the table whose
process was running, also if the failure is when opening, closing or measuring the size of its store, or, if the
failure is when preparing a starting store, that of the first workload that was going to use it (`lecturas-con-escritor`
for W12 and W13); `<causa>`, the text of the error given by the engine, the compilation or the system.

## Output

**Standard output**, when it finishes, a single line, with `<archivo>` as in *Validations and messages*:

```text
Krummi cumple 21 de 22 umbrales. Informe: banco-20261103-101500.md
```

With `--reducido`: `Ejecución reducida, sin umbrales. Informe: <archivo>`.

**The report** is a Markdown file in Spanish with this structure, the same on Windows and on Linux. In the example, the
figures are illustrative and the tables show only some rows:

```markdown
# Banco comparativo de Krummi

- Fecha: 2026-11-03 10:15 (UTC+01:00)
- Ejecución: completa
- Sistema: Windows 11 Pro 10.0.26200
- Procesador: <modelo>, <núcleos lógicos> núcleos lógicos
- Memoria: <GiB> GiB
- Disco: <modelo>, <sistema de archivos>
- Krummi: <commit> (con cambios sin confirmar)
- Motores: Voron 7.2.7 (.NET <versión>), LMDB <versión> (heed 0.23.1), redb 4.4.0, canopydb 0.2.5, fjall 3.1.13
- Rust: <versión>
- Repeticiones: 1 previa y 3 medidas, mediana

## Resultado

Krummi cumple 21 de 22 umbrales. Incumple: `escrituras-concurrentes-16`.

## Operaciones por segundo

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall | Krummi / Voron | Umbral |
|---|---|---|---|---|---|---|---|---|
| carga-en-orden | 812345 | 790122 | 1012345 | 501234 | 700123 | 1500234 | 1,02 | cumple |
| tamano-fijo-carga | 950000 | 910000 | 1200000 | 600000 | no aplica | no aplica | 1,04 | cumple |

## Tamaño del almacén (bytes)

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall |
|---|---|---|---|---|---|---|
| carga-en-orden | 150994944 | 160432128 | 146800640 | 170000000 | 155000000 | 120000000 |

## Opciones cambiadas

- LMDB: tamaño del mapa de 64 GiB; claves duplicadas ordenadas en W19 y W20; claves enteras en W21 y W22.
- redb: caché de 1 GiB.
- canopydb: caché de 1 GiB; diario activado; confirmación que espera al disco.
- fjall: caché de bloques de 1 GiB; llevar al disco tras cada confirmación.
```

Rules of the form:

- *Operaciones por segundo* has one row per workload, the 22 in the order of the workload table, and *Tamaño del
  almacén* one for each workload of K9, in the same order. The engine columns are always the six, in the order of K3.
- The operations per second and the sizes are integers, rounded to the nearest, with no thousands separator. The
  Krummi / Voron ratio carries two decimals with a comma, **truncated**, not rounded, so that a displayed ratio of
  `0,80` or more always meets the threshold.
- The *Umbral* column says `cumple` or `incumple`. In a reduced run, *Ejecución* says `reducida`, *Repeticiones* says
  `1 medida`, *Resultado* says `Ejecución reducida, sin umbrales.` and the *Umbral* column says `sin evaluar`; the
  other columns and the size table are shown the same.
- With all the thresholds met, *Resultado* says `Krummi cumple 22 de 22 umbrales.` without the *Incumple* sentence;
  with several missed, the sentence lists them between backticks, separated by commas, in the order of the table.
- *Fecha* is that of the start of the run, the same as in the default name of the report. *Sistema* is the name and
  version of the operating system; on Linux, the distribution with its version and the kernel version. The LMDB
  version is that of the C library that heed compiles.
- *Krummi* gives the first 12 characters of the identifier of the last commit of the working copy and adds
  `(con cambios sin confirmar)` only when it has changes in tracked files; untracked files do not count. If there is
  no repository or commit, it says `desconocido`. *Disco* is that of the working folder.
- A piece of machine data that the benchmark cannot obtain is shown as `desconocido`.
- *Opciones cambiadas* also names how each engine brings its commits to disk when the benchmark changes it (K3). It
  has one line per engine with some option changed, in the order of K3, with its options separated by semicolons; if no
  engine has changed options, the section says `Ninguna.`

## Reuse

- The seeded random key generator and the table output of the existing measurements of Krummi.
- The parent's messages for a folder that does not exist, an unusable folder and a destination that already exists
  (*Validations and messages*).
- The parent's output conventions (*Output*): results on the standard output, errors on the error output, and the
  codes `0`, `1` and `2`, with the refinement of K13.

## Out of scope

- Optimizing Krummi to meet a threshold: each miss is a separate change (K13).
- Thresholds of the modules after the 5th: each module with an impact on performance adds its workloads and its
  threshold to this benchmark (D5).
- Store size thresholds (K9).
- Other engines (sled, RocksDB, SQLite and others) and other ways of using the chosen engines: Voron through the
  RavenDB server, LMDB with another library, optimistic multiple writers in canopydb or fjall (K3).
- Per-engine tuning beyond K3: .NET ahead-of-time compilation, another memory allocator, performance options.
- Non-durable modes (the deferred mode of J10 and equivalents) and temporary mode (G9).
- Data that does not fit in memory, cold cache (emptying the system cache) and network disks.
- Measuring memory or CPU use, recovery after a crash and the size of the journal separately.
- Measuring the dispersion between repetitions or flagging unstable results.
- Other platforms (D3), and virtual machines, containers or WSL as the environment of the full runs.
- Preparing the machine (antivirus, power mode, background processes) and checking beforehand that there is enough
  disk and memory: it is the maintainer's responsibility.
- Choosing from the command which engines or which workloads to run, and resuming an interrupted run.
- A time limit per workload: an engine that hangs is interrupted with Ctrl+C, and in continuous integration the job's
  own limit cuts it off.
- Automatically checking each engine's durability against a power cut: its configuration is reviewed (AC-K8).
- Charts, run history, comparison between reports and output in formats other than Markdown.
- Publishing the results.

## Left to the implementation

- How the benchmark controls the Voron program and how it compiles it from the `--ravendb` folder.
- How it is checked that the `--ravendb` folder is of version `7.2.7`.
- The options each engine needs to meet K3 and to offer the structures of K6, such as the map size of LMDB or the byte
  order of integer keys; all of them are listed in the report.
- How the benchmark prepares a starting store (by loading it or by copying a closed one), as long as it has exactly the
  content of K4.
- How the data is generated with the fixed seed, as long as all the engines receive the same bytes in the same order
  (K4).
- The format of the progress on the error output and the name of the working subfolder.

## Acceptance criteria

They check this module. AC19 of the parent is checked by the maintainer before each version (K13).

1. **AC-K1** — Given a full run on Windows and another on Linux, when they finish, then each one writes a report with
   the structure of [Output](#output), the 22 workloads, the six engines in their versions of K1 and `no aplica`
   exactly in the canopydb and fjall cells of W19 to W22, in both tables.
2. **AC-K2** — Given results per workload and engine and machine data injected for the test, when the benchmark writes
   the report and the standard output line, then they are identical byte for byte to those expected for each case: all
   thresholds met, one missed, several missed, a ratio of `0,7996` (which is shown as `0,79` and misses), an exact
   ratio of `0,80` (for example 800 against 1000, which is shown as `0,80` and meets), reduced run, no changed options
   and unknown machine data.
3. **AC-K3** — Given injected results, when the benchmark evaluates each threshold, then it says `cumple` if and only
   if Krummi's metric is at least 0.80 times Voron's unrounded (K7), and the exit code is `0` if all say `cumple` and
   `1` if any says `incumple`.
4. **AC-K4** — Given injected times and numbers of operations per repetition, also different between repetitions as in
   W11 and W12, when the benchmark computes the metric of a workload, then it uses the median of the metrics of the
   three measured repetitions, separately for W12 and W13, and discards the warm-up one; and in a reduced run, the
   metric of the single repetition (K8, K11).
5. **AC-K5** — Given a run with `--reducido`, when it finishes, then each engine ran the quantities of the *Reduced*
   column, the standard output is `Ejecución reducida, sin umbrales. Informe: <archivo>` and the exit code is `0`
   (K11).
6. **AC-K6** — Given continuous integration, when it runs on Windows and on Linux, then it runs the benchmark with
   `--reducido` and fails if it does not end with code `0` (K12).
7. **AC-K7** — Given the fixed seed, when the data of each workload is generated for each engine and in two runs, then
   all of them receive the same key and value bytes in the same order (K4).
8. **AC-K8** — Given each engine, when it is reviewed how the benchmark uses it, then each commit uses the option or
   operation that the engine's documentation gives for bringing the changes to the physical medium (K3), the review is
   written with the citation of that documentation in the module's closing report, and the benchmark's report names
   that option in *Opciones cambiadas* when it is not the default.
9. **AC-K9** — Given each situation of [Validations and messages](#validations-and-messages), when the command is run,
   then it shows its exact message on the error output, ends with its code, writes no report and the working folder
   stays as it was.
10. **AC-K10** — Given an engine that fails in a workload, that reads a key it does not find or whose checksum does not
    match, simulated for the test, when the benchmark detects it, then it stops with the message of the table and the
    working folder stays as it was.
11. **AC-K11** — Given a run, when the processes are observed, then two engines are never measured at the same time,
    each workload of each engine runs in its own process (W12 and W13 in a single one) and the order of engines and
    workloads is that of K3.
12. **AC-K12** — Given a closed store of known content, with subfolders, when the benchmark measures its size, then it
    gives the sum of the lengths of all its files (K9).
13. **AC-K13** — Given a run without `--informe`, when it finishes, then the report is in the current folder with the
    name of K10 and the working subfolder no longer exists.
14. **AC-K14** — Given the Krummi library, when its dependency tree is computed, then it contains no package of the
    benchmark nor of the other engines, and the check of AC21 of the parent still passes (K14).

## Annotations to other PRDs

Added on 2026-11-03 to the [root PRD](krummi.prd.en.md) and its translation:

- **Root PRD, objective 3 and D5**: "competitive" means the thresholds of K7, against Voron; exception to D5: Voron,
  which is not a Rust engine, enters the benchmark and is the reference of the thresholds (K2, K7).
- **Root PRD, D2 and D12**: the benchmark is a maintainer tool, not part of the public surface; it may use unsafe code
  only where another engine's interface requires it (K14).
- **Root PRD, *Output***: the benchmark command ends with `1` when Krummi misses a threshold (K13).
- **Root PRD, AC19**: the thresholds in force are those of K7; AC19 is required before each version, not to close
  module 6 (K13).

## References

- Parent: [Krummi — root PRD](krummi.prd.en.md) (objective 3, D2, D5, D12, *Output*, AC19, AC21).
- Earlier measurements of Krummi: [free space](free-space.prd.en.md) (F13),
  [journal group commit](journal-group-commit.prd.en.md) (E16), [large values](trees-large-values.prd.en.md) (V11),
  [multi-value trees](trees-multivalue.prd.en.md) (M15), [fixed-size trees](trees-fixed-size.prd.en.md) (X13).
- Voron, in the RavenDB repository: <https://github.com/ravendb/ravendb/tree/7.2.7/src/Voron>
- LMDB through heed: <https://github.com/meilisearch/heed>
- redb: <https://github.com/cberner/redb>
- canopydb: <https://github.com/arthurprs/canopydb>
- fjall: <https://github.com/fjall-rs/fjall>

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| e5a00060 | 2026-11-03 07:01 UTC+00:00 | 2 h 18 min | 1 h 29 min | 49 min | 24 min | 25 min | 7 |

<!-- prd-time: [{"session": "e5a00060-eace-41af-8b65-01e51beb6679", "start": "2026-11-03T07:01:57.102000+00:00", "wall": 8284, "discarded": 5367, "agent": 1422, "human": 1493, "interventions": 7, "idle_minutes": 10}] -->
