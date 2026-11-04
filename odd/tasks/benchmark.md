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
  defaults. canopydb and fjall have no multi-value or integer keys (K6). Every engine is built with its default
  Cargo features (`7286171`), so fjall keeps its default LZ4 block compression. Voron's write transactions wait for
  the write lock up to 10 minutes; Voron's default limit of 30 s failed W5 on WSL (L10), and an unlimited wait
  (`a881d48`) would hang on a lock that is never released, because the orchestrator has no time limit per process
  (L11). It is a wait limit, not a storage option, and it changes no measurement of a run that finishes, so the
  report does not list it (default taken during the run).
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
    last commit; W12 counts exactly the reads that end in that time (R1 of the Codex code review, rounds 1 and 2).
    The writer takes its start, publishes `WRITING`, writes, publishes `FINISHING`, takes its end and publishes
    `ENDED`. A reader checks the phase after every read: a read it sees end in `WRITING` counts; one it sees end in
    `NOT_STARTED` or `FINISHING` gets its end time and counts if it is in the writer's time; at `ENDED` it stops
    without counting that read. A read ends when its reader records it, right after its value's checksum. W12 operations: the reads counted; W13: `writes · batch`; both lines carry the writer's nanoseconds.
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
| T6 | S1, S8 | inline + background runs | done: Windows 5/22, Linux (WSL 2) 7/22 thresholds met; Voron lock-wait fix | `9ba17b7`, `a881d48`, `e4ac4f2` |
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
- **L7** (2026-11-03): independent verifier (ODD Verify) over the whole feature at `642d5ab`: no blocker; S2–S7,
  S9–S14 met, S1 and S8 pending the complete runs and the closing report. Advisories fixed in `7286171`: the engines
  keep their default features (fjall had lost its default LZ4 compression, against K3); a report path that ends in a
  separator now fails before the run; `ratio_text` no longer loops when Voron has no operations. Not fixed: a hard
  kill leaves the work subfolder (Ctrl+C cleans it); Voron silently drops write-through if its O_DIRECT probe fails
  on a file system (not the case on NTFS or ext4 here).
