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

Status: proposal. Nothing here is implemented, built or measured, and no PRD takes it yet. It comes after module 17
(queries), which it builds on; what module 17 must provide from its first version so that this work needs no
redesign later is listed in *Requirements for module 17*. The product questions at the end stay open until this work
is planned.

Evidence labels, as in documents 13 and 14: **[V]** verified in a primary source (fetched 2026-11-21, with the URL in
*Sources*); **[K]** Krummi's own documents; **[I]** inference or estimate; **[L]** from knowledge, not fetched.

## Summary

1. **Three layers, each useful alone.** A **profile** records what queries run, how often, what they cost and how far
   their estimates missed. **Estimate feedback** turns repeated misses into small, explainable **directives** that the
   planner applies. A **tuner** ranks the queries by total cost and recommends indexes, index removals, directives and
   plans for the few that matter.
2. **The profile lives in a separate Krummi store**, written by a background thread in one deferred commit per
   interval, fire and forget. A query only adds to counters in memory. The profile never takes the main store's write
   turn, and a profile that fails to open, write or read turns profiling off without failing any query.
3. **Feedback corrects the source of an estimate, never the number.** No correction factors: a predicate whose
   estimate misses by more than 4× in the same direction, over several executions, gets a directive that estimates it
   from a sample of the table, which already holds the correlation between its columns. Directives are validated after
   they apply and expire when the statistics or the catalog change.
4. **One tuner, two places to run.** The tuner is a library function over a snapshot of the main store and the
   profile, with one output format. Without an analysis machine it runs inside the process, at low priority, with short
   reads, and evaluates candidate indexes as hypothetical indexes (*what-if*). With a machine it runs on online backups
   of both stores, builds the candidates for real and measures them. In both cases applying a recommendation is an
   explicit call, accepted items live in the main store's catalog, and the profile validates the change afterwards.
5. **Cost is measured in logical pages, not time.** Krummi maps its data file, so physical reads per query cannot be
   observed; pages touched can, and they compare across machines. Duration ranks only within one machine.
6. **Indexes are never created automatically.** Building one holds the write turn for the whole build (IX2), so it is
   applied in a window chosen by the application. Directives, which are cheap and reversible, can apply on their own.

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
| H7 | Replaying a query needs its parameter values | Without them the tuner can only estimate, which is what fails | A bounded, opt-in sample of parameter values per shape, encrypted with the store (question 1) |
| H8 | An index makes reads cheaper and writes dearer | A tuner that sees only queries recommends indexes that sink write loads | Writes per table and kind, and entries written per index, in the profile (M4) |
| H9 | Candidates for several shapes overlap | Five similar indexes, past the limit of 64 per table (IX3) and the write budget | Merge candidates that share a prefix; a budget per table |
| H10 | A daily window misses periodic loads | Month-end queries never rank | A rolling window of 7 to 30 days, weighted towards recent days (question 4) |
| H11 | Ranking by total cost misses rare slow queries | A screen with a 30 s p99 never ranks | Two lists: total cost, and p95 and maximum |
| H12 | A full profile | Query Store goes read-only when full [V] | Evict the cheapest shapes; never stop recording |
| H13 | Many threads update one map | Contention on the query path | Sharded accumulators, swapped and merged at each flush |
| H14 | Direct table calls (read by key, scan an index) are not module 17 queries | They cost resources but have no plan to tune | Counted per table and index (M4); not tuned |
| H15 | What survives the loss of the profile | If accepted directives and fixed plans live in the profile, a damaged profile changes production performance | Learned directives live in the profile and are disposable; accepted items live in the main store's catalog (M7) |
| H16 | A profile copied next to another store | Feedback from one store applied to another | The profile records the main store's identity and is discarded when it differs |
| H17 | Unique indexes are constraints | Dropping an unused unique index changes what the application can write | Never recommend dropping a unique index |
| H18 | The feedback loop and the tuner act on the same shapes | A directive changes the plan, the profile sees another plan, the tuner adds an index, the directive no longer fits | Directives are bound to the catalog state tokens (P12) and the statistics they were learned on, and expire with them |

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
logical pages touched, peak memory, bytes spilled, replans, and per node the estimated and actual rows with the
completed flag (M1). With no sink, nothing is built.

The sink adds the record to sharded in-memory accumulators keyed by (shape, plan) for the current interval: a hash
lookup and a few additions under a shard's lock. The first time a shape or plan is seen, its canonical bytes are
kept once, identified by hash with a full comparison after it (document 11). Per shape it also keeps the few worst
executions of the interval by pages, with their parameter values when capture of values is on (question 1).

### Flush

Every interval (15 minutes as a start [I]; Query Store aggregates per 60 minutes and writes every 15 by default [V])
the profile's background thread swaps the accumulators, merges the
shards and writes the interval in one write transaction of the profile store, committed in deferred mode (J10). A
crash loses at most the interval in flight, which is acceptable for statistics that need be neither exact nor
current. If the write fails or the profile is slow, the next interval merges in memory up to a memory bound; past
it the cheapest shapes are dropped and counted. A query never waits for any of this.

### Contents

Krummi tables in the profile store, so the tuner and the application can query them with module 17:

| Table | Key | Contents |
|---|---|---|
| `meta` | — | Format version, the main store's identity (H16), dropped counts, first and last interval |
| `shapes` | shape id | Canonical shape bytes, table, first and last seen |
| `plans` | plan id | Shape, plan bytes, origin (planner, directive, accepted), first and last seen |
| `runtime` | plan, interval start | Executions; errors by class; cancelled; duration sum, minimum, maximum and a logarithmic histogram (for p95); rows; pages sum and maximum; peak memory; bytes spilled; replans |
| `node_estimates` | plan, node, interval | Completed executions; maximum q-error; executions over 4×, under and over |
| `predicate_feedback` | predicate signature, interval | Observations; under and over counts; maximum q-error; estimate source (statistics, sample, stale statistics); the statistics' modification counter |
| `table_writes` | table, interval | Inserts, updates, deletes and bulk rows; per index, entries written, seeks, scans and rows read (M4) |
| `samples` | shape, interval, rank | The worst executions with their parameter values, when capture of values is on |
| `directives` | predicate signature | Learned directives with their state (*Estimate feedback*) |

