# Krummi Tables — PRD

**Version**: 1.0
**Date**: 2026-11-09
**Parent document**: [Krummi, an embedded storage engine inspired by Voron — PRD](krummi.prd.en.md) (v1.1)

> English translation of [tables.prd.md](tables.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Build the **tables**, module 7 of the [parent's split](krummi.prd.en.md#split-into-child-prds): named collections of
rows with a schema of typed columns, a primary key and secondary indexes, on top of the B+ trees of module 5. It is
the first structured way of storing data in Krummi: until today, an application stores bytes in trees and decides
itself how to encode and order them. Queries (module 17) and the C interface with the C# binding (module 18) rely on
these tables, and the parked document product ([documents.prd.md](../documents/documents.prd.md)) would store its
collections in them.

The module is too large for a single execution, so this document is its umbrella, as the one of the
[trees](trees.prd.en.md) is for module 5: it sets the rules common to all tables and splits the module into child
PRDs, each with its own execution and its own closing (see [Split into child PRDs](#split-into-child-prds)). This
document has no execution of its own.

**Why this way**: the journal had to change its format after being built, for performance (O8 of the
[performance plan](../../performance/plan.md)). To avoid repeating that, the technical design was written before this
PRD: the [table design documents](../../design/tables/README.md) compare Voron with other structured storage systems
on each topic and record, in the user's own words, each decision of the user. This PRD takes from them what is
observable; the how stays in those documents, which are the technical starting state of each child PRD.

**Why now**: the trees (module 5) and the comparative benchmark (module 6) are closed, and Krummi meets the 22
benchmark thresholds on Windows and on Linux (full runs of 2026-11-09).

This PRD inherits everything from the parent and from the trees (B1–B12) and their children; here only what is
common to the module is added. What changes or refines earlier rules is declared in its decisions and annotated in
the affected documents (see [Annotations to other PRDs](#annotations-to-other-prds)).

In this document, a **table** is a named collection of rows inside a store; a **row** is a value for each column of
the schema; the **schema** is the list of the columns of the table, with their name, their type, whether they admit
nulls and their options (N4, N6), and its primary key; the **primary key** is the list of columns whose values
identify a row; and a **secondary index** is a list of columns, different from the primary key, by which rows can be
found.

### Product decisions

| ID | Decision |
|---|---|
| N1 | **Public surface and catalog**: from this module on, applications create tables with a schema, store, read, update and delete rows, and traverse them by the primary key and by their indexes, in Rust; they open them by their name inside a transaction with the rules of the trees (B1, B9) and with the opening modes and the catalog operations of the trees (A1, A3) that 7.1 sets. Tables and trees share the catalog and its namespace: a name belongs to a tree or to a table, never to both. Listing the catalog shows both, each with its type. Tree operations never destroy a table, nor table operations a tree: opening, deleting, renaming or emptying as a tree something that is a table, or the other way around, fails without effect with the class of different tree type (B3; in this document, *different type*), and so does renaming a tree with overwrite to a name that belongs to a table, or the other way around; without overwrite it fails with the class of already existing, as between trees of different types (A3); Exists says that the name exists whether it belongs to a tree or to a table, because they share the namespace. The trees that make up a table (its rows, its indexes and its statistics) do not appear in the catalog, cannot be opened as trees and take no user name; the operations that copy or check the entire store (the backup and the compaction of modules 10 and 11, and the verification and the rescue of the command-line tool of module 12, D9 of the parent) treat them as trees. Querying tables with filters, order and aggregates belongs to module 17. |
| N2 | **Split and closing**: the module is split into the child PRDs of the [table](#split-into-child-prds), which are built in that order; this document has no execution of its own. Each common rule is implemented and tested by the first child PRD that needs it, and the following ones keep complying with it. Module 7 is closed when 7.1, 7.2 and 7.3 are closed; dictionary compression (7.4) is deferred and outside objective 2 and D4 of the parent, like the leaf page compression of module 5. |
| N3 | **Schema and its changes**: a table has from 1 to the maximum number of columns that 7.1 sets, each with a name (UTF-8 text compared by its bytes, like tree names, B4), a type (N4) and whether it admits nulls; every column admits them unless it is declared not to. The **primary key is mandatory**: one or several distinct columns, none of vector type and none that admits nulls; a schema that does not meet this fails without effect when the table is created. The schema can change without rewriting the rows, and each change gives a new version of the table's schema: adding a column that admits nulls (earlier rows read it as null); adding a column with a default value, a constant set when adding it, which earlier rows read; removing a column that is not part of the primary key or of an index; renaming a column; and allowing nulls in a column that did not admit them and is not part of the primary key. A column added with the name of one that was removed is new: the values of the earlier one never reappear. Changing the type of a column, changing the primary key or forbidding nulls in an existing column is not supported in this module. |
| N4 | **Column types**: `bool`; signed and unsigned integers of 8, 16, 32 and 64 bits; `f32` and `f64`, where `-0` is stored as `+0` and every NaN is stored as a single NaN; `decimal(p, s)` of up to 38 digits; `string`, UTF-8 text; `bytes`; `date`, `time`, `timestamp` (an instant in UTC with microsecond resolution, with no stored time zone) and `duration`; `uuid`, with the order of RFC 9562 or, if the column declares it, that of SQL Server (N5); `ulid`; and `vector(element, dimension)`, with elements `f32`, `f16`, `i8` or `bit`, a fixed dimension per column from 1 to 4,096, a multiple of 8 for `bit`, and optionally, except with `bit` elements, the cosine metric: a column that declares it stores the norm next to each vector and does not admit zero vectors. No vector admits NaN or infinities. Autonumeric columns are 64-bit integers (N6). A value that does not meet the rules of its column (outside the domain of its type, a vector of another dimension or with NaN or infinities, a zero vector in a column with the cosine metric, a `decimal` that does not fit its precision) or a null in a column that does not admit them fails with the new class of invalid value (N10). |
| N5 | **Order**: the rows of a table and the entries of an index are ordered by the values of their columns, column by column, and the first one that differs decides: `bool`, with false before true; numbers by their numeric value, with NaN after +∞; `string` and `bytes` byte by byte, with no collation; dates, times, instants and durations in chronological order; `ulid` by its time and then by its bytes; `uuid` by its 16 bytes in the order of RFC 9562, or, in a column that declares it, in the order in which SQL Server compares a `uniqueidentifier`. In an index, each column can be ordered ascending or descending, and its nulls go first (by default) or last in the forward traversal order of that index. Vectors are not ordered: they cannot be the primary key or columns of an index. The primary key and the unique indexes compare the values after the normalization of N4, so `-0` and `+0` are the same value, and two NaN too. |
| N6 | **Rows and primary key**: a row is read, updated and deleted by its primary key, and rows are traversed by a range of the primary key, forwards and backwards, in the order of N5. An opening reports at once, without traversing the table, how many rows the table has according to what its transaction sees, like the number of entries of a tree (A5). A range of the primary key can be deleted at once, and the indexes are left as if their rows had been deleted one by one. The **bulk load** inserts at once a sequence of rows given by the application, with the rules of insert, and is faster when the rows arrive in primary key order; to load a table with indexes it is better to load it first and create the indexes afterwards (N7). There are three operations to store a row, and all of them pass the whole row: **insert**, which fails without effect with the new class of duplicate key (N10) if the key already exists; **update**, which replaces the row if it exists and, if not, does nothing and reports that it did not find it; and **insert or update**, which replaces or inserts. Krummi generates increasing primary key values, also within the same millisecond: `ulid`; `uuid` version 7, for columns with the order of RFC 9562; sequential `uuid` in the order of SQL Server, for columns with that order; and 64-bit autonumeric columns, whose value Krummi assigns when inserting, increasing and never reused: in the same table, the value of a committed row is not assigned again, even if the row is deleted, the table is emptied or the process crashes. |
| N7 | **Secondary indexes**: an index is unique or non-unique, with columns distinct from each other, ascending or descending (N5), and can carry included columns, whose values it returns without the row. Every write of a row leaves all its indexes in agreement with the table in the same transaction (G1). In a unique index, two rows with some null in their columns do not collide with each other, as in the SQL standard. An index entry too large for a tree key (B5) fails without effect with the class of invalid size (B7). Partial indexes (with a condition) are reserved in the index definition and are implemented when the query tree exists (module 17); the reservation is checked by the design review of 7.2, not by a test. |
| N8 | **Statistics**: each table has statistics of its columns, also of those that have no index, like those of SQL Server, so that the planner of module 17 can estimate how many rows meet each condition. They do not have to be exact or always up to date. From 7.3 there is a public **estimation** operation (for example, how many rows a range of a column has, or what fraction has a value), the one the planner of module 17 will use: when it does not find the statistics of the column, or finds them **stale**, it answers with a default estimation and asks for them to be built in the background; another public operation, **analyze**, builds them on request. The table carries a modification counter that survives closing the store and never decreases (emptying the table or deleting a range add the rows they remove), and the statistics of a column are stale when the modifications counted since the snapshot they were built from exceed the threshold of SQL Server: 500 if the table had up to 500 rows when they were built and, if it had more, the smaller of 500 plus 20 % of the rows and the square root of 1,000 times the rows. Statistics built from a snapshot never replace others of the same column built from a later snapshot. No other operation of the application waits for them to be computed (analyze may wait, as 7.3 sets); storing them is a short write that takes the write turn (G4), so a write of the application may wait for it like for any other writer, and that wait counts toward its time limit (T4). |
| N9 | **Dictionary compression**: deferred (N2). From 7.1, the format of the tables reserves what it will need, so that adding it later does not change the layout of the rows or force rewriting the existing ones; the design review of 7.1 checks it, not a test. Reserving it does not make activating it compatible: whether its change is incompatible is declared by 7.4 with B12. |
| N10 | **New errors** (D8 of the parent): **duplicate key**, when inserting a row whose primary key already exists, or when storing a row with the values of a unique index that already has another row, with no nulls in those columns (N6, N7), and in that case the error names the index, like SQL Server, which reports both cases as a duplicate key; **different schema**, when opening or creating a table with a schema that is not its own, in the cases that 7.1 sets; and **invalid value**, for a value that does not meet the rules of its column or a null in a column that does not admit them (N4). A row, a primary key, a column name or an index entry larger than its maximum fail with the class of invalid size (B7). The three are detected before the operation changes anything and follow the rule of the public validation errors of B7. Their data and their precedence over the other failures are set by the child PRD that introduces them. |
| N11 | **Measurement**: each child PRD includes single-workload measurements before and after its change, on Windows and on Linux, with the power plan of the benchmark. 7.1 and 7.2 add to the comparative benchmark (module 6) table workloads against the tables of Voron, with the threshold of K7 (Krummi at least 0.80 times Voron): 7.1 insert with a `ulid` key in order and with a random `uuid`, bulk load in primary key order, read by primary key, traverse a range of the primary key, update and delete; 7.2 insert with two indexes, search by an index and traverse a range of an index. The other engines of the benchmark do not run these workloads and their cells say `no aplica`, as in K6: they are key-value stores with no schema or secondary indexes (the tables of redb are key-value maps), and the benchmark does not emulate them. 7.3 measures, with a single workload, how much building the statistics in the background costs the write workloads of 7.1. |

*Annotations of 2026-11-10 from tables with schema ([tables-schema.prd.md](tables-schema.prd.md), 7.1, Spanish, authoritative):*

- *N1 and AC-N5: the catalog has three object classes, trees, tables and sequences, with one name space; Exists and List apply to all three, and the other operations fail with different type on another class (E14).*
- *N4: there are no autonumeric columns; the sentence "Autonumeric columns are 64-bit integers (N6)." no longer holds (E14). The date and time domains are those of E8.*
- *N6: "all of them pass the whole row" and "replaces the row" no longer hold: update, and insert or update when the row exists, change only the columns given (E10). The `ulid` and `uuid` generators are public functions (E15). Autonumeric columns are replaced by sequences (E14), with the same promise: a committed value is never given again, even if rows are deleted, the table is emptied or the process crashes.*
- *N10: a fourth class is added, invalid schema (E5).*
- *AC-N3: the second insert or update updates the columns given; the cases of a generated key colliding with an explicit one and of the exhausted autonumeric counter disappear; and a failed load leaves the sequences as before (E13). The cases with indexes are checked by 7.2.*
- *AC-N4: its second part applies to sequences instead of the autonumeric column (AC11 of 7.1).*
- *AC-N7: removing a column of an index fails with invalid schema (E5).*
- *Out of scope: "Updating part of a row: update replaces the whole row (N6)" no longer holds, and "Resetting or setting the counter of an autonumeric column" becomes resetting or setting a sequence after creating it.*

### Starting state

Verified on 2026-11-09 in the code, in the PRDs of the ancestors and in the design documents:

| Aspect | Situation |
|---|---|
| Data | Applications store bytes in named trees: normal, multi-value and fixed-size (module 5). Keys are bytes from 1 to 2,048, ordered byte by byte (B5); values, from 0 to 256 MiB (B6). There are no types, schema or secondary indexes. |
| Catalog | It stores trees with their type: normal, multi-value or fixed-size (B3), with the opening modes and the operations of A1 and A3. |
| Format | The current format version is `10`. |
| Benchmark | 22 workloads with a threshold against Voron (K7); none of tables. |
| Voron (reference) | Tables with schema and indexes, whose rows carry no types or nulls and whose keys are built by the application; optional dictionary compression (design documents 1, 2 and 7). |
| Design | Eight design documents with the user's decisions and a verification of their claims against primary sources ([README](../../design/tables/README.md), [verification](../../design/tables/verification.md)). *Annotation of 2026-11-10:* there are twelve since that day; the four additions cover the interchange format of queries, the tooling for agents, the query contract and the approximate vector indexes. |

## Actors

- **Integrating developer**: defines tables with their schema and their indexes, and stores, reads, updates,
  deletes and traverses rows inside transactions, in Rust.
- **Maintainer**: runs on Windows and on Linux the tests and the measurements of N11.

## Flow

1. The application creates a table with its schema and its primary key (N3), and then its indexes (N7).
2. Inside a transaction it opens the table by its name (N1).
3. In a write it inserts, updates and deletes rows; each value is validated against its column before anything
   changes (N4, N10), and the indexes are left in agreement with the table (N7).
4. In any transaction it reads rows by their primary key and traverses them by ranges of the primary key or of an
   index (N6, N7).
5. When an estimation is requested, the statistics that are missing or stale are built in the background (N8).

## Split into child PRDs

Each submodule is a child PRD of this one, with its own execution. The order is the build order. The column *Sets*
names what that child PRD must decide, in addition to its content and its own errors.

| # | Child PRD | Content | Sets |
|---|---|---|---|
| 7.1 | **Tables with schema** | Create, open, delete, rename, empty and list tables (N1); the column types (N4); the primary key (N6); insert, update, insert or update, read and delete rows by the primary key; traverse and delete by ranges of the primary key; the number of rows; the bulk load; the generators of N6; the schema changes (N3); what it reserves for compression (N9); its benchmark workloads (N11). | **Catalog**: which opening modes (A1) and which catalog operations (A3) tables have, with which classes a table that does not exist or that already exists fails, whether renaming a table can overwrite another table, what emptying a table empties (its indexes and its statistics included), and which of its operations are changes for T10 and end the references (A10), for example update without finding the row or insert or update with an identical row; the precedence of its failures (A7) and which of them can fail with damage (A9). **Limits**: the maximum number of columns and of bytes of a column name; the maximum size of a row and of a primary key (N10 sets their class). **Rows**: what happens when reading or deleting a row that does not exist; whether the primary key of a row can be updated and how; what happens with the columns that are missing, extra or have an explicit null when storing a row; the domains of `date`, `time`, `timestamp`, `duration` and `decimal`, and what happens with a `decimal` with more decimals than its scale or an instant or a duration with more resolution than the microsecond; whether a `string` admits the null character and whether the empty text and the empty bytes are valid keys; how `uuid` values with the SQL Server order are passed (in the byte order of RFC 9562 or in that of .NET), the `f16` elements and the bit order of a `bit` element; whether a `-0` inside a vector is stored as is; whether there is conversion of `ulid` and `uuid` to text and from text; whether the columns of the primary key can be descending; the semantics of the bounds of a range of a composite primary key (prefix, inclusive or exclusive), when traversing it and when deleting it, and whether deleting a range reports how many rows it deleted. **Bulk load**: whether it requires an empty table or one with no indexes; what it does with rows out of order; whether a row with a repeated key or an invalid value makes the entire load fail without effect or only that row; whether it fits in a single write and what it reports when it ends. **Generators**: whether those of `ulid` and `uuid` are public functions or assign the value when inserting, and how the application learns the assigned value; the sign of the autonumeric columns, their first value, whether it leaves gaps, how many a table can have and whether they must be part of the primary key; whether an explicit value is admitted in them and what insert or update does with a row without that value; whether an aborted write consumes values; with which class an insert fails when the counter reaches its maximum; whether the generators are monotonic per store or per process, and what they do if the clock goes backwards or the increment is exhausted. **Schema**: with which class creating a table with an invalid schema fails (without a primary key, with a key column that admits nulls or is a vector, with repeated column names, with a key that names a column that does not exist, with a vector or a `decimal` outside its limits, with the cosine metric on `bit` elements); when opening a table is a different schema; with which class an unsupported schema change fails; what happens when adding a column with a name that already exists or that exceeds the maximum, when renaming to a name that already exists, and when adding a column with the name of one that was removed (its earlier values never reappear); whether a column can have a default value when the table is created, whether the default value can be null, with which class a default value that does not meet the rules of its column fails, and whether a column added with a default value can fail to admit nulls; whether an autonumeric column or a vector with the cosine metric can be added to a table with rows; whether the schema version is visible to the application; what the openings of a table see whose schema changes in the same write. **Errors**: the data of duplicate key, different schema and invalid value, and their precedence (A7). **Benchmark**: the data of its workloads (number of rows, columns and row size, key distribution), their schema in Voron, their numbers and names, their place in the report, in the size table and in the reduced run (K4, K11). **Format**: whether its change is incompatible (B12). **Tests**: the test vectors of the SQL Server order (AC-N1), the test clock of the generators (AC-N4), the hook that shows that no name reaches an internal tree (AC-N5), the one that counts the pages a schema change writes (AC-N6) and the one that counts the pages read by asking for the number of rows (AC-N3). **Information**: whether the internal trees count in the tree and page numbers of the store information (P12). |
| 7.2 | **Secondary indexes** | Create and delete unique and non-unique indexes on a table with rows; maintain them in every write; read and traverse by an index; included columns, descending ones and the order of nulls (N5, N7); their benchmark workloads (N11). | How indexes are named and their limits (bytes of the name, columns per index and indexes per table); whether the definition of a table can carry its indexes when it is created; with which class invalid index definitions fail (on a table or a column that does not exist, on a vector column, with a repeated column in the key or in the included ones, or in both, equal to the primary key or to a prefix of it, equal to another index, or with a name that already exists); whether a vector column can be an included column; whether creating an index on a large table is done in a single transaction and where it sorts its entries; what happens when creating a unique index on rows that already repeat their values; what happens to an index when renaming one of its columns; the data of duplicate key in a unique index; which operations return rows by an index and whether they return the values of the included columns without copying; the semantics of the bounds of a range of an index (as in 7.1) and whether a range can include the nulls or ask for only them; when an index entry is too large and what its error says; the data of its benchmark workloads, as in 7.1. |
| 7.3 | **Statistics** | The per-column statistics, the estimation operation, their construction in the background and the analyze operation (N8); their measurement (N11). | Which estimations it offers and which is the default estimation; in which transactions estimation and analyze can be requested, and whether analyze waits for them to be stored; what counts as a modification for the threshold (for example, insert or update with an identical row, update without finding the row or an aborted write); whether a write that requests an estimation sees its own changes; whether they exist in temporary mode; what happens if storing them fails, where it is reported and what happens to them in a store that does not accept writes (T12), including whether a failure to store them is the first failure of T12 and who reports it; when the sampling thread is created and when it ends, whether closing the store waits for a sampling in progress, and in which place of the write queue (T4) the storing enters; what the store information (P12) reports about them, if anything; what happens to them when removing or renaming a column or deleting the table; whether vector columns have statistics; the test hooks that stop the sampling and the storing of the statistics (AC-N8). |
| 7.4 | **Dictionary compression** | Deferred (N2, N9). | — |

*Annotation of 2026-11-10 (child PRD 7.5):* child PRD **7.5, JSON columns** is added, built after 7.3 and before
module 17, so that queries can filter by JSON paths from the start. User: «Sí anótalo en el reparto del PRD de tablas
como PRD hijo 7.5, después de 7.3». **Content**: a JSON column type stored in a binary format read without copying or
parsing text, in the style of RavenDB's blittable or PostgreSQL's `jsonb`; building it, navigating it and reading its
properties; its conversion to and from text; its benchmark loads (N11). **Sets**: the binary format (one of those two
or Krummi's own) and whether its change is incompatible (B12); how numbers are represented (integers, `f64`,
`decimal`), repeated keys and property order, and when two documents are equal; its size and depth limits; the
classes with which an invalid text or document fails; whether parsing the text uses a dependency (with its D12
annotation, like P11 of design document 11) or a reader of Krummi's own; that a JSON is not a primary key or an index
column. With this annotation, N2 closes module 7 when 7.1, 7.2, 7.3 and 7.5 are closed, and "JSON" leaves *Out of
scope*. Indexing paths inside a JSON stays outside 7.5 (expression indexes, after 7.2 and module 17).

*Annotation of 2026-11-10 (child PRD 7.6):* child PRD **7.6, Approximate vector indexes** is added, built after 7.5
and before module 17, so that queries can search for the nearest neighbours approximately from the start. User:
«Además los índices ANN hay que inluirlos en el alcance» and, on the order, «Tras 7.5, antes del 17». Its technical
starting state is [design document 12](../../design/tables/12-vector-indexes.md). **Content**: an approximate vector
index over a vector column, of a class distinct from the secondary indexes of 7.2, which document 12 recommends
building with partitions (IVF) and 8-bit codes per component (SQ8); its asynchronous maintenance: writing a row only
records that the row changed, and the store brings the index up to date in the background, in short batches, also
when it is created and rebuilt; a search for the k nearest neighbours that the application asks for explicitly as
approximate and that returns only rows visible in its snapshot that satisfy its filters, with their exact distance;
its benchmark loads (N11). Exact distance searches never use the index and return the same as without it. **Sets**:
the algorithm, after measuring document 12's experiment against its recall and latency thresholds, and the
alternative if it misses them; which element types and metrics it supports; whether a search sees the rows the index
has not yet brought up to date and what it does when there are too many; whether an index can declare partition
columns, for example per tenant; whether creating and rebuilding it is done without blocking writes; how long the
background work may hold the write turn (G4), and with how many threads and how much memory it trains; whether
rebuilds are explicit and which metrics recommend them; what a search does while the index is not ready and how many
rows it needs to train; the classes with which invalid definitions fail; what the store information (P12) reports
about the index; what happens to the index when the table or the column is deleted, when the table is emptied and in
a bulk load; whether its change is incompatible (B12). The six product questions of document 12 are part of what it
sets. With this annotation, N2 closes module 7 when 7.1, 7.2, 7.3, 7.5 and 7.6 are closed; the sentence of N7 "Every
write of a row leaves all its indexes in agreement with the table in the same transaction (G1)" holds for secondary
indexes, not for approximate vector indexes, which are brought up to date after the transaction; "cannot be the
primary key or columns of an index", of N5, refers to the primary key and to secondary indexes; and "Vector indexes" leaves
*Out of scope*. Filtering by a distance threshold stays exact and belongs to module 17.

## Validations and messages

Those of the ancestors hold. This module adds these situations, only from the library, with the rule of N10:

| Situation | Error class |
|---|---|
| A row is inserted whose primary key already exists | Duplicate key (new) |
| A row is stored with the values of a unique index that already has another row, with no nulls in those columns | Duplicate key (new), with the name of the index |
| A table is opened or created with a schema that is not its own, in the cases that 7.1 sets | Different schema (new) |
| A value that does not meet the rules of its column, or a null in a column that does not admit them | Invalid value (new) |
| A row, a primary key, a column name or an index entry larger than its maximum | Invalid size (B7) |
| A tree operation on a table, or a table operation on a tree, including renaming with overwrite onto the other type | Different type (B3) |

Updating a row that does not exist is not an error: the operation reports that it did not find it (N6). Each child
PRD adds the situations of its operations and sets the data of each error.

## Output

The module writes nothing to the screen. A read returns the stored values, with the normalization of N4 (`-0` and
NaN); those of `string`, `bytes` and vector, without copying them (D11, T16, B10), and the others, decoded into a
copy owned by the application (exception to D11, like List in A3). The store information (P12) adds no data except
what 7.3 sets. The benchmark report adds the workloads of N11 and counts their thresholds with the others.

## Reuse

- The B+ trees, their catalog, their rules for names, sizes and errors (B1–B12, A1–A15), and their traversals (5.2).
- The transactions, the journal and the recovery: the changes of the tables, their indexes and their statistics are
  tree changes (G1, G5).
- The single-workload measurements of the performance plan and the comparative benchmark (module 6).

## Out of scope

- Queries with filters, order, limits and aggregates, the cost-based planner and the execution plans (module 17).
- The C interface and the C# binding (module 18).
- Vector indexes: filtering by distance is a traversal of module 17. *Annotation of 2026-11-10:* approximate vector
  indexes come into scope as child PRD 7.6 (see *Split into child PRDs*), with [design document
  12](../../design/tables/12-vector-indexes.md) as its technical starting state.
- Raw data sections: storing bytes under an identifier is covered by the containers (module 13).
- Joins, `GROUP BY`, views and triggers; foreign keys, constraints between tables, and `CHECK` constraints.
- Arrays, enums, JSON, stored time zones, `f16` as a scalar and case-insensitive text comparison.
- Changing the type of a column or the primary key of a table, forbidding nulls in an existing column, and changing
  the default value of a column or the order of the columns.
- Updating part of a row: update replaces the whole row (N6).
- Rows larger than the maximum that 7.1 sets (streams, module 8).
- Tables without a primary key.
- Partial indexes (reserved, N7) and dictionary compression (deferred, N9).
- Exact counts of the rows between two keys (the number of rows of the whole table is included, N6).
- Resetting or setting the counter of an autonumeric column.
- Renaming an index or changing its definition: it is deleted and created again.
- Statistics of several columns together.
- Importing and exporting row files (CSV, JSON lines, Parquet): module 19. From the application, the bulk load
  imports (N6) and a traversal exports.
- Encrypting the tables: confidentiality at rest is delegated to the file system (annotation of 2026-11-09 to the
  parent).
- Showing from the command-line tool the rows decoded according to their schema (module 12); the verification and
  the rescue do cover the tables, as trees (N1).
- Several writes at the same time on different tables of the same store (G4).

## Left to the implementation

- How rows and keys are encoded and how the tables, their indexes and their statistics are organized in trees,
  provided they meet the guarantees of the parent and this PRD. The design documents set their starting point:
  rows clustered by the primary key, keys with the tuple encoding of FoundationDB, and index entries made of their
  columns and the primary key.
- The names of the public operations, of the types and of the new error classes, which follow the Rust conventions
  and are in English (D8 of the parent).

## Acceptance criteria

Those of the ancestors hold. Each criterion is checked by the child PRD named in it.

1. **AC-N1** — Given a table with one column of each type of N4 that can be ordered, as a one-column primary key
   and as part of a multi-column one, when rows are inserted and traversed by the primary key forwards and
   backwards, then they come back in the order of N5 for each type, including `ulid`, negative integers, `-0` and
   NaN, and `uuid` in both orders: that of RFC 9562 is that of its 16 bytes, and that of SQL Server is the one that
   `SqlGuid` of .NET gives to the same values, on test vectors set by 7.1 ordered with `SqlGuid`, and empty `bytes`
   and `bytes` with the zero byte in any position (and empty `string` and `string` with the null character if 7.1
   admits them) in front of other columns of the key; when a range of the primary key is traversed, on a whole key
   and on a prefix of a composite one, with each combination of included, excluded and absent bounds, then exactly
   the rows of the range come back according to the semantics that 7.1 sets; when a property test stores random
   rows in tables with random primary keys, of one to four columns of all the types that can be ordered, with their
   extreme values (`-0` and `+0`, NaN with different bits, `uuid` in both orders, the date and time types and
   `ulid`) and keys at the size limit, then the order of the traversals, the rows of each range and the duplicate
   keys are those of a reference model of N5 that compares the values by their type, without using their encoding;
   and each row is read with the stored values, `-0` as `+0` and every NaN as the single NaN (7.1).
2. **AC-N2** — Given a table, when a row is inserted with a primary key that already exists, a value that does not
   meet the rules of its column (one for each rule of N4) or a null in a column that does not admit them, then the
   operation fails with its class (N10), the table and the transaction stay as before, and the transaction can
   commit its other changes; inserting `+0` with a stored `-0` key, or a NaN with another, gives duplicate key
   (N5); when values at the limits of their type are stored (vectors of dimension 1, or 8 for `bit`, and 4,096, of
   each element type, and a `decimal` of 38 digits), then they are read back equal; when a table is created with
   each of the invalid schemas that 7.1 lists, then it fails without effect with the class that 7.1 sets; and when
   a table is opened in each case of different schema that 7.1 sets, then it fails with different schema (7.1).
3. **AC-N3** — Given a table, when a row that does not exist is updated, then the table does not change and the
   operation reports that it did not find it; when a row that does not exist is inserted or updated and then again
   with other values, then the first time it inserts it and the second it replaces it; reading and deleting a row
   that does not exist give what 7.1 sets; the number of rows that an opening reports is, after each operation of
   the test, that of the rows its transaction sees, reading the same pages with 10 as with 100,000 rows according to
   the test hook of 7.1, also in a concurrent read that does not see the write; when a range of the primary key is
   deleted, then exactly the rows outside the range remain; and when 100,000 rows are bulk loaded in primary key
   order, then the table has the same rows as if they had been inserted one by one; and loads with rows out of
   order, repeated or invalid give what 7.1 sets, also when the one that fails is the last, when a generated key
   collides with an explicit one or with a row that already existed and when the autonumeric counter is exhausted:
   if 7.1 sets that the load fails entirely, then the table, its indexes and its counters stay as before the load,
   also when the source of the rows fails or the application cancels the load halfway, and another write gets its
   turn afterwards (7.1).
4. **AC-N4** — Given 100,000 values of each generator of N6, generated with the test clock that 7.1 sets, as many in
   the same millisecond as the increment that 7.1 sets allows and the rest in successive milliseconds, from a single
   thread, when they are ordered with the order of their column, then they end up in the order in which they were
   generated; and given a table with an autonumeric column, when insertions are committed, the row with the highest
   value is deleted and committed, the table is emptied and committed, and a process crash is simulated and the
   store reopened, then the following insertions in that table never repeat a value of a committed row (7.1).
5. **AC-N5** — Given a tree and a table in a store, when a table is created with the name of the table, then it
   fails with the class of already existing that 7.1 sets; when a table is created with the name of the tree, or a
   tree with that of the table, then it fails with different type, which wins over already existing (B3); when the
   table is opened, deleted, renamed or emptied as a tree and the tree as a table, or one is renamed to the name of
   the other with overwrite, then each operation fails without effect with different type; when one is renamed to
   the name of the other without overwrite, then it fails without effect with already existing; Exists says that
   both names exist; and listing the catalog shows the table with its type and none of its internal trees, which,
   according to the test hook that 7.1 sets, no name reaches as a tree (7.1).
6. **AC-N6** — Given a table with rows, when a column that admits nulls is added, one with a default value, a column
   is removed, another is renamed and nulls are allowed in another, then the rows stored before are read with the
   new schema, with null or with the default value in the added columns, a column added with the name of a removed
   one is read as null, and the write of each change writes the same pages with 10 as with 100,000 rows, according
   to the test hook that 7.1 sets; and when each unsupported change of N3 is requested, or removing a column of the
   primary key, then it fails without effect with the class that 7.1 sets (7.1).
7. **AC-N7** — Given a table with rows, when unique and non-unique indexes are created, ascending and descending,
   with nulls first and last, with each combination of direction and order of nulls in consecutive columns, also
   after `bytes` columns with the zero byte (and `string` with the null character if 7.1 admits it), and with
   included columns, then each index has the entries of the rows that were already there;
   when a write inserts, updates, deletes rows and deletes a range of the primary key and commits, and in another
   case aborts or a process crash is simulated, then each index has exactly the entries of the rows of the table in
   the order of N5, and two rows with nulls in a unique index do not collide; when an index is deleted, then it
   disappears and the table does not change; when each index is read and traversed forwards and backwards, then the
   rows, or the values of their included columns, come back in that order; when a range of a column of an index is
   traversed with the previous ones fixed, with each combination of included, excluded and absent bounds, then
   exactly the rows of the range come back according to the semantics that 7.2 sets; the same property test of
   AC-N1, with random indexes of one to four columns of any direction and order of nulls, gives the order and the
   rows of the reference model; and
   when a row is stored that repeats the values without nulls of a unique index, or whose index entry is too large,
   or a column of an index is removed, then it fails without effect with duplicate key, with the name of the index,
   with invalid size or with the class that 7.1 sets, also when inserting and when updating in a table with several
   unique indexes when each of them fails in turn, and the row, the other indexes and the number of rows stay as
   they were and the transaction can commit its other changes; and when a unique index is created on rows that
   already repeat their values and the write commits other changes, then the index does not exist, the table and its
   other indexes do not change, and, if 7.2 sets that this case fails without effect, the store information (P12)
   is the one it would have without the attempt (7.2).
8. **AC-N8** — Given a table with columns without an index and the sampling stopped at the hook that 7.3 sets, when
   an estimation is requested without statistics, then it answers the default estimation without waiting, and the
   reads and writes of the application finish; when the sampling is released, then the statistics are stored and
   the same estimation, on data chosen so that it differs from the default estimation, uses them; when the
   modifications exceed the threshold of N8 and another estimation is requested, then it answers the default
   estimation and they are built again, also when part of those modifications was made before closing and reopening
   the store and when the table is emptied; when analyze is requested, then, when it ends as 7.3 sets, the
   estimation uses the new statistics, also if it is requested while they are being built in the background and
   either of the two ends first, and without either of them being left waiting for the other in the transactions in
   which 7.3 allows requesting it; if 7.3 allows analyzing inside a write, when that write changes rows, analyzes
   and aborts, then the rows, the counter and the statistics stay as before, and when it changes more rows after
   analyzing and commits, then those modifications count toward the threshold; when a bulk load fails while
   statistics are being built in the background, then the ones that are stored count only the rows of the load that
   remained committed according to what 7.1 sets (none if the load fails entirely); and with the storing of the
   statistics stopped at its hook, a write of the application with a time limit fails with the timeout error when it
   elapses (T4) (7.3).
9. **AC-N9** — Given the workloads of N11, when they are compared with the tables of Voron in a full run of the
   benchmark, then Krummi reaches the threshold of K7 in each one, on Windows and on Linux; a missed threshold
   follows K13: it does not prevent closing the child PRD and becomes a separate change (7.1, 7.2).
10. **AC-N10** — Given the closing of each child PRD, when its report is reviewed, then it includes the single-workload
    measurements of N11, on Windows and on Linux: the benchmark workloads that already existed, before and after its
    change, and its new workloads after (7.1, 7.2, 7.3).

## Annotations to other PRDs

This PRD changes or refines rules of earlier documents, which carry the annotations of 2026-11-10 (in their
translations too):

- Root parent, split: the row of module 7 now says that it also sets the column types (N4) and the invalid value
  error (N10), without the raw data sections (out of scope; module 13 covers them), with dictionary compression
  deferred (N2). The **module 17, Queries** (query tree built with a fluent API, validation, cost-based planner,
  execution plans that the public interface also accepts, and execution by cursor) and the **module 18, C interface
  and C# binding**, which are built right after 7, in that order, and the **module 19, Importing and exporting row
  files** (CSV, JSON lines, Parquet), after 18, are added; numbered this way so that the numbers that other documents
  already cite do not move.
- Root parent, objective 2 and D4: the product is complete with the modules of the split without 9 (annotation of
  2026-11-09 on encryption), with 17, 18 and 19, and without dictionary compression (N2), although Voron has it.
- Root parent, D2: the public surface adds the C interface and the C# binding of module 18.
- Root parent, D11 and AC20: the values of the columns that are not `string`, `bytes` or vector are returned decoded
  into a copy owned by the application (Output), like List in A3.
- Root parent, Out of scope: the item «Servidor de red, protocolo remoto, lenguaje de consulta o SQL» now excludes
  only the network server, the remote protocol, a text query language and SQL: the query tree and the execution plans
  enter module 17. The item «Bindings para otros lenguajes (API C, .NET, Python)» now excludes the other languages:
  the C interface and the .NET binding enter module 18.
- Root parent, D12 and AC21: the C interface (module 18) is, with the Pager, the only part of the engine that may
  use unsafe code; a dependency that uses unsafe code is admitted case by case, if it is widely used and maintained,
  Rust only (no C), without a reasonable safe alternative, and annotated with its reason in the PRD that introduces
  it; dependencies with C code are out unless expressly decided ([design](../../design/tables/README.md)).
- Root parent, modules 10, 11 and 12 of the split: the backup, the compaction and the verification and the rescue
  also copy or check the internal trees of the tables, as trees (N1).
- Root parent, AC19: the benchmark workloads are those of K6 and K7 plus the table workloads of N11.
- Benchmark, K7 and *Output*: besides the 22 workloads of modules 1 to 5, the table workloads of N11 have a threshold
  (D5), only against Voron; the report counts all the thresholds, so «Krummi cumple N de 22 umbrales» now counts over
  the total, and K13 holds for them: missing one ends with code `1`. K6: the table workloads follow its rule of
  engines that do not offer them (`no aplica`). K4 and K11: the data of the table workloads and their place in the
  reduced run are set by 7.1 and 7.2.
- Trees, B3: the catalog also stores tables (N1). Requesting as a tree something that is a table, or the other way
  around, fails with different type, with the same precedence over already existing; and tree operations never
  destroy a table, so the sentence «Las operaciones que nombran un árbol sin pedir un tipo (como borrarlo) nunca
  fallan con tipo de árbol distinto» no longer holds when the name belongs to a table. B8: the data of the invalid
  size of the new sizes (an index entry, a column name, a primary key, a row) are set by 7.1 and 7.2, and those of
  the different tree type may name the table type; the list of types of B3 does not change, because a table is not
  a tree type, but it shares its catalog.
  B11: the internal trees of the tables have no limit of their own; the limit of indexes per table that 7.2 sets is
  a limit of the tables, not of the number of trees.
  B4 still holds because the internal trees of a table take no names. B7: the rule of the public validation errors
  also holds for the three classes of N10.
- Key operations, A1, A3, A6, A7, A9 and A10: deleting, renaming or emptying a table as a tree fails with different
  type, and Rename with overwrite does not delete a destination that is a table, it also fails with different type
  (A3 said «sea del tipo que sea»); without overwrite, a destination that is a table fails with tree already
  existing, as A3 says; Exists and List also cover the tables, with their type; the opening modes, the classes of not
  found and already existing, the precedence, the damage and the changes of the table operations are set by 7.1.
- Transactions, T4: the write that waits may wait for the internal write that stores the statistics (N8). With a
  limit of zero, a write fails if that storing is in the queue. T12: that internal write may be the one that finds
  the first input and output failure; how it is reported is set by 7.3.
- Trees, Out of scope «Tablas con esquema, índices secundarios y compresión de valores por diccionario (módulo 7)»:
  this module covers it.

## References

- Parent: [krummi.prd.en.md](krummi.prd.en.md) v1.1 (D2, D4, D5, D8, D9, D11, D12, G1, G4, G5, AC19, AC21, split)
- Trees: [trees.prd.en.md](trees.prd.en.md) (B1–B12) and their children, especially
  [trees-keys.prd.en.md](trees-keys.prd.en.md) (A1–A15) and [trees-scan.prd.en.md](trees-scan.prd.en.md)
- Transactions: [transactions.prd.en.md](transactions.prd.en.md) (T4, T10, T11, T12, T16)
- Benchmark: [benchmark.prd.en.md](benchmark.prd.en.md) (K4, K6, K7, K11, K13, *Output*)
- Design: [docs/design/tables/README.md](../../design/tables/README.md) and its documents 1 to 12
- Parked document product: [documents.prd.md](../documents/documents.prd.md)
