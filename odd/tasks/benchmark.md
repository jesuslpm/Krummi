# Comparative benchmark (module 6) — ODD execution document

Objective: implement [benchmark.prd.md](../../docs/prd/krummi/benchmark.prd.md) (decisions K1–K14, workloads W1–W22,
criteria AC-K1–AC-K14), with its ancestry (krummi.prd.md). Branch: `feat/benchmark` (PRD commit `8008b69`, branch
point `aed68a6`). Delivery: work-unit commits on the branch, unmerged. Test runner: `cargo test --workspace`
(Windows; Linux in WSL 2 Ubuntu). Estimate: 7 h – 11 h. Real start: 2026-11-03 10:11:07 +0000.

## Specs

Each S# quotes one acceptance criterion of the PRD verbatim (Spanish, authoritative). The decisions K1–K14 and the
workload table of the PRD are part of every spec; *Fuera de alcance* limits them and *Dejado a la implementación* is
the only room for the design choices below.

- **S1** — **AC-K1** — Dada una ejecución completa en Windows y otra en Linux, cuando terminan, entonces cada una escribe un informe con la estructura de [Salida](#salida), las 22 cargas, los seis motores en sus versiones de K1 y `no aplica` exactamente en las celdas de canopydb y fjall de W19 a W22, en las dos tablas.
- **S2** — **AC-K2** — Dados unos resultados por carga y motor y unos datos del equipo inyectados para la prueba, cuando el banco escribe el informe y la línea de la salida estándar, entonces son idénticos byte a byte a los esperados para cada caso: todos los umbrales cumplidos, uno incumplido, varios incumplidos, una razón de `0,7996` (que se muestra `0,79` e incumple), una razón exacta de `0,80` (por ejemplo 800 frente a 1000, que se muestra `0,80` y cumple), ejecución reducida, sin opciones cambiadas y datos del equipo desconocidos.
- **S3** — **AC-K3** — Dados unos resultados inyectados, cuando el banco evalúa cada umbral, entonces dice `cumple` si y solo si la métrica de Krummi es al menos 0,80 veces la de Voron sin redondear (K7), y el código de salida es `0` si todas dicen `cumple` y `1` si alguna dice `incumple`.
- **S4** — **AC-K4** — Dados unos tiempos y unos números de operaciones por repetición inyectados, también distintos entre repeticiones como en W11 y W12, cuando el banco calcula la métrica de una carga, entonces usa la mediana de las métricas de las tres repeticiones medidas, por separado para W12 y W13, y descarta la previa; y en una ejecución reducida, la métrica de la única repetición (K8, K11).
- **S5** — **AC-K5** — Dada una ejecución con `--reducido`, cuando termina, entonces cada motor ejecutó las cantidades de la columna *Reducida*, la salida estándar es `Ejecución reducida, sin umbrales. Informe: <archivo>` y el código de salida es `0` (K11).
- **S6** — **AC-K6** — Dada la integración continua, cuando se ejecuta en Windows y en Linux, entonces ejecuta el banco con `--reducido` y falla si no termina con código `0` (K12).
- **S7** — **AC-K7** — Dada la semilla fija, cuando se generan los datos de cada carga para cada motor y en dos ejecuciones, entonces todos reciben los mismos bytes de claves y valores en el mismo orden (K4).
- **S8** — **AC-K8** — Dado cada motor, cuando se revisa cómo lo usa el banco, entonces cada confirmación usa la opción u operación que la documentación del motor da para llevar los cambios al medio físico (K3), la revisión queda escrita con la cita de esa documentación en el informe de cierre del módulo, y el informe del banco nombra esa opción en *Opciones cambiadas* cuando no es la predeterminada.
- **S9** — **AC-K9** — Dada cada situación de [Validaciones y mensajes](#validaciones-y-mensajes), cuando se ejecuta la orden, entonces muestra su mensaje exacto en la salida de errores, termina con su código, no escribe informe y la carpeta de trabajo queda como estaba.
- **S10** — **AC-K10** — Dado un motor que falla en una carga, que lee una clave que no encuentra o cuya suma no coincide, simulado para la prueba, cuando el banco lo detecta, entonces se detiene con el mensaje de la tabla y la carpeta de trabajo queda como estaba.
- **S11** — **AC-K11** — Dada una ejecución, cuando se observan los procesos, entonces nunca miden dos motores a la vez, cada carga de cada motor corre en su propio proceso (W12 y W13 en uno solo) y el orden de motores y cargas es el de K3.
- **S12** — **AC-K12** — Dado un almacén cerrado de contenido conocido, con subcarpetas, cuando el banco mide su tamaño, entonces da la suma de las longitudes de todos sus archivos (K9).
- **S13** — **AC-K13** — Dada una ejecución sin `--informe`, cuando termina, entonces el informe está en la carpeta actual con el nombre de K10 y la subcarpeta de trabajo ya no existe.
- **S14** — **AC-K14** — Dada la biblioteca de Krummi, cuando se calcula su árbol de dependencias, entonces no contiene ningún paquete del banco ni de los demás motores, y la comprobación de AC21 del padre sigue pasando (K14).

## Design

Decisions within *Dejado a la implementación*:

- **D1 Layout.** Package `krummi-bench` in `crates/krummi-bench`: a library (`src/lib.rs`) with everything the
  orchestrator and the Rust worker share, the default binary `krummi-bench` (orchestrator, the K10 command) and a
  second binary `krummi-bench-worker` (Rust engines). Voron is driven by a .NET program in
  `crates/krummi-bench/voron/` (`VoronBench.csproj`) that references `<ravendb>/src/Voron/Voron.csproj` through the
  MSBuild property `RavenDbRoot` and copies `librvnpal` and `libzstd` from `<ravendb>/libs`.
- **D2 Building Voron.** `dotnet restore --configfile <ravendb>/NuGet.Config` and `dotnet build -c Release
  --no-restore`, both with `--artifacts-path <work subfolder>/dotnet` and the work subfolder as current directory, so
  the repository and the RavenDB folder stay untouched (probed: `git status` of RavenDB clean after a build; 46 s).
- **D3 Version check.** The `--ravendb` folder is RavenDB 7.2.7 when `src/Voron/Voron.csproj` exists and
  `src/CommonAssemblyInfo.cs` contains `AssemblyInformationalVersion("7.2.7")`. The SDK check: a line of
  `dotnet --list-sdks` that starts with `10.`.
- **D4 Worker protocol.** The orchestrator starts one worker process per (engine, load) (W12 and W13 together) with
  positional arguments: engine, load, scale (`full`/`reduced`), store folder and, for loads that start from another
  store, the source folder. The worker prints one line per repetition on stdout:
  `rep <index> <load> <operations> <nanoseconds>` (index 0 is the warm-up), and nothing else; a failure is one line
  on stderr and exit code 1. A `digest <load>` command prints the SHA-256 of the data stream of a load, so the
  orchestrator checks before measuring that the Rust and .NET generators agree (AC-K7 on every run).
- **D5 Data.** SplitMix64 as in the timing examples. Key of index `i`: bytes 0..8 `splitmix(i ^ 0x5EED_0001)` and
  8..16 `splitmix(!i)`, big-endian (a bijection, so distinct indices give distinct keys). Value of index `i` and
  length `n`: the big-endian bytes of `splitmix(i * 0x1_0000 + j ^ 0xBA5E_0000_0000_0000)` for `j = 0, 1, …`,
  truncated to `n`. Each load has its own index range (base `0..N`, new keys after it), so new keys are distinct from
  the base and from each other. Permutations: Fisher–Yates with a SplitMix64 state seeded per load and thread.
  Fixed-size keys: `splitmix(i ^ 0xF1DE_0000)` (a bijection). Multi-value: key index `k`, value index
  `k * 10 + v`, values 16 bytes.
- **D6 Checksums.** The worker computes the expected sum of every byte a repetition reads before starting the
  timer; reading adds every byte (key bytes too for W10, W11, W20); a mismatch or a missing key is a failure.
- **D7 Stores.** The orchestrator prepares the base store once per engine (a worker `prepare` command, not
  measured), closes it, and gives every process a fresh copy (file copy of the closed store). Loads that start from
  W17/W19/W21 get a copy of the last measured repetition of that load. Sizes (K9) are measured by the orchestrator
  on the last repetition's folder, which the worker leaves closed.
- **D8 Engine options.** LMDB: map size 1 GiB (on Windows the data file takes the whole map, K9, and every repetition copies it, so the map is kept small), `DUP_SORT` for
  W19–W20, native-endian `u64` keys with `INTEGER_KEY` for W21–W22. redb: cache 1 GiB, `Durability::Immediate`,
  multimap and `u64` tables. canopydb: page cache 1 GiB (its default, so not listed as changed), WAL on, `commit_with(true)`, `begin_write`. fjall:
  `SingleWriterTxDatabase`, block cache 1 GiB, `durability(Some(PersistMode::SyncData))`. Krummi and Voron:
  defaults. canopydb and fjall have no multi-value or integer keys (K6).
- **D9 Reports.** The first complete reports go to `docs/benchmark/2026-11-03-windows.md` and `-linux.md`.
- **D1 revised (commit `838fb35`).** There is no second binary: `cargo run` does not promise to build it. The Rust
  worker is the same `krummi-bench` executable started with `KRUMMI_BENCH_ROLE=worker` (not an option of K10). The
  protocol, including where stores live (`<store>/rep-<i>`) and who copies them (the workers), is the doc comment of
  `crates/krummi-bench/src/protocol.rs`, which supersedes D4 and D7 where they differ.
- **D10 Load semantics** (both workers implement exactly this; `N` = `Scale::base_entries`, `p` = `params(load,
  scale)`, `B` = `index_base(load)`, `value(i)` = `value(i, 100)` unless said; trees: normal `bench`, multi-value
  `multi`, fixed-size `fixed` with value length 8; every write commits durably; "digest" lists what the `data` line
  feeds, in order):
  - *prepare* (base store): indices `0..N` in increasing order, `key(i)`/`value(i)`, writes of 1,000.
  - W1: indices `0..N` sorted by `key(i)` bytes; W2: increasing order; W3: increasing order in one write. Digest:
    key and value of every insert.
  - W4, W5, W6: thread `t` of `p.threads` inserts `B + t·per + j` for `j` in `0..per` (`per = p.entries /
    p.threads`), one entry per write, all threads started together. Digest: threads in order, key and value.
  - W7: one read transaction, `get key(i)` for `i` in `read_order(W7, 0, N)`; W8, W9: thread `t` in its own read
    transaction, `read_order(load, t, N)`. Sum: the value bytes. Digest: threads in order, the keys.
  - W10: one read transaction, the whole tree forward; sum: key and value bytes; must see `N` entries. Digest:
    `feed_u64(N)`.
  - W11: one read transaction; starts are the first `p.ranges` of `permutation(N, seed(W11, 0))`; each scans forward
    from `key(start)` included and reads up to 100 entries. Operations: entries read. Sum: key and value bytes
    (expected from the base keys sorted, with prefix sums). Digest: the start keys.
  - W12 and W13 (one process): 4 reader threads, reader `t` cycles through `read_order(W12, t, N)` (restarting when
    exhausted), opening a new read transaction every 1,000 reads; one writer does `p.writes` writes of `p.batch`
    entries `B + w·batch + j`. All five start together; the time is the writer's, from its first operation to its
    last commit; a reader checks after every read whether the writer finished and, if so, stops without counting
    that read. W12 operations: the reads counted; W13: `writes · batch`; both lines carry the writer's nanoseconds.
    Readers check each value's byte sum. Digests: W12 the readers' first pass keys, threads in order; W13 the
    writer's keys and values.
  - W14: remove indices `0..N` in increasing order, writes of 1,000. W15: remove `0..N/2`, then insert `B + j` for
    `j` in `0..N/2`, writes of 1,000. Digest: removed keys, then inserted keys and values.
  - W16: per repetition, `p.opens` times: open the store, `get key(j)` for the `j`-th open, close; the time
    includes opening and closing. Digest: the keys.
  - W17: insert `key(B + i)` with `value(B + i, 65536)` for `i` in `0..p.entries`, writes of 256. W18: one read
    transaction, `get key(index_base(W17) + i)` for `i` in `0..p.entries`, sum of value bytes. Digests: W17 keys and
    values, W18 keys.
  - W19: keys `key(B + k)` for `k` in `0..p.keys`; value `v` of key `k` is `value(B + k·10 + v, 16)`; order
    `for v in 0..10 { for k in 0..keys }`, writes of 1,000 adds. W20: one read transaction, for `k` in `0..keys`
    every value of `key(index_base(W19) + k)` in the engine's order; must be 10 per key; sum: the key bytes once
    per key and every value's bytes; operations: values read. Digests: W19 key and value of every add, W20 keys.
  - W21: fixed tree, keys `fixed_key(i)` for `i` in `0..N` with value `value(B + i, 8)`, writes of 1,000. W22:
    one read transaction, `get fixed_key(i)` for `i` in `0..N`, sum of value bytes. Digest: `feed_u64(key)` then
    the value (W21), `feed_u64(key)` (W22).
  - A missing key, a wrong count or a sum that differs from the expected one, computed before the timer, is a
    failure of the engine.
  - Simulated failures for tests (AC-K10): `KRUMMI_BENCH_FAIL=<engine>:<load>:<kind>` makes that worker fail during
    that load: `error` (an engine error), `missing` (a key not found), `checksum` (a sum that differs) or `exit`
    (the process ends with code 3 and no message).

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T0 | — | inline | done: exploration, design D1–D9 | — |
| T1 | S2, S3, S4, S7 (Rust), S12 | inline | done: 26 unit tests | `06a37d3`, `838fb35`, `d9b4822` |
| T2 | S5, S7, S8, S10 (Rust worker) | delegated (parallel with T3) | done: 9 worker integration tests | `24baebb` |
| T3 | S7, S8, S10 (Voron program) | delegated (parallel with T2) | done: selftest equals Rust, 21 reduced processes | `fa65b43` |
| T4 | S5, S9, S10, S11, S13 | delegated (after T2) | done: 8 cli + 8 run tests (ignored without .NET), Ctrl+C by hand | `586bb7f` |
| T5 | S6, S14 | inline | done: CI job, AC21 change for K14, dependency test | `06a37d3` (AC21), `5ca33da` |
| T6 | S1, S8 | inline + background runs | pending | — |
| T7 | all | inline: checks, verification, RDD, Codex, closing | pending | — |

T1: library core (workload table with full and reduced counts, data generator, median and metrics, threshold and
report rendering, folder size, CLI parsing and validation order, protocol types). T2: Rust worker with the five
Rust engine adapters and the 22 loads. T3: .NET program for Voron with the same loads, generator and protocol. T4:
orchestrator (plan, Voron build, store preparation, worker runs, failures, Ctrl+C, report writing) and its
integration tests. T5: CI job, AC21 test change for K14, dependency test. T6: complete runs on Windows and WSL.

Route change (2026-11-03): T4 runs after T2, not with it: both compile the same crate, so a half-written worker
would break the orchestrator's builds and tests (the shared-file rule applies to the shared compilation unit). T3
(.NET) stays parallel with T2.

Route evidence: T2, T3 and T4 are independent units of behavior with disjoint edit surfaces (worker and adapters;
.NET program; orchestrator), each several files and hundreds of lines, so they are delegated in parallel once T1
fixes the shared library and protocol (Writer trigger a). Forecast: T1 about 900 lines, T2 about 1,300, T3 about
900, T4 about 900, T5 about 150, T6 reports.

## Log

- **L1** (2026-11-03, user, verbatim): "Empecemos con la implementación con ODD"
- **L2** (2026-11-03, user decision before the run): the complete Linux run of AC-K1 runs in WSL 2 Ubuntu on ext4 on
  the same machine, and the PRD is annotated to accept it (the PRD put WSL out of scope for complete runs).
- **L3** (2026-11-03): the Codex review of the PRD is pending; round 1 failed on the Codex usage limit (resets
  2026-11-04 06:48). The user chose to close the PRD and run Codex later.
- **L4** (2026-11-03 10:11:07 +0000): estimate 7 h – 11 h confirmed by the user; branch `feat/benchmark`, PRD commit
  `8008b69`.
- **L5** (2026-11-03): T1 done (`06a37d3`); worker becomes a role of the same executable (`838fb35`, D1 revised);
  LMDB map 1 GiB (`d9b4822`); PRD annotated for WSL 2 (`955a590`); T5 done (`5ca33da`). RavenDB 7.2.7 cloned to
  `work/ravendb-7.2.7`; Voron builds from an outside project with RavenDB's NuGet.Config (46 s) and needs
  `librvnpal` and `libzstd` next to the executable. WSL: Rust 1.99, .NET SDK 10.0.403 installed in `~/.dotnet`.
- **L6** (2026-11-03): T3 (`fa65b43`), T2 (`24baebb`) and T4 (`586bb7f`) done. Alignment decided by the parent: W13
  writer keys use `index_base(W13)`; reading timers include opening read transactions; multi-threaded time from the
  barrier release to the last thread end; simulated failures fire at the start of each repetition; failure causes in
  Spanish as the Voron worker prints them. canopydb's 1 GiB page cache is its default and no longer listed as a
  changed option. heed 0.23 deprecates `INTEGER_KEY`; `IntegerComparator` sets `MDB_INTEGERKEY` itself. AC-R5's
  package list gains `krummi-bench`. Seam check: `cargo fmt --all --check` ok, clippy clean, `cargo test --workspace`
  512 passed, 0 failed, 30 ignored; T4 ran `cargo test --release -p krummi-bench -- --include-ignored` all green and
  a real reduced run (exit 0, 76 s).
