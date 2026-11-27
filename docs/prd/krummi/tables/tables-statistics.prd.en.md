# Table Statistics — PRD

**Version**: 1.0
**Date**: 2026-11-19
**Parent document**: [Krummi Tables — PRD](tables.prd.en.md) (v1.0)

> English translation of [tables-statistics.prd.md](tables-statistics.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Build child PRD **7.3** of the [split of the tables module](tables.prd.en.md#split-into-child-prds): the statistics of
the columns of each table, the modification counter that says when they are stale, the public **estimate** operation
that uses them, their construction in the background and the **analyze** operation, which builds them on request (N8 of
the parent), and their measurement (N11).

**Why now**: 7.1 and 7.2 are closed, and N2 of the parent builds the child PRDs in order. The query planner (module 17)
estimates with these statistics how many rows meet each condition, to choose between scanning the table or an index.

The technical design is already decided in [design document 3](../../../design/tables/03-keys-and-indexes.md)
(*Statistics*: what is stored per column, the persistent counter, the construction from a snapshot, the rule that some
statistics never replace newer ones and the order of the locks, with review rounds H7, H8 and H18), which is the
technical starting point of this PRD, with [document 4](../../../design/tables/04-query-tree-and-planner.md)
(how the planner uses them) and [document 10](../../../design/tables/10-agent-tooling.md) (C2 and C10). This PRD changes
a decision of document 3: without statistics, the estimate does not answer with a default estimate, but samples at that
moment (ST3).

Decisions of the user of 2026-11-19, on the proposals of the draft: reading the statistics is public, with the
«Lectura completa» (full read) (ST16); there is no option to turn off the background statistics («No, fuera de
alcance») (no, out of scope); without statistics the answer is not a guess: «si no espera al muestreo entonces tiene que
usar "suposiciones" (guesses) y es lo que no quiero» (if it does not wait for the sampling then it has to use "guesses"
and that is what I do not want), so the estimate samples (ST3); and with stale statistics, «La a» (option a): they are
used and refreshed in the background (ST2). For conditions on several columns, storing a sample of rows «me parece muy
costoso» (seems very costly to me); the statistics of several columns, those of the prefixes of the indexes in 7.3 and
the declared ones later («Vale»), are adopted, and, when there are none, a combination that prefers to overestimate
(«prefiero sobrestimación. Generalmente subestimar es peor que sobrestimar» (I prefer overestimation. Generally
underestimating is worse than overestimating), but «No es cuestión tampoco de ponerse en el peor caso min(s1, s2)» (it
is not a matter of taking the worst case min(s1, s2) either)), the exponential backoff «sesgado a sobrestimar» (biased
to overestimate) (ST18). Statistics of fewer than 500 rows are sampled again when the table has more than double («a»,
ST2).

This PRD inherits everything from its ancestors. It makes precise or extends rules of its parent, of
[tables-schema.prd.en.md](tables-schema.prd.en.md) (7.1) and of the transactions, which carry its annotation
(*Annotations to other PRDs*).

In this document, the **statistics** of a column are the summary of its values that is built by reading rows of a
snapshot; the **source snapshot** is that of the transaction they were built from; and the **modification counter** of a
table is the number of rows that its committed writes have inserted, changed or removed since it was created (ST4).

### Product decisions

| ID | Decision |
|---|---|
| ST1 | **What they are and which ones a transaction sees**: every column of every table, with an index or not, except vector columns, may have statistics: a summary of its values in its source snapshot (how many rows the table had, what fraction of the column was null, which values are the most frequent and how the others are distributed) with the modification counter of that snapshot (ST4). A table of up to 30,000 rows is read whole; a larger one, through a sample. The **large values**, those of `string` or `bytes` of more than 2,048 bytes (the maximum of a key, B5, so they are never in the key of an index), are not read: the sampling only counts that the row has one, and the statistics store their fraction, outside the frequent values and the histogram. A row stored outside its leaf, in a run of consecutive pages (B6), is sampled by reading only its first page, with the header, the fixed part and the offsets of the variable columns, and the pages of its columns that are not large, without verifying the checksum of the whole run, which would require reading all of it (exception to G6 of the root for sampling; user: «La a) sí»); bounds are checked, so a damaged run never ends in a panic, and an offset outside the run counts as a large value. A statistic built from a damaged run may be wrong, never the result of a query. To estimate equalities on large values precisely, the application may add a column with their hash (user: «Si quieres condiciones de igualdad mejor añadir una columna de hash» (if you want equality conditions, better add a hash column)). Vector columns have no statistics (document 3). In addition, every index with two or more columns in its key has statistics of the prefixes of its key (ST17); there are no other statistics of several columns together: those declared on columns without an index are left for a later PRD. Those of JSON columns are fixed by 7.5. The **stored** statistics are data of the table: each transaction sees the committed ones in its snapshot, or those it stored itself (ST7), as it sees the rows (G1). In addition, the store keeps in **memory** the latest ones that have been built of each column and of each index prefix (when sampling in an estimate, ST3; in the background, ST6; or when analyzing on the store, ST7), from when they are built until the store is closed, newer ones of the same column replace them, or the column, the index or the table cease to exist or the table is emptied, at the moment that change is committed (ST8); those of analyzing in a write do not go to memory. Of a column or a prefix, a transaction **sees** the one with the highest source counter among the stored ones it sees and those in memory, except the memory ones built before an emptying of the table that this transaction sees; each time it uses them it sees those of that moment, so two estimates of the same transaction may use different statistics if newer ones were published between them. |
| ST2 | **Estimate**: in a read or write transaction, on an open table, an estimate is requested of one of these conditions: **equal to a value** in a column, **in an interval** of a column, with a lower and an upper bound, each one included, excluded or absent, or **equal in several columns** (ST18); the columns are named by their name. The rules for the values are those of an interval of an index with a prefix of one or of zero columns (IX9), with one difference: an equality value of null asks for the rows with the column null also in a column that does not admit nulls, where it estimates zero, instead of failing with null not admitted; with some bound, the rows with the column null are not in the interval and, with no bound, they are; the values are normalized (N4, E7) and rounded (E8), so `-0` estimates `+0` and any NaN the stored NaN. Unlike IX9, the values have no size limit: any value valid for its column is accepted (E18), and no estimate fails with invalid size because of its values. The large values (ST1) count like this: an equality with a large value estimates the fraction of large values, which is an upper bound; an equality with a value that is not large does not count them; and an interval with some bound always adds that fraction, because their place in the order is not known. The estimate returns the **estimated rows**, a real number between zero and the number of rows of the table that the transaction sees (N6); whether any of the statistics it used is **stale** (ST5); and whether it is **joint**, that is, whether the same statistics cover all its columns, as always in a condition on one column, or whether it combines several (ST18). **It never answers with a guess**: if the transaction sees the statistics it needs, it uses them, stale or not, and if they are stale it also asks for them to be refreshed in the background (ST6), without waiting; if any is missing, or it sees some whose source rows are fewer than 500 and the transaction sees more than double the rows (also some of zero rows, when it sees any), it samples at that moment (ST3) and uses what it read. The estimated rows are the fraction given by the statistics times the number of rows the transaction sees, so a write that estimates sees its own rows in that number, although the statistics do not see its changes. If the transaction sees no rows, every estimate is zero, without sampling or asking for anything. After computing them, these **exact rules** always hold: an empty or inverted interval estimates zero; an interval without bounds estimates all the rows; in a column that does not admit nulls, equal to null estimates zero; and in a column that is by itself the primary key or the key of a unique index, equal to a non-null value estimates the smaller of what was computed and one row. A write whose own modifications make the statistics stale asks for their refresh, although this will not see those modifications until they are committed. Estimating is not a change for T10 nor does it end references (E16), and only reads pages of the rows or of the indexes when it samples. |
| ST3 | **Sampling in an estimate**: the estimate reads the sample in the thread of whoever requests it and waits to have it; it is the only operation of the application, besides analyze, that waits for a sampling (changes N8 of the parent). It reads the committed snapshot that the transaction sees, like a read transaction that blocks no one: its own in a read and, in a write, the one a new read would see, without its changes, which may be somewhat older than the one the write started from if an earlier commit is not yet durable (user: «a) ya que las estadísticas son estadísticas y no tienen que ser exactas ni estar siempre al día. Es el principio fundamental.») (the sampling does not ask for the write turn; only the later storing, a short write, asks for it, ST6; a write of the application that estimates keeps holding its own turn while it waits for the sample, as in any operation of its own, G4); it builds the statistics of all the columns of the table that are not vector and of the prefixes of its indexes (ST17); it publishes them in memory (ST1), with the discard rules of ST6, so that the other transactions use them; and it asks the thread of ST6 to store them, except in a store that does not accept writes (T12). If in that snapshot the table does not exist or is not the same, the write emptied it afterwards, or a column or an index that the estimate needs is missing (because the write created them), it reads what the write sees, and what it builds is only valid for that write: it is not published nor stored, and it is forgotten when the write ends. Two estimates that sample the same table at the same time do not wait for each other: each one samples and in memory the one with the highest source counter stays (with equal counters, either). A failure when reading the sample (damage, input/output, out of memory) makes the estimate fail with its class, without changing anything; in a write, T11 follows. |
| ST4 | **Modification counter**: each table has one, which starts at zero when it is created. It is data of the table like its rows: each operation adds in the view of its write, so it is committed with the rows it counts, an aborted write leaves nothing and each transaction reads that of the rows it sees (G1). These add: insert, one per row; update or insert or update, one per row inserted or changed; delete a row that was there, one; delete a range, the rows it deleted; empty the table (E3), the rows it removes; and a bulk load, the rows it inserts. Inserting with increasing keys adds the same. These do not add: an identical update (E10), updating without finding the row, deleting a row that was not there, every operation that fails, schema changes, creating or dropping an index, renaming the table, asking for a value of a sequence (E14) and analyze. It never decreases; it survives closing and reopening the store and a crash (G5); renaming the table keeps it; a table dropped and created again starts at zero. It is not read directly: the application sees it only as the source counter of some statistics (ST16) and through ST5. |
| ST5 | **Stale statistics**: the statistics of a column are stale for a transaction when the counter that transaction sees minus the counter of their source snapshot exceeds the threshold of N8, computed with the rows the table had in its source snapshot. If the difference is negative (some in memory built from a snapshot later than the transaction's), they are not stale. |
| ST6 | **Background work**: each store has at most one thread of its own for the statistics; it is created with the first request and ends when the store is closed. It does two kinds of work: **storing** those that an estimate sampled (ST3) and **refreshing** the stale ones (ST2), that is, sampling, for all the columns and prefixes of ST3, a new snapshot of a read transaction, which blocks no one (like every long read, it retains the pages it sees while it lasts), publishing what it builds in memory (ST1) and storing it. A request is for a table by its identity (E2), not by its name, so it follows the table if it is renamed. The requests wait in a queue, at most one of each kind pending per table besides the one being served, and are served in order of arrival. Storing is a short write of its own that asks for its turn like any write (G4, T4), with no time limit, at the end of the queue; it changes neither rows nor the counter, and a write of the application may wait for it and fail by its limit (N8). A refresh request whose table does not exist in the snapshot it samples is discarded. **Discard**: when publishing and when storing, the statistics of a column or a prefix are discarded if the column or the index no longer exist, if the table is no longer the same (it was dropped, also if it was created again with the same name), if the table was emptied after their source snapshot, or if those already there (in memory or stored) have a higher source counter (N8: they never replace newer ones); with equal counters, the two describe the same rows and either may remain. No sampling waits for another. In a store that does not accept writes (T12) no work is requested. Storing statistics takes up pages like any write, so it may leave a later write of the application without space (G7, P10). |
| ST7 | **Analyze**: it builds in the thread of whoever requests it, like a background construction, the statistics of a table or of one of its columns, and returns when they are stored or discarded by ST6; it returns nothing, also when it discards them. There are two forms. **Analyze on the store**, outside any transaction, receives the name of the table, optionally that of a column and optionally a time limit: it builds from a new snapshot, publishes it in memory (ST1) and then stores it with its own short write, which asks for its turn like any write (T4); the limit counts only that wait for the turn, which begins when the sampling ends, so with a limit of zero and another write open it fails after sampling. A thread that has a write open on that store gets misuse at once, without sampling (T5). If it fails after sampling (when waiting for the turn or when storing), what was published stays in memory, as in ST9: the store does not change and the next estimate that uses those statistics asks for them to be stored (user: «Se queda en memoria» (it stays in memory)). It reports its own failures with their class: those of reading the sample (damage, input/output, out of memory) and those of storing (disk full of G7, maximum size of P10, out of memory of P13, input/output, which follows T12 and of which it is the operation that reports it if it is the first). **Analyze in a write**, on a table open in it, with an optional column: it builds from what the write sees, its changes included, with the counter of its view, and stores inside it, without asking for another turn; its failures follow those of any operation of a write (T11); if the write is aborted, the statistics stay as before, and if it changes more rows afterwards and commits, those modifications count for ST5. In a read transaction the operation does not exist. Analyzing a table without rows stores statistics of zero rows. Analyzing in a write is a change for T10 when it stores, and it ends the earlier references even if it fails (E16). |
| ST8 | **Statistics and table changes**: renaming a column keeps them (they are identified by the identity of the column, E2); allowing nulls in a column keeps them; an added column has none until they are built; removing a column removes its own in that write; emptying the table, with Empty or with Create or empty, removes all of them (E3) and adds to the counter the rows it removes (ST4); renaming the table keeps them with its counter; dropping the table, also when overwriting it when renaming another, deletes them with it. Creating or dropping an index does not change those of the columns. A new index has no statistics of its prefixes until they are built, and an estimate that needs them samples (ST3); dropping an index removes its own in that write; emptying the table removes them too. |
| ST9 | **Failures of the background work**: a failure when sampling to refresh (damage, input/output when reading, out of memory) discards that refresh without informing anyone; the stale statistics keep being used and the next estimate that sees them stale requests it again. A failure when storing (disk full of G7, maximum size of P10, out of memory of P13) leaves the store as it was and the statistics in memory, which keep being used until closing; the next estimate that uses some that are only in memory requests again that they be stored. An input/output failure when committing the storing follows T12: the store stops accepting writes; no operation of the application reports it when it happens, but the following write requests, which fail at once with the input/output class, and the close, as T12 says; the statistics in memory keep being used, and the estimates keep sampling when they see none (ST3). |
| ST10 | **Closing the store**: closing stops the sampling in progress of the thread without publishing or storing it, empties the queue, discards a storing that is still waiting for its turn and waits for the thread to end; if its write already has the turn, it waits for it to be committed. Ceasing to use the store without closing it does the same. What was in memory without being stored is lost, and no request is remembered when reopening: the first estimate that sees no stored statistics samples again. |
| ST11 | **Temporary mode**: the statistics, the counter, the estimate, the background construction and analyze work the same in a temporary store (G9), in memory. |
| ST12 | **Errors** (extends E17): a table or column name that is not 1 to 1,024 bytes long fails with **invalid size** (B8); estimating or analyzing a column that the table does not have fails with **different schema**, with the table and the column, like traversing an index that does not exist (IX9); estimating or analyzing a vector column fails with **invalid schema**, with the table, the column and the rule vector column (like IX3); reading the statistics of an index that the table does not have fails with **different schema**, with the table and the index; a value of an estimate that breaks IX9 fails with **invalid value**, with the table, the column and the rule: type different from that of the column, outside the domain or invalid range (a null as a bound); in the equality on several columns, an empty list fails with invalid value and the rule invalid range, and a repeated column, with the rule repeated column, before the values; with several values that fail, the first (in a condition on one column, the equality value, the lower bound and the upper; on several columns, in the order of the list), with the first rule it breaks in the order of E17, like IX9. Analyzing on the store a table that does not exist fails with tree not found, and a name of another kind of object, with different type (E2). The precedence is that of E17: first misuse (T5); then the invalid size of the names; then tree not found of an opening of a dropped table; then the failures reading the catalog, tree not found and different type; then different schema and, after it, invalid schema of the column; then invalid value; then the failures reading the sample; and, when analyzing on the store, last the timeout, the input/output of T12 when asking for the turn and the failures when storing. |
| ST13 | **Format**: the store stores the counter and the statistics of each table, which an engine of format 12 does not know how to read, so the change is incompatible (B12) and the format version becomes 13: a store of format 12 is of an earlier format and this engine does not open it (F11). The information of the store (P12) does not change its form: the pages of the statistics count in its totals. |
| ST14 | **Test hooks** (those that 7.3 fixes for AC-N8): one stops a sampling of an estimate after reading and before publishing; another stops a sampling of the thread or of analyze on the store after reading and before publishing; another stops a storing after obtaining the turn and before committing; another reports whether the thread of ST6 exists, how many requests of each kind there are in its queue, without counting the one it is serving, and how many samplings have been made in the store; another waits for the thread to have no work; another counts the pages a sampling reads; and another sets the seed with which the sample of a table of more than 30,000 rows is chosen. They are test mechanisms, not public contracts. |
| ST15 | **Measurement** (N11): besides the single-run measurements of W1–W32 before and after the change, which measure what carrying the counter costs, the closing report measures, on Windows and on Linux: how long the first estimate takes on the table of 1,000,000 rows of W23, which samples (ST3); and W23 and W28 with a second thread that, after each write of 1,000 rows, requests an estimate of each column, so that it is sampled the first time and refreshed in the background each time they are stale, against the same loads without that thread. There are no new loads of the benchmark nor new thresholds: Voron has no statistics. |
| ST16 | **Reading the statistics** (C2 of design document 10): in a read or write transaction, on an open table, the statistics of a column are requested by its name. It returns that there are none, if the transaction sees no statistics of that column (ST1), or, of those it sees, in a copy owned by the application: the **source rows**, the number of rows of the table in its source snapshot; the **rows read** to build them (all, up to 30,000; ST1); the **source counter** (ST4); whether they are **stale** for the transaction (ST5); the **null fraction**, a real number from 0 to 1; the **fraction of large values** (ST1), zero in columns that are neither `string` nor `bytes`; the **distinct**, the estimated number of distinct non-null values, a non-negative real number; the **frequent values**, a list, possibly empty, of pairs of a non-null value and its fraction of the rows, from highest to lowest fraction and, with equal fraction, in the order of N5 of the type of the column; and the **histogram**, a list, possibly empty, of buckets over the non-null values that are not among the frequent ones, in the order of N5, each with its **upper bound** (the greatest value of the bucket), its fraction of the rows and whether the bound is **shortened**. The fractions refer to the source rows, and the null fraction, the fraction of large values, those of the frequent values and those of the buckets add up to 1, up to rounding, in a table with rows; in one of zero rows, everything is zero and the lists are empty. The values have the normalization of N4 and E7 (a `uuid` in the RFC 9562 byte order also in a column with the SQL Server order) and are decoded in the copy, the text and the bytes too (like the keys of IX8). A text or bytes bound may be shortened to a prefix of it, the text at a character boundary; then the bound that is returned is that prefix and it is marked as shortened. Reading the statistics neither samples nor requests any work, does not read pages of the rows or of the indexes, is not a change for T10 and does not end references (E16). In the same way the statistics of an index are requested by its name: it returns that there are none, if the transaction sees those of none of its prefixes or the index has a single column in its key, or, for each prefix from 2 columns up to all those of the key, in that order: the number of columns of the prefix, the source rows, the rows read, the source counter, whether they are stale, the **distinct** (the estimated number of distinct combinations of values, counting null as one more value) and the **frequent combinations**, a list, possibly empty, of pairs of a combination of values (any may be null), in the order of the columns of the prefix, and its fraction of the rows, from highest to lowest fraction and, with equal fraction, in the order of the index (N5, with its direction and its nulls). Its errors are those of estimating (ST12), without those of the values. |
| ST17 | **Statistics of the prefixes of an index**: every index with two or more columns in its key (IX1) has, for each prefix of its key from 2 columns up to all, statistics with the estimated number of distinct combinations of the values of those columns (null counts as one more value) and the most frequent combinations with their fraction of the rows, from the same source snapshot and with the same counter as those of the columns (ST1, ST5). They are built with the same sample, in the same sampling, without reading the index; they are identified by the identity of the index (IX6) and its number of columns, so renaming one of its columns keeps them. They serve for the equality on several columns (ST18) and are read with ST16. There are no statistics of several columns for columns that do not form the prefix of an index: the declared ones, like `CREATE STATISTICS` of PostgreSQL, are left for a later PRD (user: «Vale» (OK), to leave them for later). |
| ST18 | **Equality on several columns**: the condition is a list of pairs of a column and a value, with distinct columns, from one to all the columns that are not vector; each value follows the equality rules of ST2, null too. The estimated rows are computed like this: (1) **groups**: while columns of the condition remain uncovered, among the index prefixes (ST17) whose columns are all in the condition and none already covered, the one with the most columns is taken and, with an equal number, that of the index that comes first in the order of their names according to B5; each column that no prefix covers is a group of one column; (2) **fraction of each group**: the one its statistics give for the values of its columns, as an equality of ST2; (3) **combination**, biased to overestimate (user: «sesgado a sobrestimar»): with the fractions of the groups sorted from smallest to largest, f₁ ≤ f₂ ≤ f₃ ≤ f₄, f₁ × f₂^(1/4) × f₃^(1/16) × f₄^(1/64) is taken, without the groups beyond the fourth, which could only reduce it; it lies between the result of assuming the columns independent (the product) and that of assuming them totally correlated (the smallest fraction); with a single group, it is its fraction and the estimate is **joint**; (4) it is multiplied by the number of rows the transaction sees. Then the exact rules of ST2 hold, and in addition: a null in a column that does not admit nulls estimates zero; and if the pairs give non-null values to all the columns of the primary key, or to all those of the key of a unique index, at most one row. If any of the statistics of the groups is missing, the estimate samples (ST3); the statistics of a prefix that cannot be built because its index was created in the write that estimates are built as ST3 says. With a single column, it is the equality of ST2. The exponents are the exponential backoff of SQL Server (s₁ × s₂^½ × s₃^¼ × s₄^⅛) with each exponent squared, so that the combination stays closer to the smallest fraction. |

*Annotations of 2026-11-21 from the JSON columns ([tables-json.prd.md](tables-json.prd.md), 7.5, Spanish,
authoritative):*

- *ST1 and Out of scope: JSON columns have no statistics, like vector columns, and sampling does not read their
  documents (JS10).*
- *ST12: estimating, reading or analyzing a JSON column fails with invalid schema and the rule JSON column, in the
  place of the rule vector column (JS10).*
- *ST18: "the columns that are not vector" also excludes JSON columns (JS10).*

*Annotations of 2026-11-22 from the temporal values in microseconds ([tables-temporal-micros.prd.md](tables-temporal-micros.prd.md), Spanish, authoritative):*

- *ST2: a temporal value of a condition is already in microseconds and is not rounded; the temporal values of the
  statistics are returned in microseconds (TU1, TU3).*

*Annotations of 2026-11-23 from the collation of text columns ([tables-collation.prd.md](tables-collation.prd.md), 7.7, Spanish,
authoritative):*

- *ST1, ST2, ST16, ST17 and ST18: the statistics and estimates of a `ci` or `ci_ai` column use folded forms; a text
  whose folded form exceeds 2,048 bytes is also large, and a bound is shortened at a scalar value boundary of the
  folded form (IC7, IC8).*

*Annotation of 2026-11-25 from the approximate vector indexes ([tables-vector-indexes.prd.md](tables-vector-indexes.prd.md), 7.6, Spanish,
authoritative):*
*the filter of a neighbor search starts from the conditions of ST2, adds IN and NOT IN and treats nulls as SQL does,
unlike ST2 (VI7); the estimates of 7.3 do not change.*

### Starting state

Verified on 2026-11-19 in the code and in the design documents:

- There are no statistics, nor modification counter, nor estimate or analyze operations. The entry of a table in the
  catalog stores its identity, the root and the number of rows of its row tree, its schema history, the version and the
  state of the schema, its columns, its primary key, the state of the indexes and its indexes, with no fields reserved
  for statistics; its read rejects extra bytes.
- Tables and columns have random identities that are kept when renaming; dropping a table retires its identity, so
  creating it again gives it another.
- The writes that change the number of rows go through a single point, except the fast path of appending at the end and
  emptying the table, which sets it to zero directly;
  update reports whether it did not find the row, an identical update writes nothing and delete a range reports how
  many rows it deleted.
- The store has two threads of its own, that of the journal and the one that carries the pages to the data file;
  closing the store waits for them to end. No internal write asks for the write turn.
- The trees have no operation to estimate an interval nor to count between two keys.
- The information of the store counts pages, not trees nor tables. The format version is 12.

## Actors

- **Application developer (Rust)**: requests estimates of the columns of their tables and, when needed, analyzes a
  table or a column.
- **Query planner (module 17)**: will use the same estimate (N8 of the parent).
- **Maintainer**: runs the measurements of ST15 on Windows and on Linux.

## Flow

1. The application creates a table and stores rows (7.1); each write adds to the counter (ST4).
2. It requests an estimate of a column (ST2); the first time, without statistics, the estimate samples the table,
   answers with what it read and leaves it in memory for the other transactions (ST3).
3. The thread of the store stores them with a short write (ST6).
4. The following estimates use them; when the modifications exceed the threshold (ST5), they keep using them, say
   they are stale and ask the thread to refresh them.
5. If it wants up-to-date statistics without waiting for them to become stale, the application analyzes the table (ST7).

## Validations and messages

Those of the ancestors apply. This PRD adds, only in the library, in the order of their precedence (ST12):

| Situation | Error class |
|---|---|
| Analyzing on the store from a thread that has a write open on it | Misuse (T5) |
| Analyzing on the store with a table name, or estimating, reading or analyzing with a column or index name, that is not 1 to 1,024 bytes long | Invalid size (B7, B8) |
| Estimating or analyzing through an opening of a table that the write dropped | Tree not found (A6, E17) |
| Analyzing on the store a table that does not exist, or a name of another kind of object | Tree not found; different type (E2) |
| Estimating, reading or analyzing a column that the table does not have, or reading the statistics of an index it does not have | Different schema, with the table and the column or the index |
| Estimating, reading or analyzing a vector column | Invalid schema, rule vector column |
| An equality on several columns that is empty or has a repeated column; an estimate with a value of another type, outside the domain or with a null as a bound | Invalid value (E17) |
| A failure reading the sample when estimating or analyzing | Damage, input/output or out of memory, with T11 in a write |
| Analyzing on the store with a time limit that expires before the turn | Timeout (T4) |
| Analyzing on the store when the store does not accept writes | Input/output (T12) |
| A failure when storing while analyzing | Disk full (G7), maximum size (P10), out of memory (P13) or input/output (T12) |

An estimate without statistics (it samples), with stale statistics, on an empty table or with an empty interval is not
an error.

## Output

The module writes nothing on screen. Estimating returns the estimated rows, a real number, whether any of the statistics
it used is stale and whether it is joint (ST2, ST18). Reading the statistics of a column or an index returns that there
are none or the fields of ST16, in a copy owned by the application. Analyze returns nothing. The reads, traversals,
writes and the information of the store do not change their output.

## Reuse

- The rules for the values of an interval of an index (IX9) for the conditions of an estimate.
- The write turn and its queue (G4, T4) for the background storing and analyze on the store.
- The identities of the tables and columns (E2) to discard constructions of a table or a column that no longer exist.
- The number of rows that an opening reports (N6, E11).
- The single-run measurements and the loads W23 and W28 of the benchmark (module 6, 7.1).

## Out of scope

- Reading the current modification counter of a table (ST4).
- Declared statistics of several columns without an index (`CREATE STATISTICS` of PostgreSQL): a later PRD (ST17).
- Estimating an interval by descending the tree (design document 3).
- Estimating conditions that mix equalities with an interval, several intervals, not equal to, `is not null` or `OR`:
  the planner of module 17 combines them, with the rule of ST18 for the conjunctions.
- Adjusting the estimates with what the queries return, like *CE feedback* of SQL Server 2022: module 17.
- Statistics of vector columns.
- An option to turn off the background statistics (question 7 of design document 10): the tooling module adds it if it
  needs it.
- Configuring the size of the sample, the number of buckets or of frequent values, or the threshold of ST5.
- Exporting and importing statistics.
- Default estimates or guesses: without statistics, the estimate samples (ST3; user: «si no espera al muestreo entonces
  tiene que usar "suposiciones" (guesses) y es lo que no quiero»).
- Sampling again in the estimate when the statistics are stale: they are used and refreshed in the background, like the
  asynchronous update mode of SQL Server (ST2).
- Remembering between openings of the store the pending requests or the statistics in memory that were not stored.
- Building statistics without an estimate or analyze asking for them: when opening the store, after a bulk load or
  periodically.
- The statistics of JSON columns: fixed by 7.5.
- Copying, compacting or verifying the statistics and the counter: modules 10, 11 and 12 treat them as trees (N1).
- Canceling the sampling of an estimate or giving it a time limit of its own (ST3).
- Reading or building statistics from the command-line tool (C2 and C10 of design document 10): fixed by the module of
  that tool.
- A promised precision for the estimates of a sampled table, beyond AC-ST1.
- Exact counts of an interval (parent).

## Left to the implementation

- How the statistics and the counter are stored in the catalog and in trees, with the starting point of document 3.
- The size of the sample of a table of more than 30,000 rows and how it is chosen, the number of buckets, the number of
  frequent values, the length, of fewer than 1,000 bytes, to which text and bytes bounds are shortened, how many
  frequent combinations a prefix stores, and how the fraction of each estimate and of each group is computed with the
  statistics, as long as they meet ST1, ST2, ST17, ST18 and the acceptance criteria.
- The names of the operations, the types and the error classes, in English and with the conventions of Rust (D8).
- How the test hooks of the criteria are built: they are test mechanisms, not public contracts.

## Acceptance criteria

Those of the ancestors apply. This PRD meets AC-N8 of the parent and AC-N10 with what these criteria fix.

1. **AC-ST1** — Given a table of 10,000 rows, generated with a fixed seed, with a column of each type of N4 that can
   be sorted, each one (except `bool`) with a value in 40 % of the rows, 10 % nulls and the rest with at least 100
   distinct values distributed uniformly, and a `bool` column with 40 % true, 50 % false and 10 % null, analyzed, when
   the following are estimated: equal to the frequent value (in `bool`, true and false), equal to null, equal to a value
   that is not in the table (except in `bool`), and intervals whose bounds are values of the table chosen with the seed,
   with each combination of included, excluded and absent bounds, then they are not stale, they are joint, equal to the
   frequent value and equal to null give their rows with a relative difference of at most 10⁻⁹ (the table was read
   whole, ST1), and the value that is not there and each interval give their actual rows with an error of at most 5 % of
   the rows of the table; `-0` and NaN estimate the normalized values; and the exact rules of ST2 give what they say,
   with the same difference. Given the same distribution in a table of 200,000 rows, sampled with the seed of the test
   hook (ST14), equal to the frequent value and equal to null give their actual rows with the same error of at most 5 %.
2. **AC-ST2** — Given a table of 10,000 rows like that of AC-ST1, without statistics and with the storing stopped at
   its hook (ST14), when a read estimates each condition of AC-ST1, then the first estimate samples (the hook counts
   one sampling), and all give the values of AC-ST1 and are not stale; when another read, and a write, estimate
   afterwards, then they use those in memory without sampling; when the storing is released and the store is closed and
   reopened, then the first estimate uses the stored ones without sampling; when it is closed with the storing still in
   the queue and reopened, then the first estimate samples again (ST10); when two reads estimate at the same time with
   the sampling of the estimates stopped at its hook, then neither waits for the other and both answer when it is
   released; an empty table estimates zero without sampling; and a table analyzed without rows, to which 400 are added,
   samples again in the first estimate; one analyzed with 100 rows samples again with 201 and not with 200, and one
   analyzed with 500 does not sample again with 1,001; a read older than some statistics in memory uses them and they
   are not stale (ST5); and a write that inserts 1,000 rows and estimates uses those rows in the number of rows (AC-N8,
   changed: there is no default estimate).
3. **AC-ST3** — Given an analyzed table, when each operation of ST4 is committed in a write (insert with keys in order
   and at random, a bulk load in order and out of order, and delete a range), and in another it is aborted, then the
   counter, observed through ST5, adds exactly what ST4 says: with modifications up to the threshold the statistics are
   not stale and with one more they are, in a table of 400 rows (threshold 500), of 10,000 (2,500) and of 100,000
   (10,000); the operations that do not add do not make them stale; emptying an analyzed table of 400 rows and analyzing
   it again gives a source counter 400 higher than the previous one, read with ST16; a bulk load that fails, whole
   according to E13, adds nothing; and when a bulk load fails while a background refresh is stopped after sampling, then
   the statistics that are stored count no row of the load (AC-N8).
4. **AC-ST4** — Given an analyzed table, when modifications are made below the threshold, the store is closed and
   reopened, also after simulating a process crash, and those that are missing to exceed it are made, then the next
   estimate uses the statistics, says they are stale, does not sample in the estimate and asks for a refresh, which the
   hook counts in the queue; when the hook says the thread has no work, then the next estimate says they are not stale
   and gives the values of the new rows; and when a table with more rows than the threshold is emptied and rows are
   added to it, then its statistics do not exist, also those in memory, and the next estimate samples (AC-N8).
5. **AC-ST5** — Given a background refresh stopped at its hook after sampling, and a write committed afterwards, when
   the table is analyzed on the store and ends, the construction is released and ends afterwards, and the other way
   round, then the one from the newer snapshot stays, with neither waiting for the other; when a write changes rows and
   analyzes while a construction of an earlier snapshot is pending, then there is no deadlock, it commits, and the
   statistics that remain are those of the write; when a write changes rows, analyzes and aborts, then the rows, the
   counter and the statistics stay as before; and when it changes more rows after analyzing and commits, then those
   modifications count for ST5 (AC-N8).
6. **AC-ST6** — Given a refresh stopped after sampling, and on the other side a sampling of an estimate stopped at its
   hook, when the column is renamed, then they are published and stored, and are read with the new name; when the column
   is removed, then they are neither published nor stored for it; when the table is dropped and created again with the
   same name, then they are not stored for the new one; when the table is emptied, then they are not stored; when the
   table is renamed, then they are stored for it with its new name; and in each case those of the other columns follow
   ST6. Renaming the table and creating or dropping an index keep them; an added column has none; and an estimate in a
   write on a table created in it samples what the write sees, publishes nothing in memory nor asks for it to be
   stored, and another estimate of the same write does not sample again (ST3, ST6, ST8).
7. **AC-ST7** — Given the storing of a construction stopped at its hook holding the write turn, when a write of the
   application asks for its turn with a time limit, then it fails with timeout when it expires (T4), and with a limit of
   zero it fails at once; when the store is closed with a refresh stopped after sampling and another request in the
   queue, then the close ends without storing either, and the hook says the thread no longer exists; with the storing
   stopped, the close waits for it to be committed and, when reopening, the statistics are there; and analyzing on the
   store with a limit of zero while another write is open fails with timeout after sampling (AC-N8, ST7, ST10).
8. **AC-ST8** — Given a simulated input/output failure when committing a background storing, then the next write
   request fails at once with the input/output class, the estimates keep using those in memory, an estimate on another
   table without statistics samples without asking for it to be stored, and the close follows T12; given damage in a
   page of the table, a full disk and out of memory simulated during a refresh, with the failure simulators of the
   ancestors, then it is discarded without a visible error, the stale ones keep being used, the store keeps accepting
   writes and the next estimate requests it again; given a full disk when storing, those in memory keep being used and
   the next estimate asks to store them; given the same failures when reading the sample in an estimate, then the
   estimate fails with its class, and in a write with damage, it can no longer be committed (T11); and the same failures
   when analyzing on the store are reported by analyze with its class, and the input/output one when storing leaves the
   store not accepting writes (ST3, ST7, ST9).
9. **AC-ST9** — Given each case of *Validations and messages*, when estimating, reading or analyzing, then it fails with
   its class and its data without changing the store (an analyze on the store that fails after sampling leaves what was
   published in memory, ST7, and a later estimate uses it without sampling), and with two failures at once the one that
   ST12 says wins; analyzing in a read transaction does not exist in the API (it does not compile); estimating is not a
   change for T10 nor does it end references; analyzing in a write is a change and ends references, and one that fails
   ends them without being a change (ST7).
10. **AC-ST10** — Given a temporary store, when AC-ST2, AC-ST4 and AC-ST5 are repeated, then they give the same, except
    what depends on reopening, and no file or folder is created (ST11).
11. **AC-ST11** — Given a store of format 12 written by 7.2, when this engine opens it, then it fails with the format
    version (B12, F11); a new store has version 13; and the information of the store does not change its form (ST13).
12. **AC-ST12** — Given the table of 10,000 rows of AC-ST1, analyzed, when the statistics of each column are read, then
    the source rows and the rows read are 10,000, the source counter is that of the table when analyzing, they are not
    stale, the null fraction is 0.1 and, except in `bool`, the first frequent value is that of 40 % with 0.4, the buckets
    go in the order of N5 and the fractions add up to 1 with a difference of at most 10⁻⁹, and the null and
    frequent-value fractions have that same difference; with the table of 200,000 rows, the rows read are fewer than the
    source rows; a `string` column with values of 1,000 bytes has bounds marked as shortened that are prefixes of values
    of the table; a `bytes` column with 20 % of values of 2,049 bytes and the rest of 2,048 has a fraction of large
    values of 0.2, an equality with a value of 4,096 bytes estimates 20 % of the rows without failing, one with a value
    of 2,048 bytes does not count them, and an interval adds them; the test hook that counts the pages read shows that
    the sampling does not read the pages of those values; the `bool` column has false with 0.5 as its first frequent
    value; after exceeding the threshold, they are stale; a column without statistics, an added one and an empty table
    not analyzed give that there are none, without the hook counting any sampling or any request; a table analyzed
    without rows gives all zero; and reading in a write is not a change for T10 nor does it end references (ST16).
13. **AC-ST13** — Given a table of 10,000 rows, generated with a fixed seed, with columns `pais` (20 values), `ciudad`
    (200 values, each of a single country, so `ciudad` determines `pais`) and `n` (100 values, independent of the other
    two), and an index on (`pais`, `ciudad`, `n`), analyzed, when equal is estimated in (`ciudad`, `pais`) with a city
    and its country, then it is joint and gives its actual rows with a relative difference of at most 10⁻⁹ (a prefix
    covers both columns, in any order); when equal is estimated in (`ciudad`, `pais`, `n`), then it is joint; when the
    index is dropped and the same conditions are estimated, then they are not joint and give f₁ × f₂^(1/4), and with
    three columns f₁ × f₂^(1/4) × f₃^(1/16), with the fractions of each column sorted, with the same difference; with
    five columns, the fifth group does not count; with two indexes whose prefixes cover the same columns, the one with
    the most columns is used and, if equal, the first by its name; a null in a column that does not admit nulls
    estimates zero; all the columns of the primary key, or of a unique index, with non-null values estimate at most one
    row; when a new index is created and an estimate is made by its columns, then the estimate samples (the hook counts
    one sampling); renaming one of its columns keeps its statistics, and dropping it or emptying the table removes them;
    reading the statistics of the index returns the two prefixes with their distinct and their frequent combinations in
    the order of ST16; and an empty list or one with a repeated column fails with invalid value and its rule (ST17,
    ST18).
14. **AC-ST14** — Given a write that adds a column, creates an index or empties the table and then estimates by them,
    then it samples what the write sees, publishes nothing in memory nor asks to store, and another transaction does not
    see those statistics; and given a write that estimates with statistics obtained in that same estimate and whose own
    modifications exceed the threshold, then it says they are stale and asks for a refresh (ST2, ST3).
15. **AC-ST15** — Given the closing of this PRD, when its report is reviewed, then it includes the single-run
    measurements of W1–W32 before and after the change and those of ST15, on Windows and on Linux (AC-N10).

## Annotations to other PRDs

This PRD changes or makes precise rules of earlier documents, which carry these annotations of 2026-11-19 (in their
translations too):

- [krummi.prd.en.md](../krummi.prd.en.md): G6 (exception for sampling: a row stored in a run is read by pages
  without verifying the checksum of the whole run, ST1; annotation of 2026-11-19, before the execution).
- [krummi.prd.en.md](../krummi.prd.en.md): G3 and AC6 (exception for the statistics: those in memory are shared, so a
  read may use some from a snapshot later than its own and two estimates of the same transaction may use different
  statistics, ST1; the rows, the counter and the stored statistics still follow G3).
- [tables.prd.en.md](tables.prd.en.md): N8 (changes: there is no default estimate; without statistics, the estimate
  samples at that moment and waits for the sample, so "No other operation of the application waits for them to be
  computed" no longer holds for the estimate; with stale statistics it uses them and asks to refresh them in the
  background, ST2, ST3; reading the statistics is also a public operation, ST16; analyze on the store and in a write,
  ST7; what adds to the counter, ST4); *Flow*, step 5 (the estimate samples when they are missing and refreshes them in
  the background when they are stale); AC-N8 (changes: "it answers the default estimation without waiting" and "it
  answers the default estimation and they are built again" become the cases of AC-ST2 and AC-ST4; AC-ST2 to AC-ST8 meet
  it); row 7.3 of the split (what it fixes is fixed here).
- [tables-schema.prd.en.md](tables-schema.prd.en.md): E3 ("resets its statistics" means that it removes them, ST8); E16
  (analyze in a write is a change and ends references; estimating is not, ST2, ST7); E17 (the errors of ST12); E20
  (the format version becomes 13, ST13).
- [tables.prd.en.md](tables.prd.en.md), *Out of scope*: "Statistics of several columns together" now excludes only
  those declared on columns without an index; those of the index prefixes enter 7.3 (ST17, ST18).
- [transactions.prd.en.md](../transactions/transactions.prd.en.md): T4 (the internal write that stores statistics waits
  for its turn like any write and without a limit, ST6); T12 (an input/output failure of that write is reported by no
  operation of the application when it happens, but by the following write requests and the close, ST9).

And the design documents: [3](../../../design/tables/03-keys-and-indexes.md), *Statistics* ("No query waits for
sampling" and the default estimate no longer hold for the estimate, which samples when there are no statistics, ST3; the
statistics of an index also store, for each prefix, their frequent combinations, ST17; estimating by descending the tree
is out of 7.3); [4](../../../design/tables/04-query-tree-and-planner.md) (a conjunction of equalities is not combined
with `sel1 * sel2`, but with the biased backoff of ST18); and [10](../../../design/tables/10-agent-tooling.md), C2
(reading the statistics is public from 7.3, ST16).

## References

- Parent: [tables.prd.en.md](tables.prd.en.md) (N1, N2, N6, N8, N10, N11, AC-N8, AC-N10); its parent,
  [krummi.prd.en.md](../krummi.prd.en.md) (D8, G1, G4, G5, G9, G12).
- Siblings: [tables-schema.prd.en.md](tables-schema.prd.en.md) (E2, E3, E7, E8, E10, E13, E16, E17, E20, *Benchmark
  loads*), [tables-indexes.prd.en.md](tables-indexes.prd.en.md) (IX3, IX9).
- Trees: [trees.prd.en.md](../trees/trees.prd.en.md) (B7, B8, B12), [trees-keys.prd.en.md](../trees/trees-keys.prd.en.md)
  (A6).
- Transactions: [transactions.prd.en.md](../transactions/transactions.prd.en.md) (T4, T5, T10, T12).
- Pager: [pager.prd.en.md](../pager/pager.prd.en.md) (F11, P10, P12, P13).
- Design: `docs/design/tables/` (documents 3, 4 and 10; README, H7, H8, H18).

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| 32ed042c | 2026-11-19 09:08 UTC+00:00 | 1 h 32 min | 0 min | 1 h 32 min | 44 min | 48 min | 17 |

<!-- prd-time: [{"session": "32ed042c-b929-4114-ac1f-a407366961e6", "start": "2026-11-19T09:08:25.908000+00:00", "wall": 5495, "discarded": 0, "agent": 2641, "human": 2853, "interventions": 17, "idle_minutes": 10}] -->
