# 15 - Query profile, estimate feedback and tuning

Technical design, document 15 of the [tables design set](README.md). Written 2026-11-21 from a discussion with the
user, who opened it with "Una cosa que he sufrido durante mucho tiempo en SQL Server son los planes de ejecución
pobres debido a una mala estimación de cardinalidad incluso con las estadísticas actualizadas. Simpre me he preguntado
si no sería posible "aprender" de los errores." It continued, verbatim:

- "Sí es algo para más adelante, pero quisiera dejar la puerta abierta. O sea que cuando llegue el momento se guarde la
  información relevante para identificar planes malos. Y también diseñar una estrategia sencilla pero eficaz."
- "Hay más cosas,  por ejemplo guardar esa información en una base de datos aparte de forma asíncrona y fire and forget
  para que afecte lo mínimo posible a la base de datos principal. Y el perfilado donde se guarden estadísticas de
  ejecución de consultas, qué consultas, cuanto tiempo tardan, diferencias entre estimaciones y reales, cuanta entrada
  / salida, cuantas veces se ejecutan. Eso"
- "Hay incluso más cosas, la base de datos principal y la de query store y perfilado se pueden llevar a otra máquina y
  todos los días correr un job que optimice las consultas que más merezca la pena optimizar, creando índices y
  directivas. En mi experiencia el 1 o 2% de las consultas se comen el 95% de los recursos."
- "Busca huecos y alternativas. Analízalo todo con pros y contras y recomienda una estrategia que también sirva cuando
  no tenemos una máquina independiente de análisis."
- To writing it down as document 15: "Sí".
- Later the same day, on language models: "¿Podría ayudar un LLM? ¿Quizá un modelo pequeño en local?", "Sería para
  aplicarlo en la máquina de análisis, que te ayude a optimizar y a explicar a la hora de decidir si aplicar las
  directivas e índices recomendados.", "¿Se podría usar la CLI de codex o de Claude en este contexto?" and, to adding
  it here, "Sí, agrégalo y haz el commit".
- Then, on individual executions: "Por cierto, lo de no guardar las ejecuciones individuales y los parámetros limitan
  bastante el análisis.", "Las formas caras precisamente ocupan muchísimo porque se ejecutan mucho.", "Yo estoy
  pensando en guardar sólo una muestra de las ejecuciones individuales.", "No creo que sea necesario sortear por
  transacción, más bien interesa por consulta. Apliquemos un poco de estadística matemática, ¿Cual es el tamaño T
  aceptable de una muestra para una población P?", "Poner un tamaño fijo de muestra no tiene mucha gracia, a ver qi lo
  suyo es fijar una probabilidad de guardado en función del número de ejecuciones hasta el momento. Con una
  probabilidad mínima y una máxima, empezamos con la máxima y se va reduciendo a medida que aumenta el número de
  ejecuciones", "aparte de las ejecuciones individuales hay que mantener los agregados. ¿Cómo hacemos esto?", "El
  histograma de donde sale?", "No termino de entender el peso que añades a cada ejecución" and, to adding it here,
  "Entendido, agrégalo todo al documento 15 y haz el commit".
- On the load at the start of a period: "Lo que me preocupa es el inicio del período donde la base de datos de perfil
  se somete a fuerte carga.", "¿Cual de las tres recomiendas?", "¿Cómo encajaría esto con rate limiters? en realidad
  estamos haciendo un rate limiter un poco esotérico" and, to adding it here, "Sí".
- On new shapes: "Y qué pasa con una forma nueva?" and, to adding the counter per (shape, plan) and the trajectory,
  "Sí, agrégalo y haz el commit. Yo lo que sigo pensando es en el tramo incial. ¿No sería conveniente limitar el
  número de guardados por segundo?" and, to adding the cap per entry and second, "Sí, agrégalo y haz el commit".
- On the assessment and the threads: "Cómo calificarías/evaluarías todo esto? es eficaz? es eficiente? es
  complejo?", "Y si usamos un patrón de múltiples productores y un único consumidor en vez de calcular las cosas en
  el hilo que hace la consulta.", "Dices que el hilo de la consulta calcule la probabilidad p, ya de paso lo prodría
  sortear y enviar a la cola sólo si tiene la suerte." and, to adding it all here, "Sí, agrégalo todo y haz el
  commit". Then "Bien ahora cual es tu evaluación?" and, to adding the snapshot read, the rings' life cycle and the
  threads, "Sí".
- On 2026-11-23, on the design itself: "¿Qué te parece el diseño del perfilado? ¿Qué mejorarías?" and, to forcing the
  last good plan, recording CPU time and stating the dependency of the tuner's stage, "Haz el 1, 3 y 4, no quiero
  perder exactitud", which also kept parameter values exact by default instead of masked (question 1).

Status: proposal. Nothing here is implemented, built or measured, and no PRD takes it yet. It comes after module 17
(queries), which it builds on; what module 17 must provide from its first version so that this work needs no
redesign later is listed in *Requirements for module 17*. The product questions at the end stay open until this work
is planned.

An illustrated companion, [15-query-profile-and-tuning.html](15-query-profile-and-tuning.html), shows with
interactive charts how the sampling of executions and the aggregates work (open it in a browser).

Evidence labels, as in documents 13 and 14: **[V]** verified in a primary source (fetched 2026-11-21, with the URL in
*Sources*); **[K]** Krummi's own documents; **[I]** inference or estimate; **[L]** from knowledge, not fetched.

## Summary

1. **Three layers, each useful alone.** A **profile** records what queries run, how often, what they cost and how far
   their estimates missed. **Estimate feedback** turns repeated misses into small, explainable **directives** that the
   planner applies. A **tuner** ranks the queries by total cost and recommends indexes, index removals, directives and
   plans for the few that matter.
2. **The profile lives in a separate Krummi store**, written in one deferred commit per interval, fire and forget.
   A query's thread only writes a small record into a ring of its own and, when a draw keeps the execution, its
   parameter values; one consumer thread keeps exact, mergeable aggregates of every execution and publishes the
   probability of keeping one, which starts high and falls as its shape runs more often; a kept execution carries the
   weight that undoes that probability. The profile never takes the main store's write turn, and a profile that fails
   to open, write or read turns profiling off without failing any query.
3. **Feedback corrects the source of an estimate, never the number.** No correction factors: a predicate whose
   estimate misses by more than 4× in the same direction, over several executions, gets a directive that estimates it
   from a sample of the table, which already holds the correlation between its columns. Directives are validated after
   they apply and expire when the statistics or the catalog change.
4. **One tuner, two places to run.** The tuner is a library function over a snapshot of the main store and the
   profile, with one output format. Without an analysis machine it runs inside the process, at low priority, with short
   reads, and evaluates candidate indexes as hypothetical indexes (*what-if*). With a machine it runs on online backups
   of both stores, builds the candidates for real and measures them. In both cases applying a recommendation is an
   explicit call, accepted items live in the main store's catalog, and the profile validates the change afterwards.
5. **Cost is measured in logical pages and CPU time, not wall-clock time.** Krummi maps its data file, so physical
   reads per query cannot be observed; pages touched can, and they compare across machines. Pages miss work that
   touches few of them, such as navigating a JSON document, decompressing or sorting in memory, so each execution also
   records its thread's CPU time, which ranks within one machine, as duration does.
6. **Indexes are never created automatically.** Building one holds the write turn for the whole build (IX2), so it is
   applied in a window chosen by the application. Directives, which are cheap and reversible, can apply on their own,
   and so can forcing the last good plan of a shape whose new plan regressed, suspected from the aggregates and
   confirmed by a trial that runs both plans at random (*Plan regressions*).
7. **A language model helps on the analysis machine, outside Krummi.** An agent command-line tool (Codex or Claude
   Code, with a cloud model or a local one) reads the profile and the recommendations through Krummi's tools, explains
   them, answers questions by asking the tuner to measure, and proposes candidates the rules do not generate. It
   proposes and explains; Krummi measures; a person decides. Every number in its report is checked against the
   evidence by a deterministic check (*Language models on the analysis machine*).
8. **Built in stages, each decided by a measurement.** First the requirements for module 17, the aggregates, the
   producers and one consumer, and forcing the last good plan; then a minimal sampling; then the tuner, measured on a
   copy once the online backup exists; estimate directives only if the profile shows they pay (*Assessment and
   stages*).

## Starting state

