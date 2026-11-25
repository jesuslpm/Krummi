# Approximate Vector Indexes — PRD

**Version**: 1.1
**Date**: 2026-11-25
**Parent document**: [Krummi Tables — PRD](tables.prd.en.md) (v1.0)

> English translation of [tables-vector-indexes.prd.md](tables-vector-indexes.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Build child PRD **7.6** of the [split of the tables module](tables.prd.en.md#split-into-child-prds):

- an **approximate vector index** on a vector column, of a class different from the secondary indexes of 7.2;
- its **asynchronous maintenance** in the background;
- a **search for the k nearest neighbors** that combines relational filters with distance: it first filters by the
  relational conditions and then searches by distance among the rows that meet them;
- its measurement (N11).

**User's goal**, in their words: «quisiera como objetivo ser capaz de combinar filtros normales relacionales
con filtros vectoriales, donde primero se filtre por los relacionales normales y luego se busquen en los vectores» (as a
goal, I would like to be able to combine normal relational filters with vector filters, where the normal relational ones
filter first and then the vectors are searched).

**Why now**: 7.5 and 7.7 are closed, and the annotation of 2026-11-10 to the parent builds 7.6 after 7.5 and before
module 17, so that queries can search for approximate neighbors from the start. The
[experiment of design document 12](../../../benchmark/2026-11-24-ann-experiment.md) measured the algorithm against its
thresholds: with 1 M vectors of 768 dimensions and L2, IVF-SQ8 finds 95.8 % of the 10 exact neighbors at a p95 of
17.20 ms, one thread, probing 4 % of the lists. The user decided to go on with IVF-SQ8: «Sigue con IVF-SQ8».

The technical design is in [design document 12](../../../design/tables/12-vector-indexes.md) (structures, the pending
rows log, reconciliation, the worker, the states, the guarantees of the searches, training and quantizer drift) and in
[document 5](../../../design/tables/05-vectors.md) (the element types, the metrics, the deterministic distance kernels
and the exact search), which are the technical starting state of this PRD. This PRD changes three parts of document 12:

- **The filters.** The document chose between the exact distance and the index with a planner estimate, and with the
  index it widened the probe and filtered when re-ranking. Here the search first finds the rows that meet the filter,
  chooses the path according to how many pass and, with the index, considers only those rows (VI6).
- **The rebuilds.** The document proposed them as explicit, with indicators that recommend them; here they are
  automatic, and the application can also request them (VI9).
- **The criteria for switching to HNSW.** In the experiment two are triggered: the dot product misses the p95 by 0.9 ms
  at base frequency, and GloVe needs 14.7 % of the lists. The user, with those figures in view, kept IVF-SQ8 for all
  the metrics.

Decisions of the user of 2026-11-25:

- the algorithm, after the experiment: «Sigue con IVF-SQ8»;
- how to search among the rows that pass the filter: «Híbrido (Recomendado)», that is, exact distance when few rows
  pass and the index, limited to those that pass, when many pass (VI6);
- which filters it accepts: «Lo de 7.3, módulo 17, IN y NOT IN» (VI7), with nulls as in SQL;
- with fewer than k candidates in the probed lists: «Ampliar el sondeo (Recomendado)» (VI6);
- the elements: «f32, f16 e i8 (Recomendado)» (VI2);
- the distance of L2: «distancia euclídea» (VI8);
- the exact search: «Sí, pública (Recomendado)» (VI16);
- the minimum to train: «10.000 filas (Recomendado)» (VI4);
- the rebuilds: «Automática» (VI9);
- the turn and the threads: «15 ms y 1 hilo (Recomendado)» (VI3, VI4);
- the comparison reference: «pgvector» (VI15);
- the other proposals of the draft, which follow the rules of 7.2 and 7.3 (VI1, VI9 on request, VI10 to VI13): «Aceptar
  todas (Recomendado)».

*Annotation of 2026-11-25, at the start of the execution (v1.1): the execution runs on Kubuntu and cannot test or
measure on Windows (user: «Solo Linux, anotado»). This PRD is validated on Linux x86-64 with glibc only and Windows stops
being a criterion: the tests, AC-VI10, AC-VI11 and the measurements of VI15 are Linux's, and the PRD can close without
Windows. It is an exception to D3 and AC18 of the [root](../krummi.prd.en.md), which carries its annotation.*

This PRD inherits everything from its ancestors. It makes precise or extends rules of its parent, of
[tables-schema.prd.en.md](tables-schema.prd.en.md) (7.1) and of [tables-indexes.prd.en.md](tables-indexes.prd.en.md)
(7.2), which carry its annotation (*Annotations to other PRDs*).

In this document, a **vector index** is an approximate vector index; its **pending log** is the list of rows that
changed and that the index does not yet reflect; the **worker** is the store thread that brings it up to date; a
**generation** is a complete version of the index, trained and built; and the **filter** of a search is its relational
conditions.

### Product decisions

| ID | Decision |
|---|---|
| VI1 | **Index class**: a vector index is created on a vector column of a table, with a name from the same namespace as the secondary indexes of that table, and a metric (VI2). It divides the vectors into lists by their nearest centroid (IVF) and stores for each row an 8-bit code per component (SQ8). A search probes the lists nearest to the query and re-ranks the candidates with the exact distance. It is not a secondary index (N7): it has no column key, admits no included columns, is not unique and is not traversed. A table may have several, also on the same column with different metrics, but not two equal ones (same column and metric). It follows the rules of 7.2 for names (IX1), the create new and create if it does not exist modes (IX2; a definition is another one if its column or its metric changes), drop (IX4) and the changes for T10 (IX13); it counts toward the maximum of 64 indexes per table and toward the size of the schema (IX3). Describe (IX6) also returns the list of the vector indexes of the table, apart from that of the secondary ones and empty if it has none, in the order of their names according to B5, each with its identity, its name, its column (identity and name) and its metric; creating or dropping one changes the state of the indexes of IX6, and its training, its construction and its rebuilds do not. Naming a vector index in an operation of the secondary indexes (traversing, reading their statistics) fails like a secondary index that does not exist. A row with a null vector is not in the index and is never a result. |
| VI2 | **Elements and metrics** (user: «f32, f16 e i8 (Recomendado)»): columns of `f32`, `f16` or `i8` elements, with the L2 or dot product metric; in a column with the cosine metric (N4), the metric of the index is cosine. Columns of `bit` elements do not admit a vector index. The distances are those of document 5, with its deterministic kernels: the exact search gives the same rows and distances on all platforms with the same data, and the approximate one does too with the same store file. Two indexes built separately on the same data may differ, because their training depends on the sample. |
| VI3 | **Asynchronous maintenance**: a write that inserts, updates or deletes a row only records the row in the pending log of each vector index of the table, in the same transaction; an update records it only if the vector changes. The worker, a store thread, takes the pending rows in short batches and brings the index up to date in writes of its own, which take the write turn (G4) like any writer, in their order of arrival: a write of the application may wait for a batch, and that wait counts toward its time limit (T4), as with statistics (N8). A batch holds the turn for at most 15 ms, the threshold of document 12 (user: «15 ms y 1 hilo (Recomendado)»), and its commits do not wait for the disk: a crash may lose batches, always the last ones, and their work is repeated on opening. A batch is a write of the worker's own, with changes for T10 and in the journal like any other; it changes no rows of the table, and since it is another transaction it does not end the references of those of the application (A10). A batch prepared for an index that is no longer the same (dropped, emptied or created again, or with another generation published) is discarded without writing anything. The index never stops reflecting a committed row: either it reflects it, or the row is in the pending log. |
| VI4 | **Create, train and build**: creating the index is a short write that stores its definition; it does not block the writes of the application. Afterwards, the worker trains it with a sample of the rows (centroids and quantizer) and builds it by traversing the table in batches, while the application keeps writing; the rows that change during construction are collected from the pending log. States: **training** (from when it is created, or from when the table is emptied, VI10), **building** (after training), **ready** (when construction ends) and **ready and rebuilding** (VI9, back to ready on publishing). It trains when the table has at least 10,000 rows with a vector (user: «10.000 filas (Recomendado)»); until then it keeps training. Training uses a single thread, the worker's, and a sample of at most 256 vectors per list (the same «15 ms y 1 hilo (Recomendado)»). The number of lists comes from the rows with a vector at training: those rows divided by 1,000 and rounded down, up to 1 M rows, and above that their square root rounded down. |
| VI5 | **Search**: within a read or write transaction, on an open table, the **k nearest neighbors** of a query vector are requested, naming the vector index, which decides the column and the metric, with an optional filter (VI7) and optionally how many lists to probe first (VI6). It returns up to k rows visible in the transaction's snapshot that meet the filter, each with its exact distance, from lowest to highest distance, with ties ordered by primary key. k goes from 1 to 10,000 (document 5). A result is approximate only because of the lists it does not probe and because of the 8-bit codes; the worker's lag never hides a row, because the search evaluates the rows of the pending log with the exact distance. While the index is training or building (there is no ready generation), the search computes the exact distance of all the rows that pass the filter: the result is exact. |
| VI6 | **Filter first** («Híbrido (Recomendado)»): with a filter, the search first finds the rows that meet it and then searches among them by distance. Without a filter, all rows meet the filter. Throughout this PRD, the rows that **pass** are those that meet the filter and have a non-null vector (VI1): they are the ones that count for the threshold and for k. If at most a **row threshold** pass, it computes the exact distance of all of them and the result is exact; if more pass, it traverses the vector index considering only the rows that passed, and the result is approximate. If the probed lists have fewer than k candidates that passed, it keeps probing the following lists in order of nearness until it gathers k or has traversed them all, so it returns k rows whenever at least k pass (user: «Ampliar el sondeo (Recomendado)»). The number of lists that the application requests is that of the first probe; the widening applies then too. The threshold is set by the implementation with a measurement, in rows, and the search reports the path it took: exact, or approximate when it used the index, even if it probed all the lists. In both cases it only returns rows that meet the filter. |
| VI7 | **Filter conditions** («Lo de 7.3, módulo 17, IN y NOT IN»): those of an estimate of 7.3 (equal to a value in a column, in an interval of a column, equal in several columns, ST2), plus **is in** a list of values (IN) and **is not in** a list of values (NOT IN) of a column, joined with AND. The values have the type and the domain of their column, as in ST2, and are compared after the normalization of N4 and the collation of their column, as in the key of an index (N5), so a NaN in a list finds the rows with NaN. Several conditions on the same column must all be met, and a repeated value in a list changes nothing. Nulls follow SQL (user: «Como en SQL: supongo que es así: un nulo en IN es como si no estuviera, un nulo en NOT IN siempre da NULL» (as in SQL: I suppose it is like this: a null in IN is as if it were not there, a null in NOT IN always gives NULL)): a row with a null column never meets a condition on that column; an equality with a null, or an interval with a null as a bound, lets no row pass; a null in an IN list lets nothing pass by itself; and a NOT IN whose list has a null lets no row pass. This differs from ST2, where equal to null estimates the null rows. The other conditions (OR, not equal to, JSON paths, a distance threshold) arrive with module 17, which will be able to use this search with its own filters. |
| VI8 | **Distance units** (user, for L2: «distancia euclídea»): L2 returns the Euclidean distance, not its square; dot product returns the product with its sign changed, so that lower is nearer; cosine returns one minus the cosine. They are `f64` numbers. The order and the ties are decided with the value that document 5 compares (the square, in L2), so two rows at Euclidean distances that coincide when rounded stay in their order. It is question 6 of document 5, which was open. |
| VI9 | **Rebuild** (user: «Automática»): the worker rebuilds the index on its own when its indicators pass their thresholds: the fraction of codes with some component outside the range of the quantizer, the size of the largest list against the average, and the rows the table has against those it had at training, which change the number of lists (VI4). A rebuild trains and builds a new generation while searches keep using the previous one, and publishes it at once: a search sees the previous generation or the new one, never a mix. There is at most one rebuild in progress per index, and only a ready index of a table with at least 10,000 rows with a vector is rebuilt. The application can also request one, in a write: it takes effect when it commits, survives closing, and requesting it with another in progress, with the index training or building, or with fewer than 10,000 rows does nothing. |
| VI10 | **Table changes**: renaming the column keeps the index; removing a column with a vector index fails with invalid schema, as with a secondary index (E5); emptying the table removes the entries and the pending log, and the index goes back to training; dropping the table or the index frees all its pages in that write, also a table that another overwrites when it is renamed (E2); renaming the table keeps its vector indexes; the bulk load records each loaded row in the pending log, and a load that fails as a whole leaves the log as before (E13). |
| VI11 | **Index information**: a public operation, in a read or write transaction and according to what that transaction sees, returns for a vector index: its state (training, building, ready, or ready and rebuilding); the number of lists of the generation that searches use, or none if there is none; the number of rows of the pending log; and the indicators of VI9 of that generation. An indicator that cannot be computed is none, like the number of lists: all three, without a ready generation; the fraction of out-of-range codes, in a generation without entries; and the largest list against the average, when all the lists are empty. An indicator that is none never triggers a rebuild. The information of the store (P12) does not change. |
| VI12 | **Closing and opening the store**: closing, or ceasing to use the store without closing it, stops the worker: it abandons a training in progress and waits for the batch that already holds the turn to end. Nothing committed is lost: the pending log, the state and the progress of the construction survive closing, and the worker continues on opening, training again if it was training. A temporary store (G9) works the same, in memory. |
| VI13 | **Worker failures**, like ST9: a failure of a batch when reading (damage, input/output, out of memory) or when storing without making the store stop accepting writes (disk full of G7, maximum size of P10) leaves the index as before that batch, without informing anyone, and the worker retries it in its next pass; searches stay correct, because the rows remain pending. An input/output failure when committing makes the store stop accepting writes (T12), like that of any writer; the next operation of the application reports it, and the worker stops. The space the index takes counts toward the disk and the maximum size like that of any tree. |
| VI14 | **Format**: the change is incompatible (B12) and the format version goes from 15 to 16. |
| VI16 | **Exact search** (user: «Sí, pública (Recomendado)»): the same search of VI5, with the same filters, requested as exact and naming a metric instead of an index: it computes the exact distance of all the rows that pass the filter, never uses a vector index and does not need one to exist. It admits all the element types, `bit` included with the Hamming distance, and the metrics of VI2 in the others. Its result is always exact, with the order, the ties, the units and the maximum k of VI5 and VI8. It is the exact search of document 5 that the parent leaves as "never use the index". |
| VI15 | **Measurement**: there are no new benchmark loads with a threshold against Voron, which has no vector indexes in its storage layer. The comparison reference is **PostgreSQL with pgvector** (user: «pgvector»), with its IVFFlat index and its iterative searches (`ivfflat.iterative_scan`, since pgvector 0.8.0), on the same machine and with the same data. It is an informative comparison in the closing report, with a script and without a threshold: PostgreSQL does not enter the benchmark. It compares, on one thread (`max_parallel_workers_per_gather = 0`), the p95 of Krummi and of pgvector with the same recall@10 of at least 0.95, without a filter and with a filter that lets few rows pass and another that lets many pass, and the p95 of inserting a row into a table with the index. It reports separately the p95 of an empty PostgreSQL query (`SELECT 1`), which measures what its protocol costs, and the versions of PostgreSQL and pgvector used. Installing them on the measurement machine is part of the execution. The closing report measures, with the Cohere data of the experiment (1 M × 768, L2), one thread, warm cache, no pending rows and the worker with no work, and on Linux (annotation of 2026-11-25), these thresholds of document 12: recall@10 of at least 0.95 with a p95 of at most 20 ms, without a filter; the p95 of inserting a row into a table with the index, at most 10 % above that without it; and the time a batch holds the turn, at most 15 ms (VI3). A threshold that is not met leaves the PRD not closed until the user decides. With N11 since 7.3, also a round of all the benchmark loads before and after the change, and 10 to 20 rounds of the table write loads (W23 to W32), which are the ones affected, and of those that deviate by more than 3 %. |

### Starting state

Verified on 2026-11-25 in the code, in the PRDs of the ancestors and in the design documents:

| Aspect | Situation |
|---|---|
| Vector columns | They exist since 7.1, with `f32`, `f16`, `i8` and `bit` elements, a dimension of 1 to 4,096 (a multiple of 8 for `bit`) and the optional cosine metric, except in `bit`; a cosine column stores the norm next to the vector and admits no zero vectors (N4). |
| Distances | There are no distance kernels, no exact search for the k neighbors and no distance threshold filter in the engine: only the computation of the norm of a cosine column. The kernels of document 5 exist outside the engine, in the experiment laboratory. |
| Secondary indexes | They exist since 7.2: they are created in a single write that holds the turn while it traverses the table, are maintained in every write of a row and admit no vector in their key, although they do as an included column. A table has at most 64. |
| Background | Of the tables, the store only has one thread, that of the statistics of 7.3, which takes the write turn like any writer and stops on closing; the other threads of the store (that of the journal and the transfer one) do not take the turn. |
| Filters | The estimate of 7.3 accepts equality in a column, an interval of a column and equality in several columns. There is no IN, NOT IN or a search that filters rows by conditions: traversals are by ranges of the primary key or of an index. |
| Queries | Module 17 (planner and query nodes) does not exist. |
| Store information | P12 reports the format version, the page size, the size, the pages and the free pages; nothing about tables or indexes. |
| Format | The current version is 15. |
| Benchmark | 32 loads; none with vectors. |
| Measurement | The experiment of document 12 measured IVF-SQ8 in memory: it meets the thresholds at 1 M × 768 with L2; with the dot product it misses the p95 by 0.9 ms at base frequency, and with GloVe it needs 14.7 % of the lists. |

## Actors

- **Application developer (Rust)**: creates vector indexes, searches for neighbors with relational filters in Rust,
  queries the state of an index and, if they want, requests a rebuild before the worker does it on its own.
- **Maintainer**: runs the tests and the measurements of VI15 on Linux (annotation of 2026-11-25).

## Flow

1. The application creates a vector index on a vector column of a table, with a metric (VI1, VI2).
2. The worker trains and builds it in the background while the application keeps writing (VI4).
3. Each write of a row records the row as pending, and the worker brings the index up to date (VI3).
4. In any transaction, the application requests the k nearest neighbors of a vector, with an optional relational filter
   (VI5–VI7); the search filters first and searches afterwards.
5. The worker rebuilds the index when its indicators pass their thresholds; the application can query its state and
   request a rebuild (VI9, VI11).

## Validations and messages

Those of the ancestors apply. This PRD adds, only in the library:

| Situation | Error class |
|---|---|
| An index name that is not 1 to 1,024 bytes long | Invalid size (B7, B8), as in 7.2 (IX1) |
| Creating a vector index on a column that does not exist, that is not a vector, with `bit` elements, with the L2 or dot product metric on a cosine column or cosine on one that is not, equal to another vector index of the table, or that exceeds the maximum of indexes | Invalid schema, with the rule, as in IX3 |
| Create new an index with a name that an index of the table already has; create if it does not exist with that name and another definition | Tree already exists; different schema, as in IX2 |
| Searching with, requesting the information of or requesting the rebuild of a vector index that does not exist, or searching naming a secondary index | Different schema, with the table and the index, as in IX9 |
| Searching, requesting the information or rebuilding through an opening of a table that the write dropped | Tree not found (A6, E17) |
| Searching without a query vector, with one of another dimension or element type than the column, with NaN or infinities, or with a zero vector with the cosine metric | Invalid value, with the rule of N4 |
| Searching with k outside 1 to 10,000, with a number of lists to probe equal to zero, or, in the exact search, with a metric that the column does not admit | Invalid value |
| A filter with a column that the table does not have | Different schema, with the table and the column |
| A filter on a vector or JSON column | Invalid schema, rule vector column or JSON column, as in ST12 and JS10 |
| A filter with a value of another type or outside the domain of its column, with an empty IN or NOT IN list, or with an equality on several columns with a repeated column | Invalid value |
| Removing a column with a vector index | Invalid schema, with the index, as in IX5 |
| A failure when reading during the search | Damage, input/output or out of memory, with T11 in a write |

A number of lists to probe greater than the number of lists probes all of them. In the exact search, and while the index
is not ready, the number of lists to probe is not used. Searches and the information are not changes for T10; requesting
a rebuild is, like analyzing in a write (ST7), because it stores the request. Dropping a vector index that does not exist
does nothing, as in IX4.

A search on an empty table, with a filter that lets no row pass, or while the index is not ready is not an error.

## Output

The module writes nothing on screen. Searching, approximate or exact, returns a list of up to k results, empty if no row
passes, and a single flag for the whole search that says whether the result is exact or approximate (VI6); the exact
search and the approximate one while the index is not ready always say exact. Each result carries the primary key values
of its row, decoded into a copy owned by the application as in IX8, and its distance, an `f64` in the units of VI8; they
go from lowest to highest distance, with ties by primary key. The rows are read afterwards by their primary key. The
index information returns the fields of VI11, in a copy owned by the application. The other operations do not change
their output.

## Reuse

- The distance kernels and the exact search of document 5, which enter the engine.
- The conditions of the estimate of 7.3 (ST2) and the rules for the values of an interval of an index (IX9).
- The traversals by a secondary index (7.2) to find the rows that meet the filter.
- The write turn and its queue (G4, T4), and the model of the statistics thread (ST6, ST10) for the worker.
- The identities of tables, columns and indexes (E2), so that the worker discards a batch of an index that no longer
  exists.

## Out of scope

- Vector indexes on columns of `bit` elements and the Hamming distance in an index.
- HNSW and other graphs: the experiment did not choose them (user decision).
- Tenant partition columns (question 3 of document 12): filtering first covers them (VI6).
- Included columns in a vector index.
- A cap on the pending log that a search skips (question 2 of document 12): the search always evaluates the pending
  rows.
- Parallelism within a search.
- The cost-based planner, the query nodes and the conditions that are not those of VI7 (module 17).
- Filtering by a distance threshold: it is exact and belongs to module 17.
- More compact quantizations (PQ, RaBitQ), DiskANN and SPANN.
- Returning the columns of the row in the search result: the application reads them by the primary key.
- Showing or checking a vector index from the command-line tool: modules 10 to 12 treat it as trees (N1).
- Canceling a build or a rebuild in progress (dropping the index does end it), and reporting its progress.
- Reporting the worker's failures: they are retried without notice, like those of the statistics (VI13, ST9).
- An operation that estimates the recall of the index: the application measures it by comparing with the exact search
  (VI16).
- Slowing down writers when the pending log grows.
- A time limit or a cancellation of its own for a search.
- Per-tenant centroids and splitting lists locally.
- Configuring the turn cap, the threads or the memory of the worker, or the threshold of VI6.

## Left to the implementation

- How the index, the pending log and the generations are stored in trees, with the starting point of document 12.
- The number of lists a search probes if it is not requested, the number of candidates it re-ranks and the row
  threshold of VI6, as long as VI15 and the acceptance criteria are met.
- The thresholds of the indicators that trigger a rebuild (VI9), set with measurements.
- The margin of the quantizer and the number of rows of a batch, within the 15 ms of VI3.
- The names of the operations, the types and the error classes, in English and with the conventions of Rust (D8).
- How the test hooks of the criteria are built.

## Acceptance criteria

Those of the ancestors apply.

1. **AC-VI1** — Given a table with a vector column of each admitted element type and with the cosine metric, when a
   vector index is created with each admitted metric, then it is created; and when it is created with each invalid
   definition of *Validations and messages*, then it fails without effect with its class.
2. **AC-VI2** — Given a table with a ready vector index, when a write inserts, updates the vector of or deletes rows and
   another does not change the vector, then, before the worker processes them, a search in a later transaction returns
   the new rows, does not return the deleted ones and sees the new vectors, with their exact distance, and the row whose
   vector did not change is not left pending.
3. **AC-VI3** — Given a table of 100,000 rows, when a vector index is created while another thread writes rows
   without stopping, then no batch of the worker holds the turn for more than 15 ms according to the test hook that
   measures it, the index goes through training, building and ready, and when the writes end and the pending log
   empties, each row has in the index the entry that corresponds to it according to the test hook that compares the index
   with the rows.
4. **AC-VI4** — Given an index in any state, when the process crashes in the middle of training, of construction or of a
   batch, and the store is opened again, then the worker continues, the index reaches ready and matches the rows, and no
   page is left unused or unfreed.
5. **AC-VI5** — Given a ready index, when searching without a filter with the number of lists equal to the total and the
   test hook that re-ranks all the candidates, then the result is that of the exact search (VI16); when searching with a
   smaller number of lists, then each returned row is visible, carries the same distance that the exact search gives it
   and the order is that of VI5. Recall is measured by AC-VI11.
6. **AC-VI6** — Given a table with a secondary index, when searching with a filter of each class of VI7, with one that
   lets fewer rows pass than the threshold and with one that lets more pass (the threshold, set with a test hook), then
   only rows that meet the filter come back; with few rows, the result equals that of the exact search over the rows
   that pass and it is reported exact; with many, it is reported approximate.
7. **AC-VI7** — Given an open read transaction, when another write changes rows and the worker processes them or
   rebuilds the index and publishes it, then the search in the open transaction returns the same as before those
   changes.
8. **AC-VI8** — Given a ready index, when a rebuild is requested, then searches keep answering with the previous
   generation until publication and afterwards with the new one, and the index information reports its counters before
   and after.
9. **AC-VI9** — Given a table with a vector index, when the table is emptied, bulk loaded, the column is renamed, its
   removal is attempted and the index or the table is dropped, then what VI10 fixes happens, and in a temporary store
   the same.
10. **AC-VI10** — Given the same store file with a ready index, closed and opened again on Linux, when the same
    approximate search and the same exact search are made, then the rows and the distances are bit-for-bit identical;
    so is the exact search over the same data loaded into another store; and the distance kernels give the bits a golden
    test pins (annotation of 2026-11-25: no Windows).
11. **AC-VI11** — When the thresholds of VI15 are measured, then each one meets its figure (recall@10 ≥ 0.95 with p95
    ≤ 20 ms; inserting with the index ≤ 1.10 times without it; a batch ≤ 15 ms) on Linux (annotation of 2026-11-25), and the
    closing report brings the comparison with pgvector.
12. **AC-VI12** — Given a search with the L2, dot product and cosine metrics, when their distances are compared with the
    reference ones of VI8 computed separately, then they are those; and two rows whose Euclidean distances coincide when
    rounded come out in the order of their squares.
13. **AC-VI13** — Given a filter with each condition of VI7 on a column that admits nulls, with null rows and with a
    null in the IN list, in the NOT IN list and in an equality, when searching, then exactly the rows that SQL gives
    pass; and given a test table where the rows that pass are far from the lists that are probed, when searching by the
    index, then k rows come back; and given a filter that k rows meet, one of them with a null vector, when searching,
    then k − 1 rows come back, and that row does not count for the threshold of VI6; and when all the rows of a table
    with a ready index are deleted one by one, then the information gives none in the indicators of VI11 that cannot be
    computed and there is no rebuild.
14. **AC-VI14** — Given an injected failure in a worker batch when reading and when storing, when searching, then the
    result is correct and the index is brought up to date afterwards; and given a store of format 15, when it is opened
    with this version, then it fails with the earlier format class (F11, B12); and given an injected input/output
    failure when committing a batch, then the store stops accepting writes and the next write of the application reports
    it (T12).
15. **AC-VI15** — Given a table with 9,999 rows with a vector and an index, then the index keeps training and the search
    is reported exact; with row 10,000, the index trains, and the index information gives the number of lists of VI4 for
    10,000, 1 M and 1,000,001 rows (the latter with the hook that counts the rows at training).
16. **AC-VI16** — Given a ready index, when the test hook lowers the thresholds of VI9 below its indicators, then the
    worker rebuilds on its own and publishes a new generation; when a rebuild is requested with another in progress, with
    the index training or with fewer than 10,000 rows, then nothing happens; and when the store is closed with a rebuild
    requested and opened, then it is done.
17. **AC-VI17** — Given an index in each state, when its information is requested, then it brings the fields of VI11
    with their values, also without a ready generation; and when the store is closed cleanly in each state and opened,
    then the index continues from where it was, and Describe lists it as VI1 fixes.

## Annotations to other PRDs

Written on 2026-11-25:

- [tables.prd.en.md](tables.prd.en.md):
  - the annotation of 7.6, *Content*: the search is not only approximate, it is the hybrid one of VI6, and there is a
    public exact search (VI16); "its benchmark loads (N11)" no longer holds: there are no new loads, and the measurement
    is that of VI15;
  - *Fixes*: what it fixes is fixed here; the store information (P12) does not change, and that of the index is an
    operation of its own (VI11);
  - N11: the comparison of VI15 is with pgvector, without a threshold, instead of loads against Voron;
  - *Out of scope*: "Filtering by a distance threshold" remains in module 17.
- [12-vector-indexes.md](../../../design/tables/12-vector-indexes.md): the three parts that this PRD changes (filters,
  rebuilds and criteria for switching to HNSW) and its product questions, decided here.
- [tables-indexes.prd.en.md](tables-indexes.prd.en.md): the maximum of 64 indexes per table counts the vector ones
  (VI1), and removing a column with a vector index fails like with a secondary one (VI10).
- [tables-schema.prd.en.md](tables-schema.prd.en.md): emptying the table (E3), removing a column (E5) and the bulk load
  (E13) with a vector index follow VI10.
- [tables-statistics.prd.en.md](tables-statistics.prd.en.md): the conditions of VI7 add IN and NOT IN and treat nulls as
  SQL does, unlike ST2; the estimates of 7.3 do not change.
- [05-vectors.md](../../../design/tables/05-vectors.md): question 6 is decided by VI8.

## References

- Parent: [tables.prd.en.md](tables.prd.en.md). Root: [krummi.prd.en.md](../krummi.prd.en.md).
- Siblings: [tables-schema.prd.en.md](tables-schema.prd.en.md) (7.1), [tables-indexes.prd.en.md](tables-indexes.prd.en.md)
  (7.2), [tables-statistics.prd.en.md](tables-statistics.prd.en.md) (7.3).
- Design: [document 12](../../../design/tables/12-vector-indexes.md), [document 5](../../../design/tables/05-vectors.md).
- Measurement: [ANN experiment](../../../benchmark/2026-11-24-ann-experiment.md).

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| 9df599dc | 2026-11-24 05:42 UTC+00:00 | 25 h 37 min | 16 h 39 min | 8 h 58 min | 7 h 48 min | 1 h 11 min | 25 |

<!-- prd-time: [{"session": "9df599dc-6ef7-4fc3-8883-fd027ed6e6d1", "start": "2026-11-24T05:42:24.309000+00:00", "wall": 92205, "discarded": 59922, "agent": 28051, "human": 4230, "interventions": 25, "idle_minutes": 10}] -->