Interval starts use the wall clock in UTC; durations use the monotonic clock. Retention follows question 4: old
intervals are removed by the flush, and when the profile passes its size bound the cheapest shapes go first (H12).

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
   with the worst p95 or maximum (H11).
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

## Alternatives considered

**Where the profile lives**

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Internal trees of the main store | One file; in every backup | Takes the write turn (G4); grows the main store and its journal | Rejected (R2) |
| **A separate Krummi store** | Isolated; reuses the engine, encryption and backup | A second folder and lock | **Chosen** |
| Rotated log files | Cheapest writes; easy to ship | A new format with no integrity check; querying means loading | Rejected (DRY, G6) |
| A callback or OpenTelemetry export | Fits existing monitoring | The tuner cannot rely on it | Possible later as an extra sink |

**Granularity of capture**

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Every execution | Full detail; replay | Volume and overhead | Rejected |
| Aggregates per interval | Bounded and cheap | Loses outliers | Part of the choice |
| **Aggregates plus a sample of the worst executions** | Cheap, keeps what replay needs | Slightly more logic | **Chosen** |

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
| **Automatic for directives, manual for indexes** | Cheap and reversible items apply alone | — | **Chosen** |
| All automatic with revert (Azure) | No intervention | With IX2, unacceptable for indexes | Rejected until an online index build exists |

## Requirements for module 17

Cheap if module 17 is designed with them, expensive to add later. They are what "dejar la puerta abierta" means in
practice.

- M1. Actual rows per node with a **completed** flag, false when a parent stopped the node early (`Limit`, `TopK`).
- M2. The planner takes the **index set, the directives and the accepted plans as inputs**, without reading the
  catalog directly, so it can plan against hypothetical indexes and apply directives without a redesign.
- M3. A **logical pages** counter per execution (pages touched by its read transaction or cursor), with its cost
  measured before it is assumed free.
- M4. **Usage counters** per index (seeks, scans, rows read) and **write counters** per table and kind (inserts,
  updates, deletes, bulk rows) with entries written per index. 7.3's modification counter (ST4) is the base but does
  not tell the kinds apart.
- M5. Shape and plan bytes that are **portable** across copies of a store. Already decided: identities and state tokens
  are random and kept (document 11, P12).
- M6. A **sink** at the end of each execution that receives its record; with no sink, nothing is built.
- M7. A place in the **catalog** for accepted directives and accepted plans.

## Costs

- Per execution: the counters of document 8 (already per batch), one record, a hash lookup and additions under a
  shard's lock [I]. Very short queries through the query interface are where this shows; to measure.
- The page counter of M3: one increment per page access [I]; to measure.
- Per interval: one deferred write in the profile store.
- Memory: bounded by the number of shapes and plans kept (pg_stat_statements keeps 5,000 statements by default [V]).
- Directives: one sample per directive at planning, kept with the plan.
- The tuner: in-process, a CPU budget and short reads; on a copy, as much as the copy allows.

## Risks

| Risk | Effect | Mitigation |
|---|---|---|
| Capture overhead on short queries | Slower point queries | Measure; sharded accumulators; sink off by option |
| Oscillating directives | Plans flip between runs | Trial, expiry, a pause per signature after a failed directive |
| Wrong what-if recommendations | An index that does not help, or hurts writes | Write overhead in the evaluation; validation after applying; revert |
| Parameter values in the profile | Sensitive data at rest | Off by default, bounded, encrypted with the store (question 1) |
| A profile from another store | Wrong feedback | Main store identity in `meta` (H16) |
| An index build blocks writers | Write stalls during a build | Never automatic; build time in the recommendation (H2) |

## Experiments to run when it is designed in detail

1. Capture overhead on a point query and on a range scan through module 17, with and without the sink, 10 to 20
   rounds (the bench's noise rule).
2. Cost of the page counter on W1-W32.
3. Gain of a sampling directive on synthetic correlated columns without a shared index.
4. Agreement between what-if and measured evaluation on the bench tables.
5. Profile size per thousand shapes over 30 days of intervals.

## Product questions

1. **Parameter values.** Recommended: off by default; when on, a bounded sample of the worst executions per shape,
   encrypted with the store. Alternative: never stored, which limits the tuner to what-if.
2. **Profiling by default.** Recommended: capture on once M3 and M6 are measured as cheap; the tuner and automatic
   directives off until the application turns them on. Alternative: everything off by default.
3. **Where the profile lives.** Recommended: a reserved subfolder of the store's folder (like `krummi.sort`), so it
   follows the store. Alternative: a folder given by the application.
4. **Retention and size.** Recommended: 30 days of intervals and a size bound, evicting the cheapest shapes.
   Alternative: options with no default bound.
5. **Automatic directives.** Recommended: on when profiling is on, with trial and expiry. Alternative: only as tuner
   recommendations.
6. **Who starts the in-process tuner.** Recommended: the application calls it. Alternative: a schedule option run by
   the store's background thread.
7. **Automatic indexes.** Recommended: never, while IX2 holds the write turn. Alternative: automatic once an online,
   batched index build exists, with revert.
8. **Thresholds** (q-error 4×, 100 rows, three observations, a 15-minute interval). Recommended: fixed defaults,
   tunable later by measurement. Alternative: options from the start.

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