- **L8** (2026-11-03): RDD. The whole range `aed68a6..7286171` exceeded the reviewers' context budget
  (`lens_context_budget_exceeded`), so it was reviewed as chained committed slices, each from a detached checkout of
  its last commit (clean tree), and the branch was checked out again afterwards. Results:

  | Slice | Lenses | Result | Findings and disposition | Lineage |
  |---|---|---|---|---|
  | `aed68a6..8008b69` (PRD) | — | passive, no review | — | — |
  | `8008b69..d9b4822` (library core) | risk, readability, reliability | approved | R1-001 suggestion (`deny` instead of `forbid` for unsafe): kept, the AC21 keyword scan limits it to the LMDB adapter; R2-001 warning (ratio text and threshold use different comparisons): fixed, the verdict now decides at the boundary; R3 warning (zero Voron metric loops): already fixed in `7286171` | `review-d098d5e49cf43551` |
  | `d9b4822..5ca33da` (CI, dependency test) | risk, reliability | approved | R1-001 suggestion (RavenDB fetched by tag): fixed, CI checks the commit `b844164`; R3 suggestion (dependency test without all features): fixed, `--all-features` | `review-3ac00a52d5953365` |
  | `5ca33da..fa65b43` (Voron worker) | reliability, resilience, readability | approved | R2-001 warning (`SortedByKey` copies the key formula): fixed, it uses `Data.KeyInto`; R2-002 and R3 warnings (malformed `KRUMMI_BENCH_FAIL` ignored): fixed, it fails as in the Rust worker; R2-003 suggestion (Params comment): fixed on both sides; R2-004 suggestion (identity through `ReadOrder`): fixed; R3 warning (selftest without assertions): fixed, it compares with the pinned answers | `review-d408eabb06d29d47` |
  | `fa65b43..24baebb` (Rust worker) | risk, resilience, readability, reliability | approved | R1-001, R4-001, R2, R3-001 warnings (canopydb sets a 1 GiB page cache that the report does not list): fixed, the adapter keeps canopydb's default, which is 1 GiB; R2 warning (multi-value stride 10 unnamed): fixed, `values_per_key`; R2 suggestion (W16 repeats the repetition loop): kept, it must time opening and closing | `review-8afa7395edc131e4` |
  | `24baebb..fa4f9c8` (orchestrator, fixes) | risk, resilience, readability, reliability | approved | R1-001 suggestion and R4/R2 warnings (default features add dependencies and LZ4 in fjall): kept on purpose, K3 asks for each engine's defaults and LZ4 is fjall's default; D8 records it; R4 suggestion (no per-process timeout): kept, out of scope in the PRD (*Fuera de alcance*: Ctrl+C or the CI job limit); R2 warning (kept stores hardcoded): fixed, derived from `Load::start`; R2 suggestion (shadowed name): fixed; R3 warning (Ctrl+C untested): kept, checked by hand in T4, not portable to automate; R3 warning (failed `versions` shows `desconocido`): kept, the PRD shows unknown data as `desconocido` and a broken worker fails at its first load; R3 suggestion (stderr joined): kept, a panic's message is on its second line | `review-d8124be01bd4049e` |
  | `fa4f9c8..4af47d6` (fixes of the findings above) | risk, readability, reliability | approved | R3-001 warning (Voron's W19 stride may not follow `values_per_key`): dismissed with evidence, `voron/Loads.cs:485` already uses `p.ValuesPerKey`; suggestions kept: the 80/79 literals next to `meets`, the failure kinds listed on both sides, a boundary test of the clamp and running `selftest` from CI (the orchestrator already compares every load's digest with Krummi's on every run) | `review-44b9db0772ddd82e` |

- **L9** (2026-11-03): checks after the fixes: `cargo fmt --all --check` ok; clippy clean; `cargo test --workspace` 512
  passed, 0 failed, 30 ignored; `cargo test --release -p krummi-bench -- --include-ignored` with
  `KRUMMI_BENCH_RAVENDB`: 34 + 8 + 8 + 9 passed (the eight end-to-end runs with Voron in 348 s). Selectorless RDD
  status: empty candidate, nothing pending. Next: T6, the complete runs.
- **L10** (2026-11-03): T6. Windows complete run at `7e70319`: exit 1, 5,830 s, "Krummi cumple 5 de 22 umbrales"
  (`9ba17b7`, `docs/benchmark/2026-11-03-windows.md`). First WSL complete run at `9ba17b7` stopped without a report: "El
  motor Voron falló en la carga escrituras-concurrentes-4: Waited for 00:00:30 for transaction write lock, but could
  not get it". Cause: Voron's write lock is a `SemaphoreSlim(1, 1)`, which is not fair, and a write transaction waits
  30 s by default (`StorageEnvironment.cs:684`); durable commits take about 6 ms on WSL (W4 157 op/s). Not
  reproduced in isolation: W5 at full scale with the unfixed worker in WSL passed 4 repetitions (about 153 op/s,
  260 s), so the failure depends on how the lock is handed out and no deterministic RED exists. Fix `a881d48`: the
  Voron writers wait without a limit (D8). RDD assessment of `9ba17b7..a881d48`: medium, `under_budget`, pending in
  the last slice. Second WSL complete run at `a881d48`: exit 1, 6,958 s, "Krummi cumple 7 de 22 umbrales"
  (`docs/benchmark/2026-11-03-linux.md`). Next: the Codex review, which waits for its usage limit (2026-11-04 06:48).
- **L11** (2026-11-03): RDD of the last slice `4af47d6..e4ac4f2` (Voron lock-wait fix, the two reports, docs),
  lenses resilience and reliability: approved, lineage `review-ea064a0969f64ddb`. Findings: R4-001 and R3-001
  warnings (an unlimited wait hangs on a lock that is never released, with no time limit per process): fixed, the
  wait is limited to 10 minutes, so such a lock fails the load with a named error. R3-002 warning (no test covers
  the fix): kept, the failure depends on how an unfair lock is handed out under slow commits and was not reproduced
  in isolation (L10); the complete WSL run at `a881d48` exercised W5 and W6 at full scale. R4-002 and R3-003
  suggestions (the Windows report ran with Voron's 30 s default): kept, the limit only matters when a wait reaches
  it, which fails the run, and the Windows run finished, so no wait reached 30 s and its measurements are the same
  under either limit; the closing report states it, and the generated report is not edited by hand.
- **L12** (2026-11-03): fix `de31a97` (the 10-minute wait), checked with `cargo test --release -p krummi-bench --
  --include-ignored` and `KRUMMI_BENCH_RAVENDB`: 34 + 8 + 8 + 9 passed (the eight end-to-end runs with Voron in
  259 s). RDD of `e4ac4f2..d60b000`, lens resilience: approved without findings, lineage `review-8821f19933732e16`.
  Final checks: `cargo fmt --all --check` ok; clippy clean; `cargo test --workspace` 512 passed, 0 failed, 30
  ignored. The run waits for the Codex reviews of the PRD and the code (usage limit until 2026-11-04 06:48); then
  the closing report.
- **L13** (2026-11-04, user decision): K13 annotated in the PRD and its translation: an optimization that changes
  nothing observable of the product needs no PRD; its acceptance is that the loads it targets meet their threshold in
  the complete runs on Windows and Linux without the other loads getting worse. The first round of the Codex PRD review reviews the
  annotated PRD.
- **L14** (2026-11-04): Codex PRD review, round 1 (`docs/reviews/benchmark/2026-11-04-prd.md`): status `findings`. R1
  major (W12 when Voron completes zero reads): pending, product decision for the user; R2 minor (AC-K1 size-table
  cells): fixed; R3 minor (reduced W17 partial write): fixed. Resolutions in `2026-11-04-prd-resolutions.md`.
- **L15** (2026-11-04, user decision on R1): W12 with a Voron metric of 0 keeps the current behavior (`∞` or
  `0,00`, threshold met), now fixed in *Salida*, AC-K2 and AC-K3; test `a_zero_voron_metric_shows_infinity_or_zero_and_meets`.
- **L16** (2026-11-04): Codex PRD follow-up (`docs/reviews/benchmark/2026-11-04-prd-02.md`): `approved`; R1–R3
  verified resolved. Next: the Codex code review of `aed68a6..HEAD`.
- **L17** (2026-11-04): RDD of `d60b000..7dba24d` (reliability, readability): approved, lineage
  `review-91f58e65622bdb58`. R3 warning (the zero-Voron test is not byte-exact): fixed, it compares the whole W12
  row and the summary line. R2 suggestion (R1 names both a round and a finding): fixed, rounds are "round 1" in
  L13 and L14; the phase log keeps its measured labels unchanged.
- **L18** (2026-11-04): RDD of `7dba24d..6cef8b1` (reliability): approved without findings, lineage
  `review-d17f096f73e107b8`. Codex code review, round 1 (`docs/reviews/benchmark/2026-11-04-code.md`): `findings`,
  R1 and R2 major, R3 and R4 minor, all fixed (`74c8697`, `6afd038`, `6c5ebd1`, `dcd39f8`); resolutions in
  `2026-11-04-code-resolutions.md`. Checks after the fixes: workspace 516 passed; bench with Voron 38 + 8 + 8 + 9.
- **L19** (2026-11-04): RDD of `6cef8b1..a935cc9` (reliability, resilience, risk): approved, lineage
  `review-520f94853eb8bbac`. R4 and R3 warnings (both W12 counts at 0 could fail the end-to-end ratio check):
  dismissed with evidence, `ratio_text(0, 0)` is `0,00` (`measure.rs:47-51`, pinned by its test), which the check
  accepts as a decimal. R4 and R3 suggestions (probe name only by process id): fixed, the name adds the time in
  nanoseconds (`14730a4`). R3 suggestion (permissions not restored if an assertion panics): fixed with a drop guard.
- **L20** (2026-11-04): RDD of `a935cc9..5ac8573` (reliability): approved, lineage `review-df58601ea9c18b8a`. Two
  suggestions kept: the probe could still repeat a name if two checks in one process read the same clock value, and
  no test leaves an old probe behind; the command checks the report folder once per process, so a repeat within a
  run cannot happen, and the old name format is never produced again.
- **L21** (2026-11-04): Codex code review, round 2 (`docs/reviews/benchmark/2026-11-04-code-02.md`): the wrapper
  said `error` (usage limit at the final message) but the report is complete: R2 and R4 verified resolved; R1 still
  open, R3 partly, new R5. Fixed in `9cb6a49` (R1: four writer phases, exact count) and `6d35cd5` (R3, R5: shared
  retried removal). Checks: workspace 518 passed; bench with Voron 40 + 8 + 8 + 9. Plan of the optimizations in
  `docs/performance/plan.md` (`7ecd78a`). Round 3 waits for the Codex usage limit (11:49 UTC).
- **L22** (2026-11-04): RDD of `8972ddf..d05ba48` (reliability, resilience, readability): approved, lineage
  `review-59de3858f39c465c`. Fixed in `e1aaa18`: the impossible writer arm of the W12 sum is `unreachable!` (R2-002);
  the doc comment of `remove_retrying` says every error is retried, so a removal that cannot succeed fails after
  five seconds (R4 and R3-002; a held file shows on Windows as errors of several kinds, so the retry is not limited
  by kind). Kept: a folder that allows creating but not deleting files is now rejected, which R5 asks for; the
  helper stays in `cli` beside the validation that needs it; the Voron side of the W12 count has no unit test,
  because no test runs the Voron program outside a whole benchmark run (its `selftest` is not called by any test),
  and its rule mirrors the Rust one, which has the deterministic boundary test.