| Source | What it fixes | Effect here |
|---|---|---|
| Document 8, *Explain* (`08-execution-plans.md:257-259`) [K] | Per node: estimated rows, actual rows, batches, time, peak memory and bytes spilled, counted once per batch; "Estimates that miss by a wide factor are visible here, which is the only feedback the user asked of a planner on estimates." | The counters exist; the last sentence is superseded by this document (annotated there) |
| Document 4, *Plan caching per shape* (`04-query-tree-and-planner.md:189-194`) [K] | Shape hash as the cache key; revalidation re-estimates the leading access path and replans when rows move by more than a factor (say 4×); the cache is never persisted | A shape is the unit of the profile; 4× is reused as the feedback threshold |
| Document 11, P4, P12, P16 [K] | Plan statements never re-plan; catalog identities and state tokens are random 64-bit values kept across copies; prepared statements are the execution path | An accepted plan is a plan statement; shapes and plans are portable to a backup |
| PRD 7.3, ST1-ST7, ST17, ST18 [K] | Statistics per column from a snapshot, with the table's modification counter; an estimate samples synchronously when statistics are missing; one background thread per store; index prefixes have distinct counts and frequent combinations; equality on several columns uses those prefixes | Sampling and the modification counter are reused; correlation is already covered for columns that share an index |
| Root PRD, D6 (`krummi.prd.md:46`) [K] | One process per store, locked exclusively; the command-line tool works only on closed stores | An external analyzer cannot open a store the application holds |
| Root PRD, module 10 (`krummi.prd.md:269`) [K] | Full and incremental backup from the library with the store in use | The way to get a copy for the analysis machine |
| PRD 7.2, IX2 (`tables-indexes.prd.md:47`) [K] | Creating an index reads all rows, sorts and writes the index inside one write transaction | Holds the write turn (G4) for the whole build |
| Journal PRD, J5, J10 [K] | A long read transaction keeps the journal from being transferred, and it grows; a deferred commit returns without waiting for durability | In-process analysis uses short reads; the profile commits in deferred mode |
| Root PRD, G8, G9 [K] | With encryption no user data reaches the disk unencrypted; temporary stores live in memory | Query shapes and parameter values are user data; profiling in a temporary store stays in memory |

## What others do

| System | What it does | Lesson |
|---|---|---|
| IBM DB2 LEO (Stillger, Lohman, Markl, Kandil, VLDB 2001) [V] | Compares the estimates with the actual values at each plan step and computes adjustments to cost estimates and statistics for later optimizations; it never rewrites the catalog statistics: the adjustments live in a second layer applied on top, and ignoring them disables learning | The first learning optimizer; adjustments are numbers, which can drift and be inconsistent with each other [I] |
| Oracle [V] | Statistics feedback ("formerly Cardinality Feedback") is part of automatic reoptimization; SQL plan directives, created from what reoptimization learns, mark predicates the optimizer misestimates and drive dynamic statistics (sampling) and column-group statistics. Column groups are created automatically only with the `AUTO_STAT_EXTENSIONS` preference on (off by default), and directives are used only with `OPTIMIZER_ADAPTIVE_STATISTICS` true (false by default). The release dates (cardinality feedback in 11g Release 2, renamed in 12c) [L] | The directive model taken here: record which source to use, not which number; even Oracle leaves its heavier artifact off by default |
| SQL Server 2022 CE feedback [V] | Needs compatibility level 160 and Query Store in read-write mode; adjusts the model's assumptions (correlation, join containment, row goal) for a query, tries the adjusted plan on a later execution and persists it as a Query Store hint only when verified; a regression, or a query the user cancels, is not kept; skipped when a plan is forced | Change assumptions, verify, revert; never fight an accepted plan |
| SQL Server Query Store [V] | Aggregates runtime statistics per plan and time interval (`INTERVAL_LENGTH_MINUTES`, default 60) and writes to disk every `DATA_FLUSH_INTERVAL_SECONDS` (default 900); maximum size 1,000 MB by default since SQL Server 2019; when it is exhausted the mode changes to `READ_ONLY` until enough space is cleared; size-based cleanup (`AUTO` by default) starts at 90 % of the maximum, stops near 80 % and removes the least expensive and oldest queries first; stale queries after 30 days | The profile model; going read-only when full is the pain point to avoid (H12) |
| Azure SQL Database automatic tuning [V] | Create index, drop index and force last good plan; a recommendation the system applies itself is validated over 30 minutes to 72 hours and reverted on regression, but not one applied through T-SQL; defaults: force last good plan on, create and drop index off | Validation in production after applying; the same split as here, plans automatic and indexes manual |
| AutoAdmin what-if (Chaudhuri, Narasayya, SIGMOD 1998) [V]; HypoPG [V]; SQL Server Database Engine Tuning Advisor [V] | Hypothetical indexes the optimizer costs without building them (HypoPG's exist only in the session and only plain `EXPLAIN` sees them); the tuning advisor recommends indexes and the removal of unneeded structures. That the advisor merges candidates: not verified [L] | The in-process tuner's evaluation |
| PostgreSQL `pg_stat_statements` [V] | Aggregates per normalized query (constants replaced by `$1`…): calls, total, minimum, maximum and mean time, rows, shared blocks hit and read; keeps `pg_stat_statements.max` statements (default 5,000) and discards the least executed | A simpler profile without plans or intervals |
| Self-tuning histograms (Aboulnaga, Chaudhuri, SIGMOD 1999) [L]; STHoles (Bruno, Chaudhuri, Gravano, SIGMOD 2001) [V]; ISOMER (Srivastava, Haas, Markl, Kutsch, Tran, ICDE 2006) [V] | Histograms refined from query feedback; ISOMER uses the maximum-entropy principle to keep the histogram consistent with the observed cardinalities | Feedback on histograms needs care to stay consistent |
| AQO, Adaptive Query Optimization [V] | A PostgreSQL extension from Postgres Professional that uses the statistics of executed queries to improve cardinality estimates with machine-learning models | Learned estimators are powerful but opaque |
| q-error (Moerkotte, Neumann, Steidl, VLDB 2009) [V] | Symmetric multiplicative bounds f/q ≤ f̂ ≤ q·f, that is max(est/actual, actual/est); bounding the q-error bounds the plan's cost to at most q⁴ times the optimal | The measure used here |

## Requirements

- R1. A bad plan can be identified later: which queries, which plans, how often, what they cost, and how far their
  estimates missed, per operator.
- R2. Capturing it affects the main store as little as possible: no write turn, no wait, no failure of a query.
- R3. Repeated estimate misses are corrected by a simple, explainable mechanism that never guesses a number (the
  fundamental principle of 7.3: estimates need be neither exact nor current, and are never a default guess).
- R4. The queries that use most resources are found and improved with indexes and directives.
- R5. R4 works with an analysis machine and without one.

## Gaps found

Each gap names what it changes in the design.

| # | Gap | Consequence | Answer |
|---|---|---|---|
| H1 | One process per store (D6) | An external analyzer cannot open the main store or the profile while the application holds them | Online backups (module 10), or the tuner as a library call in the process |
| H2 | An index build holds the write turn (IX2) | Applying a recommended index blocks every writer for the whole build | Never automatic; the recommendation carries the build time, measured when there is a copy; an online, batched build is a later option |
| H3 | A long read transaction keeps the journal from being transferred (J5) | An in-process analysis over one long snapshot grows the journal without bound | Short read transactions and sampling only |
| H4 | The data file is mapped | Physical reads per query cannot be observed | Logical pages touched and bytes spilled; the counter does not exist yet (M3) |
| H5 | Duration depends on the machine | Times measured on the analysis machine do not compare with production | Rank and compare in pages; duration only within one machine |
| H6 | `Limit` and `TopK` stop their inputs early | The actual rows of those inputs are not their cardinality | Only completed nodes produce feedback (M1) |
| H7 | Replaying a query needs its parameter values | Without them the tuner can only estimate, which is what fails | Sampled executions keep their parameter values, encrypted with the store, with masking per column (question 1) |
| H8 | An index makes reads cheaper and writes dearer | A tuner that sees only queries recommends indexes that sink write loads | Writes per table and kind, and entries written per index, in the profile (M4) |
| H9 | Candidates for several shapes overlap | Five similar indexes, past the limit of 64 per table (IX3) and the write budget | Merge candidates that share a prefix; a budget per table |
| H10 | A daily window misses periodic loads | Month-end queries never rank | A rolling window of 7 to 30 days, weighted towards recent days (question 4) |
| H11 | Ranking by total cost misses rare slow queries | A screen with a 30 s p99 never ranks | Two lists: total cost, and p95 and maximum |
| H12 | A full profile | Query Store goes read-only when full [V] | Evict the cheapest shapes; never stop recording |
| H13 | Many threads update one map | Contention on the query path | Each thread writes only its own ring; one consumer keeps every shared structure, without locks or atomics on the query's path (*Capture*) |
| H14 | Direct table calls (read by key, scan an index) are not module 17 queries | They cost resources but have no plan to tune | Counted per table and index (M4); not tuned |
| H15 | What survives the loss of the profile | If accepted directives and fixed plans live in the profile, a damaged profile changes production performance | Learned directives live in the profile and are disposable; accepted items live in the main store's catalog (M7) |
| H16 | A profile copied next to another store | Feedback from one store applied to another | The profile records the main store's identity and is discarded when it differs |
| H17 | Unique indexes are constraints | Dropping an unused unique index changes what the application can write | Never recommend dropping a unique index |
| H18 | The feedback loop and the tuner act on the same shapes | A directive changes the plan, the profile sees another plan, the tuner adds an index, the directive no longer fits | Directives are bound to the catalog state tokens (P12) and the statistics they were learned on, and expire with them |
| H19 | Aggregates alone hide the individual executions | Sensitivity to parameter values (a 2 ms mean of 0.1 ms and 800 ms executions), patterns in time (N+1, bursts), which values an estimate misses, and what happened at a given moment are invisible | A sample of individual executions with their parameter values (*Sampled executions*) |
| H20 | Expensive shapes are usually expensive because they run often | Capturing every execution of exactly those shapes is the worst case for volume (about 2 MB/s at 10,000 executions per second of 200 bytes [I]) | A probability of keeping an execution that falls as its shape runs more often |
| H21 | That probability varies: a new shape starts high, a restart starts every shape again, a spike lowers the global factor | An unweighted sample is biased towards the executions kept with a higher probability, which may be a cold cache | Each kept execution stores its probability; analysis weights it by 1/p and calibrates the weights to the exact count |
| H22 | A counter that restarts every period restarts every shape at once | A synchronized burst of sampled rows at the start of each period, about 10⁶ rows (200 MB) with 1,000 shapes and `T = 1,000` [I], with drops exactly then | A decayed count that never restarts, and a global budget that lowers the probability before the draw |
| H23 | A new plan of a frequent shape is not a new shape | With a counter per shape, a plan regression is sampled with the shape's low steady probability, exactly when detail is needed | The counter is kept per (shape, plan); shapes that re-plan on every call keep one per shape |
| H24 | The global factor acts only on the total | One entry can still spend its first rows in a fraction of a second: a load on the profile at one moment, and a start portrayed by its first instant | A cap of `R` rows per second per (shape, plan), applied before the draw |
| H25 | One consumer processes every execution | Past its throughput the rings fill and drop, and that interval's aggregates stop being exact | Measure its cost per record and size the rings; a dropping interval is marked degraded, with its drops counted |
| H26 | Shared atomic counters on the query path | For the hottest shapes every core's addition bounces one cache line between cores | No shared writes in the query's thread: a ring per thread and one consumer |
| H27 | Reading the published snapshot through a reference-counted pointer | Every execution adds to a shared counter, the bouncing of H26 again | A copy of the snapshot per thread, refreshed when a version number that is only read changes |
| H28 | A ring per thread | Memory for threads that never query, rings of threads that ended, unbounded total with many threads | Rings created with a thread's first query, drained and freed when the thread ends, total memory bounded |
| H29 | Pages miss work that touches few pages | A shape that spends its time navigating JSON documents (a walk over the bytes before the value, document 13), decompressing or sorting in memory never ranks | The CPU time of the execution's thread (M3), aggregated like duration |
| H30 | A shape's new plan can cost more than the one it replaced | After revalidation replans a shape or the catalog changes, a frequent shape costs more until someone notices | The consumer suspects a new plan from the aggregates, confirms the regression with a trial that runs both plans at random, and forces the last good plan (*Plan regressions*) |

## The profile

### Where it lives

A Krummi store of its own, opened by the main store when profiling is on, in a folder of its own (question 3). It has
its own journal, lock and write turn, so writing the profile never queues behind or ahead of the application's
writers (G4). It reuses the engine: tables of module 7 for its contents, the background-work machinery of 7.3, the
encryption of module 9 with the main store's key (G8), and the backup of module 10. In a temporary store (G9) the
profile is a temporary store too.

The profile is disposable. If it fails to open, has another format version, belongs to another main store (H16) or
is damaged, it is discarded and created again; if that fails too, profiling is off for this opening and the store
information (P12) says so. Opening the main store never fails because of the profile.

### Capture

At the end of each execution (cursor finished, dropped or failed) the executor hands one record to the profile sink
(M6): shape id, plan id, outcome (rows, an error class, cancelled), duration on a monotonic clock, rows returned,
logical pages touched, peak memory, bytes spilled, replans, per node the estimated and actual rows with the
completed flag (M1), the wall-clock time, the transaction's identity and the execution's ordinal within it, and the
parameter values. With no sink, nothing is built.

**Producers and one consumer.** The work is split so that the query's thread does almost nothing:

1. **The query's thread, a producer,** builds a fixed-size record without the parameter values (about 64 to 128
   bytes [I]). It reads its entry's probability from a snapshot that the consumer publishes (*Sampled executions*),
   draws, and only when the draw keeps the execution appends the parameter values and the probability. It writes the
   record into a **ring of bytes of its own**, with one producer and one consumer, and returns. If its ring is full it
   drops the record and counts the drop; it never waits.
2. **The profile's consumer thread** drains every thread's ring. It alone keeps the aggregates, the shape registry,
   the decayed counters, the executions per second, the demand and `g`, in plain single-threaded structures without
   atomics or locks. Every tick (100 ms as a start [I]) it publishes a new read-only snapshot with each entry's
   probability, replaced as a whole by swapping one pointer [I].
3. **The writer thread** receives each finished interval from the consumer and writes it to the profile store
   (*Flush*), so that writing never stops the draining.

**Every execution is sent, not only the kept ones.** The aggregates, the calibration of the weights and the counters
that give the probability all need the exact count. Sending only the kept executions would turn the aggregates into
estimates with thin tails, leave no exact `N` to calibrate with, and feed the probability with counts estimated from
a sample drawn with that same probability. The parameter values, the costly part of variable size, travel only with
kept executions.

**Reading the snapshot without shared writes** (H27). If each execution cloned a reference-counted pointer to read
the published probability, it would add to a counter shared by every thread: the cache line bouncing of H26, moved
to the snapshot. So each thread keeps its own copy of the snapshot and, per execution, only reads the snapshot's
version number, a read and never a write; it reloads its copy, at most once per tick, only when the version changed.

**The rings' life cycle** (H28). A thread's ring is created lazily with its first query, so threads that never query
cost no memory; the consumer drains and frees the rings of threads that ended; the total memory, the ring size
times the threads with a ring, is bounded, and a thread past the bound gets no ring and its executions count as
drops.

**Threads.** The profile adds two threads per store, the consumer and the writer, beside the statistics thread of
7.3 (ST6). The consumer sleeps while the rings are empty, so profiling costs nothing at rest, and whether the writer
can share the background thread that already exists is to measure [I].

An entry that is not yet in the snapshot uses `p = 1` for at most one tick: at most its rate times the tick in rows,
once, about 1,000 for a new shape at 10,000 executions per second with a 100 ms tick [I].

A shape's canonical bytes are kept once in the consumer's registry, identified by hash with a full comparison after
it (document 11). A thread sends them with its first record of a shape (a small set per thread of the shapes it has
sent); later records carry only the identifiers.

### Aggregates

Every field of an aggregate must merge exactly, so that the accumulators of several threads, of several intervals
and of several days combine without loss. Averages and percentiles are never stored, because they do not merge (the
mean of means is wrong); they are derived when read.

| Field | Merges by |
|---|---|
| Executions; errors by class; cancelled | sum |
| Sums of duration, CPU time, rows, pages and bytes spilled | sum |
| Sums of squares of duration, CPU time and pages (for the variance) | sum |
| Minimum and maximum of duration, CPU time and pages | minimum, maximum |
| A log-linear histogram each of duration, CPU time and pages | sum per bucket |
| A log-linear histogram of the shape's executions per transaction | sum per bucket |
| Per node: completed executions; executions over 4×, under and over; maximum q-error | sum, maximum |

**The histogram comes from the executions themselves**, not from the table's data: each execution adds 1 to the
bucket its value falls in. The bucket limits are fixed in advance, the same in every thread, interval, day and
machine, so merging two histograms adds their counters bucket by bucket. The buckets are log-linear, as in
HdrHistogram [L]: for an integer value `x` (nanoseconds or pages) and `2^k` sub-buckets per power of two,

```text
octave = 63 - leading_zeros(x)                 // the power of two that holds x
sub    = (x >> (octave - k)) & (2^k - 1)       // the k bits after the highest one
bucket = octave * 2^k + sub
```

with values below `2^k` in buckets of their own, without error. A bucket is at most `1/2^k` of its value wide, so a
percentile read at a bucket's midpoint is off by at most half that: 6.25 % with `k = 3`, 3.1 % with `k = 4`, 1.6 % with
`k = 5`. Recommended: `k = 4`, bit arithmetic only. Purely geometric buckets (DDSketch [L]) reach a smaller error for
the same count but need a logarithm per value. Durations from 100 ns to 1 h span about 35 powers of two, about 560
possible buckets with `k = 4`; only the non-empty ones are stored, and a real shape fills a few powers of two [I]. A
percentile `q` accumulates the counters in order until it reaches `q·N` and returns that bucket's midpoint.

These histograms are not 7.3's statistics, which describe the values of a column and are built from a sample of the
table; those serve here only to place a masked parameter value (question 1).

The executions per transaction come from the transaction's identity in every record: the consumer counts
executions per transaction and shape, and closes a transaction's counts when its producer sends a small record at
the end of a transaction that ran queries. A shape that runs 500 times in one transaction with different keys is an
N+1, visible without keeping any execution [I]. It belongs to a later stage (*Assessment and stages*).

The aggregates are kept by the consumer alone, in plain structures keyed by (shape, plan): no locks and no atomics,
and exact as long as no ring drops a record. A drop makes that interval's aggregates inexact; the interval is marked
**degraded**, with its dropped records counted (H25).

### Sampled executions

**Why a falling probability.** Expensive shapes are usually expensive because they run often (H20), so a fixed rate
spends most of its budget on them, and a fixed-size sample per shape (a reservoir, Vitter's Algorithm R [L]) replaces
rows already kept, which needs the sample held in memory until the period ends or rewritten in the profile, against
fire and forget. Instead, each execution is kept or not when it ends, and a kept one is written once and forgotten:
append only. The probability depends on `n`, the entry's **decayed count** of recent executions, on `c`, its
executions per second (*A cap per entry and second*), and on a global factor `g` (below):

$$p(n) = \min\Big(p_{max},\ \max\big(p_{min},\ \tfrac{T}{n}\big)\Big) \qquad p_{draw} = g \cdot \min\Big(\frac{R}{c},\ p(n)\Big)$$

The consumer computes `p_draw` for each entry and publishes it at every tick; the query's thread only reads it and
draws (*Capture*). The first version fixes `p_max = 1` and leaves `p_min` out (*Assessment and stages*); the text
below shows where they act.

**The count decays and never restarts.** At each flush the consumer multiplies every counter by
`e^(−Δt/τ)`, with `τ` = 1 day as a start [I], one operation per shape and interval, so `n` approximates the shape's
executions over the last `τ`. A new shape starts at `n = 0`, so with `p_max = 1` its first `T` executions are all kept
(rare shapes are complete) and its probability falls as it runs more often. A steady shape reaches `n ≈ λτ` (`λ` its
rate) and a nearly constant `p`, so it keeps about `max(T, p_min·N)` rows a day (`N` its executions a day), spread
evenly through the day. With `T = 2,000`, `p_min = 10⁻⁴` and 200 bytes per row [I]:

| Executions per day | `p` | Rows kept per day | Size |
|---|---|---|---|
| 10⁴ | 0.2 | 2,000 | 0.4 MB |
| 10⁶ | 0.002 | 2,000 | 0.4 MB |
| 10⁷ | 0.0002 | 2,000 | 0.4 MB |
| 10⁸ | 10⁻⁴ (`p_min`) | 10,000 | 2 MB |
| 10⁹ | 10⁻⁴ (`p_min`) | 100,000 | 20 MB |

In its first `τ` a new shape keeps more, about `T(1 + ln(N/T))` rows (the harmonic series of `T/n`), and new shapes
appear at different moments, so this costs no burst.

**A new shape's trajectory.** At a constant rate `λ` the decayed count follows `n(t) = λτ(1 − e^(−t/τ))`, so
`p(t) = T / (λτ(1 − e^(−t/τ)))`. While `t ≪ τ`, `n ≈ λt` and `p ≈ T/(λt)`: the shape starts as a counter since its
first execution would, keeping everything and then falling as `T/n`. When `t ≫ τ`, `n → λτ` and `p → T/(λτ)`,
constant. Example with `λ` = 100 executions per second (8.64 million a day), `T = 2,000` and `τ` = 1 day [I]: its first
2,000 executions, in 20 seconds, are all kept; after a minute `p ≈ 0.33`; after an hour `p ≈ 0.0057`; after a few
days `p ≈ 2.3 × 10⁻⁴`. It keeps about `T(1 + ln(λτ/T))` ≈ 18,700 rows its first day and about 2,000 a day afterwards.
The extra detail at the start is wanted: a new shape is usually a new deployment. The weights undo the bias, and `g`
brakes when hundreds of new shapes arrive together.

A shape that comes back after a long absence has a counter near zero and is treated as new: a monthly report gets
full detail on the first day of the month with nothing special. Entries whose counter is near zero can be removed
from the registry to bound its memory.

**The key is (shape, plan)** (H23). A counter per shape would sample a new plan of a frequent shape, a plan
regression, with the low steady probability of the shape, exactly when detail is needed. The counter is therefore
kept per (shape, plan), the key the aggregates already use, so a new plan starts at `p_max` and gets detail at once,
for the same cost: one counter per entry. Shapes marked "plan per call" by document 4, which re-plan often, keep one
counter per shape, so that each re-planning does not count as a new plan.

**Why not a counter that restarts every period** (H22). Restarting every shape's counter at the same moment puts
every shape back at `p = 1` together: with 1,000 active shapes and `T = 1,000`, about 10⁶ rows (200 MB) in the first
seconds or minutes of each period [I], with the buffers filling and dropping exactly then. It is a fixed-window rate
limiter, biased towards the start of each window; the decayed count is its sliding-window counterpart.

**A cap per entry and second** (H24). `g` is global and acts only when the total demand passes the budget `B`, so it
does not stop one entry from spending its first rows in a few seconds: at 100 executions per second the first
2,000 rows take 20 s, at 10,000 per second 0.2 s, a load on the profile at one moment and a start portrayed by its
first instant instead of its first minutes. A cap of `R` rows per second per (shape, plan) is one more factor before
the draw: `c` is the larger of the entry's executions in the previous second and in the current second so far. The
larger of the two covers an entry's first second, which has no previous one: for an entry that runs more than `R`
times per second, `p` falls within it as `R/c`, so it keeps at most about `R(1 + ln(λ/R))` rows, about 79 with
`λ` = 10,000 per second and `R = 10` [I]; an entry that runs `R` times per second or fewer has `R/c ≥ 1` and the cap
never acts on it. It is unbiased, because it
brakes before the draw and the resulting probability is stored. It bounds the expected rate, not the count: the
rows kept in a second are random, and an entry not yet in the snapshot keeps up to one tick of rows with `p = 1`
before the cap acts (*Capture*). It bites only at starts and spikes: with `R = 10`
an entry may keep up to 864,000 rows a day, far above `T`. With `λ` = 100 per second and `T = 2,000`:

| | Without the cap | With `R = 10` |
|---|---|---|
| The first 2,000 rows | in 20 s | in about 200 s |
| Rows in the first 200 s | about 6,600 | about 2,000 |

The consumer counts each entry's executions per second as it drains them and folds the cap into the probability it
publishes at each tick; nothing is counted in the query's thread.

**`p_min` gives the most frequent shapes more rows than `T`**, which the analysis of subgroups needs (*Sizing `T`*);
its cost is linear in `N`, so it is set from a target of rows for those shapes or from a byte budget [I].

**The global factor `g` brakes before the draw.** A budget of `B` sampled rows per second, a token bucket: every
second the consumer computes the expected demand `D`, the sum of the probabilities of the last second's executions,
and sets `g = min(1, B/D)`, smoothed. `g` is part of the probability the consumer publishes, so the draw uses
`g·min(R/c, p(n))`, and that is the probability stored with the row. Since `D` is the last second's demand,
smoothed, `g` brakes a spike one second late, and within that second the expected demand may pass `B`. It brakes any
spike:
a deployment with hundreds of new shapes, an unusual load, and the restart of the process, when every counter starts
again at 0. Seeding the counters at opening from the profile's last day would also spread the budget by frequency
after a restart instead of evenly during the first day; it is a refinement left out until a measurement asks for it.

**The statistical rule: brake before the draw, never after.** A deterministic token bucket that rejects rows already
drawn depends on the order of arrival, so the real probability of a kept row is unknown and its weight is wrong. A
factor applied before the draw is known and stored, so the weight stays exact. The only deterministic limit left is
the memory bound of the buffers, which protects rather than samples: if it ever drops drawn rows, the interval is
marked **degraded**, its dropped rows are counted, and the analysis knows that its weights are not reliable. With `g`
working it should never happen, so it also serves as an alarm.

**The same design in distributed tracing** [L]: AWS X-Ray's sampling rules keep a reservoir of requests per second
and a fixed rate beyond it (`p_max`, `T` and `p_min` here); Jaeger's adaptive sampling adjusts each operation's
probability towards a target of traces per second (the decayed count); OpenTelemetry's probability sampling records
an adjusted count, `1/p`, with each sampled trace so that metrics can be derived from the sample without bias (the
weight). Two pieces suffice here, the probability per shape and the factor `g`; no generic rate-limiting framework.

**The weight.** A kept execution represents `1/p_draw` executions: with `T = 1,000`, a new shape's execution 500
(`p = 1`) represents itself, execution 10,000 (`p = 0.1`) represents 10, and execution 1,000,000 (`p = 0.001`)
represents 1,000. Wherever the probability varies (a new shape, a restart, a change of `g`), an unweighted sample is
biased towards the executions kept with a higher probability (H21). Example: a new shape whose first 1,000
executions take 50 ms (a cold cache) and whose next 999,000 take 1 ms. The sample keeps the 1,000 slow ones and
about 6,900 fast ones (the sum of `1000/n` from 1,001 to 10⁶). The true mean is (1,000·50 + 999,000·1) / 1,000,000 ≈
1.05 ms; the unweighted mean of the sample is (1,000·50 + 6,900·1) / 7,900 ≈ 7.2 ms; weighted, the slow rows add
1,000 of weight and the fast ones about 999,000, so the mean is again about 1.05 ms. Unweighted, the p95 would be
50 ms, since 12.7 % of the rows are slow; the true p95 is 1 ms. When the probability is the same for every row,
weighted and unweighted agree; the weight matters exactly when it changes.

Only `p_draw` is stored, one number per row; the weight is computed when read. Totals use the Horvitz-Thompson
estimator [L], means and ratios the Hájek estimator [L], percentiles a weighted cumulative sum. Since the aggregates
know the exact `N` of each (shape, interval), the weights are **calibrated** so that they add up to `N` exactly, a
ratio estimator that lowers the variance at no cost. Totals always come from the aggregates; the sample gives detail
and relations.

**Effective size.** With unequal weights the effective size is Kish's `n_eff = (Σw)² / Σw²` [L]. A steady shape has
nearly equal weights, so `n_eff` is close to the rows kept. In a new shape's first `τ`, with `p = T/n`,
`E[Σw²] = Σ 1/p_i ≈ N²/(2T)` and `E[Σw] = N`, so `n_eff ≈ 2T` for its `T(1 + ln(N/T))` rows; `p_min` raises it,
because it stops the weights from growing.

**Sizing `T`.** For a proportion `p` with margin `e` at confidence `z`, Cochran's formula [L] gives
`T₀ = z²·p(1-p)/e²` and, with the finite population correction, `T = T₀ / (1 + (T₀-1)/P)`; the size hardly depends on
`P` once `P` is much larger than `T₀`. At 95 % (`z = 1.96`) and `p = 0.5`, the worst case: ±5 % needs 385, ±2 % 2,401
and ±1 % 9,604. A percentile `q`, whatever the distribution, has a rank whose variance is `q(1-q)/T`, so the p95
within ±1 percentile point needs 1,825 and the p99 within ±0.2 points 9,508. A mean with relative error `ε` needs
`(z·CV/ε)²`, which depends on the spread (`CV = σ/μ`; 1,537 for `CV = 1` and ±5 %, 13,830 for `CV = 3`), so means come
from the exact aggregates. These sizes apply to the effective size and **to each subgroup analyzed**: splitting a
shape's sample into 20 buckets of a parameter leaves each bucket with about a twentieth of it. With nearly equal
weights, `T = 2,000` a day gives about ±2.2 % on proportions and the p95 within ±1 point per shape and day.

**Outliers** are always kept, apart, with `p = 1` and a cap of their own: errors, executions with a q-error over 4×,
and executions beyond the p95 of their shape in the previous interval. Past the cap they are sampled like the rest,
with their probability stored.

**A sampled row** holds the record of the sink (above), its `p_draw`, whether it was kept as an outlier, and the
parameter values, masked per column when question 1 asks for it.

### Flush

Every interval (15 minutes as a start [I]; Query Store aggregates per 60 minutes and writes every 15 by default [V])
the consumer closes the interval's aggregates and the executions sampled since the last flush and hands them to the
writer thread, which writes them in **one** write transaction of the profile store committed in deferred mode
(J10). A crash loses at most the interval in flight, which is acceptable for
statistics that need be neither exact nor current; because both go in the same transaction, they are lost together,
and a sample never exists without its aggregates. If the write fails or the profile is slow, closed intervals
wait in memory up to a bound; past it the cheapest shapes are dropped and counted. A query never waits for
any of this.

Because the aggregates merge exactly, retention loses resolution, never precision: 15-minute intervals for 7 days,
then merged into hours and later into days, for 30 to 90 days (question 4). The sampled executions follow a retention
of their own.

### Contents

Krummi tables in the profile store, so the tuner and the application can query them with module 17:

| Table | Key | Contents |
|---|---|---|
| `meta` | — | Format version, the main store's identity (H16), dropped counts, first and last interval |
| `shapes` | shape id | Canonical shape bytes, table, first and last seen |
| `plans` | plan id | Shape, plan bytes, origin (planner, directive, accepted), first and last seen |
| `runtime` | plan, interval start | The aggregates of *Aggregates*; peak memory; replans |
| `node_estimates` | plan, node, interval | Completed executions; maximum q-error; executions over 4×, under and over |
| `predicate_feedback` | predicate signature, interval | Observations; under and over counts; maximum q-error; estimate source (statistics, sample, stale statistics); the statistics' modification counter |
| `table_writes` | table, interval | Inserts, updates, deletes and bulk rows; per index, entries written, seeks, scans and rows read (M4) |
| `executions` | shape, period, wall-clock time, sequence | The sampled executions of *Sampled executions*, each with its `p` and outlier mark |
| `directives` | predicate signature or shape | Learned directives, of estimates and of plans, with their state (*Estimate feedback*, *Plan regressions*) |

Interval starts use the wall clock in UTC; durations use the monotonic clock. Retention follows question 4: old
intervals are removed by the flush, and when the profile passes its size bound the cheapest shapes go first (H12).

## Plan regressions

The plan cache replans a shape when revalidation sees its rows move by more than a factor or when the catalog changes
(document 4), and the new plan can cost more than the one it replaced (H30). The aggregates per (shape, plan) already
measure each plan's cost, so catching it needs neither estimates nor a copy. It is the counterpart of Azure's *force
last good plan*, on by default there [V], and the first automatic action to build.

**Suspicion.** Each shape has a **last good plan**: the plan it ran before, while that plan has enough executions in
the window (30 as a start [I]). When the shape runs another plan, that plan is a **candidate**, and it stays pending
from one flush to the next until it has enough executions too, until the shape stops running it, or until the
catalog no longer supports the last good plan. With enough executions, the candidate is **suspect** when its mean or
its p95 of pages per execution exceeds the last good plan's over its window by a factor (2× as a start [I]). The two
windows may hold different parameter values, a cheap mix for one plan and a dear one for the other, so a suspicion
is only the reason to run a trial, never the evidence to force a plan.

**Trial.** A suspect shape runs both plans side by side. The consumer publishes a **trial directive** that gives each
of the shape's executions the last good plan with probability ½ and the candidate otherwise, and the query's thread
draws, as it already draws for sampling (*Capture*). Assigned at random, the two plans run the same mix of parameter
values at the same moments, so their means in the trial compare without bias. The trial ends when each plan has
enough executions (30 as a start [I]), within a bound on time. The candidate **regressed** when its mean pages per
execution in the trial exceed the last good plan's by the factor; otherwise it is **cleared** and becomes the shape's
last good plan. A trial runs at most about half the shape's executions on the worse plan, for as long as it lasts.

**Forcing.** A regressed candidate gets a **plan directive** in the profile's `directives` table: the shape, the last
good plan's bytes from `plans` and the catalog state tokens it was planned on. The planner receives directives as an
input (M2), so the shape's next planning takes that plan instead of choosing one. Like an estimate directive it is
learned, disposable and kept in the profile (H15), and `explain` names it.

**Directives and the plan cache.** The consumer publishes the directives with a version per shape, read like the
snapshot of H27, and each cached plan records the version it was planned with. Installing, changing, dropping or
expiring a directive increments its shape's version, so the shape's next lookup plans again instead of reusing its
cache entry, even when revalidation would keep it; a cursor already running keeps its plan. Estimate directives
reach the cache the same way.

**Expiry.** The plan directive expires when the catalog state tokens it was planned on change (for example, an index
it uses was dropped, P12), after a time to live, or when a plan of the shape is accepted into the catalog (M7). Then
the planner chooses again; a plan it chooses is a candidate again, and a new trial decides, since the data may have
changed. A shape whose plan is forced again after an expiry is a tuner recommendation to accept a plan, which a
person decides.

**What it never does.** It never overrides an accepted plan (SQL Server's lesson in *What others do*), never forces a
plan the catalog no longer supports, and never forces a plan from separate windows without a trial.

## Estimate feedback

### What is recorded

A **predicate signature** names a filter without its values: the table, the columns by identity, the operators and
the `AND`/`OR`/`NOT` structure, normalized (children of `AND` and `OR` sorted). Keying feedback by signature, not by
query, lets every shape with the same filter share what was learned.

A node produces an observation only when it completed (H6), when estimated or actual rows reach a minimum (100 as a
start, [I]; smaller misses cannot change a plan much) and when its q-error passes 4×, the revalidation factor of
document 4. The sign is kept: an underestimate is worse than an overestimate, as 7.3 decided ("overestimate").

### Directives

When a signature gathers three observations in the same direction ([I]) within the window, the feedback looks at the
cause:

| Cause | Action |
|---|---|
| The statistics were stale (ST5) | Nothing new: the rebuild that 7.3 already requests answers it |
| One column, fresh statistics: the histogram is too coarse for the values used | Directive: estimate this signature from a sample |
| Several columns under `AND` with no index prefix covering them (ST17, ST18): independence failed | Directive: estimate the whole conjunction from a sample, which holds the correlation |

The directive's sample is taken with 7.3's sampling when the plan is built, projected to the signature's columns and
kept with the cached plan, so revalidation with new parameter values evaluates the predicate on that sample instead
of sampling again. Its size is bounded ([I]: a few thousand rows of a few columns).

A directive is **on trial** for its next observations: it stays if its q-error falls below the threshold, and is
dropped otherwise, with the signature marked so that no directive is tried again for a while. It **expires** when
the statistics of its columns are rebuilt, when the catalog state tokens it was learned on change (P12, H18), or
after a time to live. Directives are loaded into memory when the store opens and the planner receives them as an
input (M2); the planner never reads the profile while planning. `explain` names the directive that produced an
estimate.

Column-group statistics without an index (Oracle's other directive) are a heavier artifact: they are a tuner
recommendation, never automatic.

## The tuner

### What it does

`tune(main snapshot, profile, options) -> recommendations`, a library function. In order:

1. **Window**: the last 7 to 30 days of intervals, weighted towards recent ones (H10).
2. **Ranking**: total cost per shape, in logical pages over the window (executions × pages per execution, H5). The
   shapes that add up to a share of the total (95 % as a start) are selected, up to a maximum count; the user's
   experience is that "el 1 o 2% de las consultas se comen el 95% de los recursos". A second list takes the shapes
   with the worst p95 or maximum (H11), and a third the shapes with the most total CPU time, on the machine
   where the profile was recorded (H29).
3. **Candidates** per selected shape: an index with the equality columns first, then one range column, then the
   order columns; the projected columns as included columns when the index can cover; directives and column-group
   statistics for predicates with repeated misses; an accepted plan when one plan of the shape is clearly better than
   the others in the profile.
4. **Merge** candidates that share a prefix, widening included columns, within a budget per table (index count under
   IX3's 64, size, and write overhead from `table_writes`, H8, H9).
5. **Removals**: secondary indexes without seeks or scans over the whole window, never unique ones (H17).
6. **Evaluation**, by one of two modes (next section).
7. **Output**: a recommendation document in the typed JSON of document 11, each item with its kind (create index,
   drop index, directive, column-group statistics, accept plan), its definition, the shapes it affects, the pages
   before and after, the evaluation mode, the build cost and size, and the write overhead.

### Evaluation modes

| Mode | How | Needs | Trust |
|---|---|---|---|
| **What-if** | The planner plans each selected shape against a hypothetical index set (M2); estimates come from statistics and, for the hypothetical index's prefixes, from sampling | Nothing beyond the main store | Estimated: the thing that fails, mitigated by sampling; build time and write overhead estimated |
| **Measured** | Build the candidates on a copy, replay the sampled executions before and after, measure pages and time, measure the build time, and apply a batch of writes shaped like `table_writes` to measure write overhead | A copy, resources and parameter values (H7) | Measured |

### Applying and validating

Applying a recommendation is an explicit call of the application. An index is created as IX2 creates it, in a write
transaction, in the window the application chooses (H2). An accepted directive or plan is recorded in the main
store's catalog (M7), so it survives the loss of the profile and travels with backups (H15).

After applying, the profile compares each affected shape's pages per execution in the windows before and after the
acceptance, and the next tuner run recommends reverting what made it worse. This validation in production is the
one that counts, whatever the evaluation mode was.

## Running it

| Setting | Where the tuner runs | Mode | Constraints |
|---|---|---|---|
| No other machine | In the application's process, at low priority, when the application calls it (question 6) | What-if | Short read transactions (H3); a CPU budget; never builds an index |
| No other machine, a spare disk | A restored backup in a scratch folder on the same machine, through the command-line tool (module 12), at quiet hours | Measured | Competes for CPU and disk with production, but never takes the main store's write turn |
| An analysis machine | Online backups of the main store and the profile (module 10), restored there; the command-line tool runs the tuner | Measured | Parameter values in the profile (question 1); the recommendations file travels back |

All three produce the same recommendation document, so applying and validating are the same everywhere.

## Language models on the analysis machine

### Where a model helps and where it does not

A language model takes milliseconds to seconds per answer, is not deterministic and cannot justify a number it gives
[I]. So it never estimates rows for the planner, which must cost microseconds, and it never decides an index on its
own, because its output is a hypothesis and what fails is what is not measured. It helps offline, on the analysis
machine, where a copy exists, everything can be measured and there is time:

1. **Explaining.** It turns each recommendation, and each plan regression in the profile, into a decision report:
   what is proposed and which shapes it improves, with pages before and after; what it costs (the measured build
   time, which is the window IX2 needs, the size and the write overhead); what could go wrong and how to revert it.
2. **Answering by measuring.** To a question such as "why not an index on (a, b)?" it does not give an opinion: it
   asks the tuner to measure that candidate on the copy (T2) and answers with the result.
3. **Proposing what the rules miss.** Patterns of the application visible in the profile: an N+1 (one shape run
   many times in one transaction with different primary keys, seen in the executions-per-transaction histogram and
   in sampled executions that share a transaction, answered by one `IN`), an expression index over JSON (document 14), a change of
   schema. Each proposal enters the tuner as one more candidate (T1) and is measured like the others, so a weak
   proposer only costs evaluation time.
4. **Summarizing** the daily report and grouping shapes by feature of the application.

### The safeguard

A model can state a wrong number with full confidence, which is poison for a decision. Each number in a decision
report cites the field of the recommendation document or of the measurement it comes from, and a deterministic check
(not another model) compares every cited number with that evidence; a report that does not match is rejected. The
model contributes the explanation; the numbers always come from Krummi.

### What Krummi provides

Krummi carries no model: an embedded engine loading gigabytes of weights goes against what it is. It exposes tools,
through the command-line tool of module 12 and the capability layer and MCP server of document 10:

- T1. The tuner accepts **external candidates** (indexes, directives, column-group statistics, plans) as input.
- T2. **Evaluating one candidate on demand** on the copy, in measured mode, with the same evidence as a recommendation.
- T3. A recommendation document whose evidence a machine can check: stable field identifiers and exact numbers.
- T4. A **skill** that guides an agent through the workflow and the template of the decision report (document 10
  already plans one).

### Agent command-line tools

Both agents installed here have what the workflow needs (read from their `--help`, 2026-11-21) [V]:

| Need | Codex CLI 0.170.0 | Claude Code 2.1.328 |
|---|---|---|
| Non-interactive run, for the daily job | `codex exec` | `claude -p` (`--print`) |
| MCP servers, for document 10's tools | `codex mcp` | `--mcp-config` |
| Restricted tools | `codex sandbox` | `--allowedTools`, `--disallowedTools` |
| A local model | `--oss` with `--local-provider lmstudio` or `ollama` | — |
| The workflow as instructions | `AGENTS.md` [L] | Skills, `CLAUDE.md` [K] |

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| A cloud model (Codex or Claude) | The best reasoning about plans; mature tool use; nothing to host | Shapes and parameter values leave the machine; a cost per run; needs a network | Allowed without parameter values (question 9) |
| A local model (Codex with `--oss`) | Data stays on the machine; no cost per run; the same command-line tool | Weaker reasoning; the model must be hosted | Chosen when parameter values are captured |
| A model embedded in Krummi | — | Gigabytes of weights in an embedded engine; ties Krummi to one model | Rejected |

Choosing or changing the agent, or moving from a cloud model to a local one, does not touch Krummi. A small local
model is enough to explain and to call the tools; a larger one proposes better, and since everything is measured,
the difference is only how many proposals turn out useful [I].

### Two rules

1. **The agent sees only the copy.** Its tools read the profile and evaluate on the copy; restricted tools or the
   sandbox keep it away from production.
2. **Applying stays a person's decision.** The agent leaves a checked decision report; a person decides what reaches
   production and in which window.

A natural flow: the daily job runs `codex exec` or `claude -p` with the skill and leaves the report; the discussion
("why not (a, b)?") happens in an interactive session on the same copy.

## Assessment and stages

Asked "Cómo calificarías/evaluarías todo esto? es eficaz? es eficiente? es complejo?", the assessment of 2026-11-21:

| Layer | Effectiveness | Efficiency | Complexity | Note |
|---|---|---|---|---|
| Requirements for module 17 (M1-M7) | High | High | Low | The most valuable part: cheap now, expensive later |
| Mergeable aggregates | High | High | Low | The model of Query Store and `pg_stat_statements` |
| Forcing the last good plan (added 2026-11-23) | High | High | Low | Suspected from the aggregates; confirmed by a randomized trial; reversible |
| Sampled executions | High | High with producers and one consumer | High | Statistically sound, but five parameters, a controller, calibration, outliers and the degraded mark add up |
| Estimate directives | Uncertain | High | Medium | See the first finding |
| What-if tuner | Medium | High | Medium | Rests on estimates, which are what fails |
| Tuner measured on a copy | High | Medium | Medium | The most reliable; needs a machine and parameter values |
| Language model agent | Medium to high | — | Low for Krummi | The deterministic check keeps the risk low |

Findings:

1. **Directives answer SQL Server's pain, which is not the pain of module 17's first version.** In SQL Server bad
   plans come mostly from joins: a misestimate picks the join order and algorithm, and the error multiplies at each
   level [I]. Module 17's first version runs one table per query, without joins (document 4), so a misestimate only
   chooses between an index, a full scan and an intersection, and revalidation per execution already exists. The
   profile itself measures whether directives are worth building: the q-error per node, and how often a miss changes
   the plan.
2. **Shared atomic counters on the query's path** (H26). The earlier design kept each entry's counter as an atomic
   value shared by all threads; for the hottest shapes every core's addition bounces one cache line between cores,
   at the busiest place. The producers and one consumer (*Capture*) remove every shared write from the query's
   thread.
3. **Too many sampling parameters to start with.** `p_max` is always 1; `p_min` only gives the hottest shapes more
   rows than `T`, which the decayed count already gives every shape each day; the outliers with a cap of their own
   and the executions per transaction can wait. The minimal core is `p = g · min(R/c, T/n)`: `T`, `τ`, `R` and the
   budget `B`.

Stages, each one decided by a measurement:

1. **With module 17:** M1-M7, the aggregates, the producers and one consumer, and forcing the last good plan
   (*Plan regressions*). Almost free, and it opens the door, which was the first aim; forcing the last good plan is
   its first automatic action.
2. **Minimal sampling:** `p = g · min(R/c, T/n)`. `p_min`, the outliers, the executions per transaction and the
   seeding of counters at opening come only when a measurement asks for them.
3. **The tuner:** measured on a copy, the reliable mode, and what-if; the language model agent with it. A copy of a
   store in use needs the online backup of module 10, and the command-line runs need module 12; both come after
   modules 17 to 19 in the root PRD. Until they exist the tuner starts with what-if in the process, and measures only
   on a copy of the store taken while it is closed (D6).
4. **Directives:** only if the profile shows that misestimates change plans often.

In short: effective; efficient once the shared atomics are gone; more complex than this stage needs as a whole,
so it is built in stages.

Asked again after the producers and one consumer ("Bien ahora cual es tu evaluación?"), the reassessment of the
same day:

| Aspect | Before | Now | Why |
|---|---|---|---|
| Effectiveness | High | High | Unchanged: exact aggregates and an unbiased sample |
| Efficiency on the query's path | Medium (shared atomics) | High on paper | No shared write: a fixed record, its own ring, a draw, a snapshot read through a version number (H27) |
| Complexity of what is built first | High | Medium | Stages 1 and 2: aggregates, rings, a single-threaded consumer and `p = g · min(R/c, T/n)` |
| Complexity of the whole system | High | High, deferred | The tuner, directives and the agent wait for a measurement that asks for them |
| Testability | Low | High | The consumer is a function of a stream of records and a clock |

Remaining risks, in order: the consumer's throughput, unmeasured and the design's bet (H25); the value of directives
in the first version, now conditioned on what the profile measures; and a design decided before module 17 exists,
mitigated by the stages and by measurement deciding each one. Verdict: effective; efficient on paper, pending two
measurements, the cost in the query's thread and the consumer's throughput (experiment 6); a complexity that fits
what is built first.

Reviewed on 2026-11-23 ("¿Qué te parece el diseño del perfilado? ¿Qué mejorarías?"), with the user's decision "Haz el
1, 3 y 4, no quiero perder exactitud":

- forcing the last good plan joins stage 1 (*Plan regressions*), because it is measured from the aggregates alone,
  while estimate directives wait for a measurement (finding 1);
- each execution records its thread's CPU time (H29, M3);
- stage 3 states its dependency on modules 10 and 12;
- masking parameter values by default was proposed and declined, because it loses the exact values (question 1).

## Alternatives considered

**Where the profile lives**

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Internal trees of the main store | One file; in every backup | Takes the write turn (G4); grows the main store and its journal | Rejected (R2) |
| **A separate Krummi store** | Isolated; reuses the engine, encryption and backup | A second folder and lock | **Chosen** |
| Rotated log files | Cheapest writes; easy to ship | A new format with no integrity check; querying means loading | Rejected (DRY, G6) |
| A callback or OpenTelemetry export | Fits existing monitoring | The tuner cannot rely on it | Possible later as an extra sink |

**Where the work happens**

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| In the query's thread, with shared atomic counters | Simple to write | The hottest shapes bounce one cache line between cores (H26) | Rejected |
| Accumulators per thread, merged at each flush | No shared writes per execution | Per-thread maps, a mutex, a merge, and logic spread over threads that is hard to test | Rejected |
| One queue with many producers | One structure | Every producer contends on the same queue | Rejected |
| **A ring per producer thread and one consumer** | Nothing shared written by the query; single-threaded logic, testable as a function of a stream of records and a clock | Aggregates inexact under extreme overload, marked degraded (H25); the consumer's throughput to measure | **Chosen** |
| Sending only the kept executions | No traffic for most executions | Aggregates estimated, no exact `N`, probability fed by its own estimates | Rejected |

**Granularity of capture**

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Every execution | Full detail; replay | Volume, worst on the frequent shapes that matter most (H20) | Rejected |
| Aggregates only | Bounded and cheap | Hide parameter sensitivity, patterns in time and the values that miss (H19) | Part of the choice |
| Aggregates plus the worst executions | Cheap; keeps the tail | A replay measured on the tail overestimates the gain for the typical execution | Rejected |
| Full capture of chosen shapes for a window | Every detail where it matters | The chosen shapes are the frequent ones, so it is the most expensive capture (H20) | Only as short windows of minutes, on demand, if ever needed |
| Fixed-size sample per shape (reservoir) | Exact size; uniform | Replaces kept rows: holds the sample in memory or rewrites it, against fire and forget | Rejected |
| Sample drawn per transaction | Keeps sequences | The interest is per query; sequences come from the executions-per-transaction histogram | Rejected |
| Fixed summaries (heavy hitters, values per bucket) | Constant size; see every execution | Answer only the questions fixed in advance; an abstraction without a need yet | Rejected |
| A counter restarted every period (a fixed window) | Simple | A synchronized burst at every restart; biased towards the start of the window (H22) | Rejected |
| A deterministic token bucket after the draw | A hard limit | The real probability of a kept row depends on the order of arrival, so weights are wrong | Rejected; only the memory bound, which marks the interval degraded |
| **Exact aggregates plus executions kept with a probability that falls with the shape's decayed count, braked by a global factor before the draw** | Append only; rare shapes complete; rows spread evenly; no burst; any question can be asked later | A weight per row, calibrated, in every estimate | **Chosen** |

**Learning from estimate misses**

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Correction factors (LEO) | Simplest | A guessed number (against R3); drift; inconsistency | Rejected |
| Model assumptions per query (SQL Server 2022) | Safe | Few cases | Covered by the sampling directive |
| **Sampling directives** | Fit 7.3; explainable; hold correlation | Sampling at planning, amortized by the plan cache | **Chosen** |
| Learned estimators (AQO, MSCN and others) | Powerful | Opaque and heavy; against simplicity | Rejected |

**Evaluation of candidates**

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| What-if | No machine, no build | Depends on estimates | **Chosen without a copy** |
| Build and measure on a copy | Measured, including build time | A copy, resources, parameter values | **Chosen with a copy** |
| Build in production | — | Blocks the writers (IX2) | Rejected |

**Applying recommendations**

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Manual only | Full control | Someone must act | For indexes |
| **Automatic for directives and the last good plan, manual for indexes** | Cheap and reversible items apply alone | — | **Chosen** |
| All automatic with revert (Azure) | No intervention | With IX2, unacceptable for indexes | Rejected until an online index build exists |

## Requirements for module 17

Cheap if module 17 is designed with them, expensive to add later. They are what "dejar la puerta abierta" means in
practice.

- M1. Actual rows per node with a **completed** flag, false when a parent stopped the node early (`Limit`, `TopK`).
- M2. The planner takes the **index set, the directives (of estimates and of plans) and the accepted plans as
  inputs**, without reading the catalog directly, so it can plan against hypothetical indexes and apply directives
  without a redesign. Each cached plan records the version of its shape's directives, so a change of directive
  replans the shape (*Plan regressions*).
- M3. A **logical pages** counter per execution (pages touched by its read transaction or cursor) and its **CPU
  time**, read from its thread's CPU clock and summed over the calls of a cursor (H29), with their cost measured
  before they are assumed free.
- M4. **Usage counters** per index (seeks, scans, rows read) and **write counters** per table and kind (inserts,
  updates, deletes, bulk rows) with entries written per index. 7.3's modification counter (ST4) is the base but does
  not tell the kinds apart.
- M5. Shape and plan bytes that are **portable** across copies of a store. Already decided: identities and state tokens
  are random and kept (document 11, P12).
- M6. A **sink** at the end of each execution that receives its record, with the transaction's identity, the
  execution's ordinal within it and the parameter values; with no sink, nothing is built.
- M7. A place in the **catalog** for accepted directives and accepted plans.

## Costs

- Per execution, in the query's thread: the counters of document 8 (already per batch), a fixed record of about 64
  to 128 bytes written into its own ring, one read of the published snapshot and one random number; the parameter
  values only when the draw keeps the execution [I]. Very short queries through the query interface are where this
  shows; to measure.
- Per record, in the consumer: a lookup and additions, one histogram bucket per measure (bit arithmetic) and the
  counters; every tick, a new snapshot [I]. Its throughput bounds the rate of executions it can follow (H25).
- Sampled executions: about `max(T, p_min·N)` rows per steady shape and day, and `T(1 + ln(N/T))` in a new shape's
  first day (*Sampled executions*): 0.4 to 20 MB per day for one shape between 10⁴ and 10⁹ executions with
  `T = 2,000` and `p_min = 10⁻⁴` [I]. `R` rows per second per (shape, plan) and `B` overall are targets of the
  expected rate, which the consumer counts, not hard quotas: a new entry keeps up to its rate times one tick before
  the first snapshot that includes it (*Capture*), about 1,000 rows at 10,000 executions per second, and `g` follows
  the demand one second late (*Sampled executions*). The only hard limit is the memory bound of the buffers.
- The page counter of M3: one increment per page access, and two reads of the thread's CPU clock per execution [I];
  to measure.
- Plan regressions: at each flush, one comparison per pending candidate; during a trial, one draw per execution of
  the shape and about half its executions on the worse plan [I].
- Per interval: one deferred write in the profile store.
- Memory: bounded by the number of shapes and plans kept (pg_stat_statements keeps 5,000 statements by default [V]).
- Directives: one sample per directive at planning, kept with the plan.
- The tuner: in-process, a CPU budget and short reads; on a copy, as much as the copy allows.

## Risks

| Risk | Effect | Mitigation |
|---|---|---|
| Capture overhead on short queries | Slower point queries | Measure; one ring per thread; sink off by option |
| Extra threads in an embedded engine | Threads the application did not ask for | The consumer sleeps while the rings are empty; the writer may share the existing background thread |
| The consumer falls behind | Rings fill and drop; that interval's aggregates are inexact | Measure its cost per record; size the rings; mark the interval degraded (H25) |
| A burst of sampled rows | Buffers fill and drop, biasing the weights | The global factor `g` before the draw; a dropping interval is marked degraded (H22) |
| An analysis ignores the weights | Estimates biased towards the start of each period (H21) | The tools return weighted, calibrated estimates; every raw row carries its `p` |
| Oscillating directives | Plans flip between runs | Trial, expiry, a pause per signature after a failed directive |
| A forced plan is worse for some parameter values | The shape is slower for those values | The trial runs both plans at random over the same executions' mix and forces only a plan that is better on average; expiry starts a new trial |
| Wrong what-if recommendations | An index that does not help, or hurts writes | Write overhead in the evaluation; validation after applying; revert |
| Parameter values in the profile | Sensitive data at rest | Encrypted with the store; masking per column (question 1) |
| A profile from another store | Wrong feedback | Main store identity in `meta` (H16) |
| An index build blocks writers | Write stalls during a build | Never automatic; build time in the recommendation (H2) |
| A model invents a number | A decision taken on a wrong figure | Every number cites its evidence and a deterministic check rejects a report that does not match |
| A cloud model receives profile data | Shapes and parameter values leave the analysis machine | A local model, or parameter values off (questions 1 and 9) |
| An agent reaches production | An unreviewed change | The agent's tools reach only the copy; applying is a person's decision |

## Experiments to run when it is designed in detail

1. Capture overhead on a point query and on a range scan through module 17, with and without the sink, 10 to 20
   rounds (the bench's noise rule).
2. Cost of the page counter and of the CPU clock reads on W1-W32.
3. Gain of a sampling directive on synthetic correlated columns without a shared index.
4. Agreement between what-if and measured evaluation on the bench tables.
5. Profile size per thousand shapes over 30 days of intervals, and the sampled executions' size per day.
6. Cost per execution in the query's thread (record, ring write, snapshot read, draw), cost per record in the
   consumer and its maximum throughput, and the ring size that a flush or a pause of the consumer needs; the cost
   at rest with the consumer asleep, and whether the writer can share the statistics thread.
7. Bias and variance of weighted, calibrated estimates against the exact aggregates, on a synthetic load whose
   behavior changes late in the period (a regression in the afternoon).
8. Stability of the global factor `g` under a spike: hundreds of new shapes at once, and a restart under load; the
   spread of a new entry's first rows over time with the cap `R`.
9. Forcing the last good plan on a synthetic regression (statistics rebuilt on a skewed column that move a shape
   from an index to a full scan); and a candidate better for every parameter value, suspected because its first
   window held dearer values, which its trial must clear.

## Product questions

1. **Parameter values.** Recommended: kept by default in the sampled executions, encrypted with the store, with
   masking per column: a column marked sensitive keeps, instead of its value, the bucket of its 7.3 histogram where
   the value falls, which still serves to analyze sensitivity and estimates but not to replay with the exact value.
   Alternative: off by default, which limits the tuner to what-if (H7). Changed 2026-11-21 after the user: "lo de no
   guardar las ejecuciones individuales y los parámetros limitan bastante el análisis". Masking by default was
   declined on 2026-11-23: "no quiero perder exactitud".
2. **Profiling by default.** Recommended: capture on once M3 and M6 are measured as cheap; the tuner and automatic
   directives off until the application turns them on. Alternative: everything off by default.
3. **Where the profile lives.** Recommended: a reserved subfolder of the store's folder (like `krummi.sort`), so it
   follows the store. Alternative: a folder given by the application.
4. **Retention and size.** Recommended: 30 days of intervals and a size bound, evicting the cheapest shapes.
   Alternative: options with no default bound.
5. **Automatic directives and the last good plan.** Recommended: on when profiling is on, with trial and expiry.
   Alternative: only as tuner recommendations.
6. **Who starts the in-process tuner.** Recommended: the application calls it. Alternative: a schedule option run by
   the store's background thread.
7. **Automatic indexes.** Recommended: never, while IX2 holds the write turn. Alternative: automatic once an online,
   batched index build exists, with revert.
8. **Thresholds** (q-error 4×, 100 rows, three observations, a 15-minute interval; for sampling `T = 2,000`,
   `p_max = 1` fixed, `p_min` deferred (10⁻⁴ when it comes), `τ` = 1 day, the cap `R = 10` per entry and second, the
   budget `B`, a 100 ms tick and `k = 4`; for plan regressions 30 executions and 2×). Recommended: fixed defaults,
   tunable later by measurement. Alternative: options from the start.
9. **Which agent on the analysis machine, and what may leave it.** Recommended: either command-line tool through the
   same tools and skill; a local model when parameter values are captured, a cloud model allowed only for shapes and
   evidence without values. Alternative: a local model only.

## Sources

Repository: `docs/prd/krummi/krummi.prd.md` (D6, G4, G8, G9, modules 10 and 12), `docs/prd/krummi/tables/tables.prd.md`
(N8, module 17), `docs/prd/krummi/tables/tables-statistics.prd.md` (ST1-ST7, ST17, ST18),
`docs/prd/krummi/tables/tables-indexes.prd.md` (IX2, IX3), `docs/prd/krummi/journal/journal.prd.md` (J5, J10),
documents 4, 8 and 11 of this set.

External, fetched 2026-11-21: SQL Server CE feedback:
https://learn.microsoft.com/en-us/sql/relational-databases/performance/intelligent-query-processing-cardinality-estimation-feedback?view=sql-server-ver17 ;
intelligent query processing (DOP and memory grant feedback):
https://learn.microsoft.com/en-us/sql/relational-databases/performance/intelligent-query-processing?view=sql-server-ver17 ;
Query Store options (`ALTER DATABASE SET`):
https://learn.microsoft.com/en-us/sql/t-sql/statements/alter-database-transact-sql-set-options?view=sql-server-ver17 ;
Azure SQL automatic tuning: https://learn.microsoft.com/en-us/azure/azure-sql/database/automatic-tuning-overview?view=azuresql ;
Database Engine Tuning Advisor:
https://learn.microsoft.com/en-us/sql/relational-databases/performance/database-engine-tuning-advisor?view=sql-server-ver17 ;
Oracle query optimizer concepts (statistics feedback, SQL plan directives):
https://docs.oracle.com/en/database/oracle/oracle-database/19/tgsql/query-optimizer-concepts.html and
https://docs.oracle.com/en/database/oracle/oracle-database/19/tgsql/optimizer-statistics-concepts.html ; LEO:
https://vldb.org/conf/2001/P019.pdf ; q-error: https://www.vldb.org/pvldb/vol2/vldb09-657.pdf ; STHoles:
https://www.microsoft.com/en-us/research/publication/stholes-a-multidimensional-workload-aware-histogram/ ; ISOMER:
https://research.ibm.com/publications/isomer-consistent-histogram-construction-using-query-feedback ; AutoAdmin
what-if: https://www.microsoft.com/en-us/research/publication/autoadmin-what-if-index-analysis-utility/ ; HypoPG:
https://github.com/HypoPG/hypopg ; `pg_stat_statements`: https://www.postgresql.org/docs/current/pgstatstatements.html ;
AQO: https://github.com/postgrespro/aqo

From knowledge, not fetched (sites that refuse automated fetches): the release dates of Oracle's cardinality
feedback and its rename; Aboulnaga and Chaudhuri, "Self-tuning histograms: building histograms without looking at
data", SIGMOD 1999; candidate merging in the Database Engine Tuning Advisor.

Statistics and data structures from knowledge, not fetched: Cochran's sample size formula with the finite population
correction; the Horvitz-Thompson and Hájek estimators; Kish's effective sample size; Vitter's reservoir sampling
(Algorithm R); HdrHistogram's log-linear buckets; DDSketch's geometric buckets; fixed-window, sliding-window and
token-bucket rate limiters; AWS X-Ray's sampling rules (reservoir and fixed rate), Jaeger's adaptive sampling and
OpenTelemetry's probability sampling with its adjusted count.
