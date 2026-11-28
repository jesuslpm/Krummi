# Column Projections of a Table — PRD

**Version**: 1.0
**Date**: 2026-11-15
**Parent document**: [Row Reads Without Checking Their Content — PRD](tables-row-reads.prd.en.md) (v1.0)

> English translation of [tables-projection.prd.md](tables-projection.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Read the columns of the rows of a table with a **projection**: the application names once the columns it is going to
read, Krummi prepares once everything that depends on the schema and not on each row, and then each row is read without
repeating that work. The prepared projection is kept in memory in the store and is shared by all its transactions.

**Why**: today every read of a column repeats, in each row, work that depends only on the schema: finding the column
and checking its type and its way of being stored. Voron reads the fields of its rows directly by their position,
without types. In the last full run of the benchmark, `tabla-intervalos` (W27) has a ratio to Voron of 0.91 on Windows
(2026-11-13) and of 0.95 on Linux (2026-11-12). In the diagnosis of 2026-11-15, a prototype that prepares that work
once per schema and shares it between transactions reduced the time of W27 by 6.7 %, with an improvement in the four
rounds (its ratio went from 0.926 to 0.992), and that of `tabla-lecturas` (W26) by 2.8 %, with one round against.
Reading with columns prepared beforehand measured more than reading by position inside the projection (6.7 % against
3.8 % in W27).

The user accepted keeping the projection in memory in the store («Me parece bien entonces.»), retaining it until the
store is closed with a cap on entries («Sí me sirve»), tying it to the schema it was prepared with, returning an error
for what depends on the schema and a panic for a failure of the program, offering only prepared columns of all the
types and not offering a warm-up of the old versions («Sí» to each one), for the benchmark: «Usa la proyección siempre
que sea útil.», and the criteria that close the gaps found in the review of the draft («Sí»).

This PRD brings forward the projections that [tables-schema.prd.en.md](tables-schema.prd.en.md) leaves for module 17,
extends **different schema**, the list of **invalid value** rules and the damage rule of that PRD, and makes precise how
W26 and W27 and K7 of the benchmark read their rows; all of them carry its annotation (*Annotations to other PRDs*).

### Product decisions

| ID | Decision |
|---|---|
| PY1 | **Prepare a projection**: from an opening of a table, in a read or a write transaction, with a list of names of live columns in any order, of the key or not, each one only once. The projection is tied to the table and to its **schema**, the state of the schema that the opening sees (E2): the one identified by the identities and the state, not the version number, so that of an abandoned write never matches another. The order of the list matters: it gives the positions of the projection, and the same list in another order is another projection. Preparing does not read rows, is not a change for T10 and does not end references (E16), like Read, and its errors are public validation errors (B7): they do not change the transaction. It is checked only once, with the precedence of E17: (1) a name that is not 1 to 1,024 bytes long fails with **invalid size**, with the data of B8, the first in the order of the list; (2) an opening of a table that the write deleted, with **tree not found**, like every operation through it (E17); (3) a name that is not that of a live column of that schema, also that of a removed column or that of one added by a later schema, with **different schema**, with the table and the column, the first in the order of the list; (4) a repeated column, with **invalid value** and the rule repeated column, which is reported as in the rows (E17); and an empty list, with **invalid value** and the new rule projection without columns, with the table. The table carried by these errors and those of PY2 and PY3 is that of the projection, with the name it had when it was prepared. |
| PY2 | **Prepared columns**: from a projection one obtains, for each position of its list, a prepared column with the type it is going to be read with, one of the column types of N4 that exist today, regardless of whether the column admits nulls; a `string` column can also be prepared to be read as its bytes (L2). There are prepared columns for all those types, also for the columns of the key. A type that is not that of the column fails with **different schema**, with the table, the column and the difference (the type requested and that of the column), like E6, without changing the transaction. A position outside the list ends with a normal panic of the thread, like that of L2: it is a failure of the program and changes neither the store nor the transaction. There is no read by position inside the projection: it is read only with prepared columns. |
| PY3 | **Read a row with the projection**: every row read by its key or traversed in a range in either direction, in a read or a write transaction, and also a **copied row** by the application (the one that keeps its values after the next change of its transaction, with its table, its store and its schema), is first tied to the projection and then read column by column with the prepared columns, each with its value or null. The values are the same, with the same normalization (N4, E7), as reading the column of the row by its position (E11), and they are returned in the same way: without copying the text, the bytes and the vectors (D11, *Output* of [tables.prd.en.md](tables.prd.en.md)), and decoded into a copy owned by the application the others; of a copied row, all are returned as when reading it today by its position, in a copy owned by the application that can outlive the copied row (user, between that copy and a borrow of the copied row: «A»). A row stored with an old version of the schema is also read, with null or its default value in the columns added after it was stored (N3). Tying to the projection a row of another table, also of one created with the same name after deleting its own, of another open store (also one opened from a copy of the same files) or of a transaction that sees another schema of the table fails with **different schema**, with the table, without changing the transaction. Reading with a prepared column of another projection, also of the same list in another order, ends with a normal panic of the thread, like that of L2: it is a failure of the program and changes neither the store nor the transaction. A prepared column serves for every projection of the same list, in the same order, over the same schema, whether the store keeps it or not (PY5). Preparing, obtaining prepared columns, tying and reading do not read pages, so they do not fail with damage: the damage of the row and of the history keeps appearing in the read or in the step of the traversal that returns the row (L1). |
| PY4 | **Schema changes**: a projection does not adapt to another schema of its table (PY3), not even within the same write. After a schema change that gives a new schema (E16), the application prepares another projection from an opening that sees it. What does not change the schema (a schema change that changes nothing, emptying the table, renaming it) does not affect it; deleting the table and creating another with its name gives another table (PY3). |
| PY5 | **The projection in the store**: it is kept in memory in the open store, never in its files, so closing it loses it; it is valid while the store is open, and the compiler prevents using it afterwards (T8). All its transactions and its threads share it (T6). Preparing again the same list, in the same order, over the same schema returns the kept projection. Until it is closed, the store keeps at most 1,024 projections, the first ones that are prepared, counting those of any table, schema or transaction, also those of a write that is abandoned; with several preparations at once when one slot is left, one of them is kept, without fixing which. Past the cap, preparing a new list works the same, but gives a projection that is not kept. A kept projection is never removed before the store is closed. |
| PY6 | **Content of the row** (L1, L2 of the parent): reading with the projection does not check the content of the row. A malformed row is read as L2 says: a column, also one of the key, may give a value that is not the one written or end with a normal panic of the thread, and a text that is not UTF-8 read as text always ends with a panic; read as its bytes, it is returned as it is. With a build of the engine that aborts on panics, each panic of this PRD ends the process (L2). |
| PY7 | **The benchmark loads** (K7): Krummi reads with a projection the rows of W26 and W27, and of the table loads of the benchmark that read column values in future PRDs, unless their PRD says otherwise. It is the read with the real capabilities of the engine, the rule that W28 already follows. Preparing the projection counts in the measured time of each repetition, before its first row, whether the store returns it kept or not. The loads do not change: the same rows, the same columns and the same checksum in both engines. The thresholds do not change. |

*Annotations of 2026-11-18 from the secondary indexes ([tables-indexes.prd.md](tables-indexes.prd.md), Spanish,
authoritative):*

- *PY3: the rows of an index traversal are tied to a projection like those of a primary key traversal; the entries of an
  index are not (decision IX8).*

*Annotations of 2026-11-21 from the JSON columns ([tables-json.prd.md](tables-json.prd.md), 7.5, Spanish,
authoritative):*

- *PY2: there are JSON prepared columns; a JSON column is prepared only as JSON, and a column of another type is not
  prepared as JSON (JS5).*
- *PY3: JSON is returned without a copy, like text, bytes and vectors, except from a copied row, where it is returned
  in a copy that is navigated the same way (JS5).*
- *Out of scope: tables-json.prd.md fixes the JSON prepared column (JS5).*

*Annotations of 2026-11-23 from the collation of text columns ([tables-collation.prd.md](tables-collation.prd.md), 7.7, Spanish,
authoritative):*

- *PY2: a `string` column is prepared with the `string` type, whatever its collation (IC1).*

*Annotation of 2026-11-28 from the queries ([queries.prd.en.md](../queries/queries.prd.en.md); Spanish
authoritative): whether module 17's queries use the projections is fixed by 17.1 of the queries.*

*Annotation of 2026-11-28 from the single-table queries ([queries-single-table.prd.md](../queries/queries-single-table.prd.md), CU13;
Spanish authoritative): queries do not take any of the projections the store keeps for the application (PY5).*

### Starting state

- A row that is read or traversed is read column by column, by the name or the position of the column among the live
  columns (E11). Each read checks the type of the column; reading a column with another type ends with a panic.
- Nothing of what is prepared to read the rows of a table is shared between transactions: each one prepares it again
  when it opens the table for the first time.
- There are no projections: [tables-schema.prd.en.md](tables-schema.prd.en.md) leaves "Reading only some columns of a
  row (projections)" for module 17.
- W26 and W27 read in Krummi all the columns of each row column by column and compute the checksum of *Benchmark loads*
  of [tables-schema.prd.en.md](tables-schema.prd.en.md); Voron reads each field by its position.
- Ratios of the last full run: W26, 0.97 on Windows and 1.10 on Linux; W27, 0.91 and 0.95. They are context, not
  criteria: the acceptance is AC-PY11.
- No stored data changes: the format of the rows, of the keys and of the store is that of 7.1 (E20).

## Actors

- **The application** that reads rows of a table and wants to read their columns at the lowest cost per row.
- **The comparative benchmark**, whose loads W26 and W27 read the columns of the rows of Krummi (K7).

## Flow

1. The application opens the table and prepares a projection with the names of the columns it is going to read (PY1).
2. It obtains from the projection a prepared column for each position, with its type (PY2).
3. It reads or traverses rows as today (E11, E12), ties each one to the projection and reads its columns with the
   prepared columns (PY3).
4. In another transaction, it prepares the same list again and obtains the kept projection (PY5); after a schema
   change, it prepares another one with the new schema (PY4).

## Validations and messages

Those of the ancestors apply. This PRD adds, only in the library, in the order of their precedence:

| Situation | Error class |
|---|---|
| Preparing with a name that is not 1 to 1,024 bytes long | Invalid size (B7, E18), with the data of B8 |
| Preparing from an opening of a table that the write deleted | Tree not found (A6, E17) |
| Preparing with a name that is not that of a live column of the schema that the opening sees | Different schema, with the table and the column |
| Preparing with a repeated column | Invalid value, rule repeated column, with the table and the column |
| Preparing with an empty list | Invalid value, rule projection without columns (new), with the table |
| Obtaining a prepared column with a type that is not that of the column | Different schema, with the table, the column and the difference |
| Tying to the projection a row of another table, of another store or of another schema | Different schema, with the table |
| Obtaining a prepared column of a position outside the list; reading with a prepared column of another projection | Normal panic of the thread, not an error (PY2, PY3) |

Errors do not change the transaction (B7); panics follow L2. Preparing, obtaining prepared columns, tying and reading
do not fail with damage (PY3).

## Output

The module writes nothing on screen. Reading a prepared column of a tied row gives its value or null, with the form and
the borrows of reading the column by its position: without copying the text, the bytes and the vectors of a row that is
read or traversed, and in a copy owned by the application those of a copied row (PY3). The reads, traversals and column
reads that already exist do not change their output. The benchmark report does not change its form.

## Reuse

- The reads and traversals of rows (E11, E12), which keep returning the rows that the projection reads.
- The values, their normalization and their borrows of today's column reads (N4, E7, D11).
- The read of rows of old versions of the schema with their default values (N3) and the lack of a check of their
  content (L1, L2).
- The error classes, their data and their precedence (E6, E17, E18, B7, B8).

## Out of scope

- Reading by position inside the projection, without prepared columns (PY2).
- A projection that adapts to another schema of its table, or that serves for several tables (PY3, PY4).
- Warming up the old versions of the schema before reading their rows.
- Configuring the cap of kept projections, removing them before closing the store, keeping them in its files or
  reporting them in the information of the store (P12).
- Reading a column as a type other than its own, except a `string` as its bytes (PY2).
- Writing rows with a projection.
- Reading less of each row from storage: the row is still read whole from its page; the projection only saves work when
  reading its columns.
- Changing the column reads that already exist, which stay as today.
- The column types that future PRDs add (JSON in 7.5), which will fix their prepared columns.
- The row readers of the engine in future PRDs: secondary indexes (7.2), statistics (7.3), JSON columns (7.5), queries
  (17), the C interface and the C# binding (18) and import and export (19), which will decide whether they use
  projections. The benchmark loads keep following PY7.

## Left to the implementation

- The names of the operations and of the types, in English and with the conventions of Rust (D8); column names are
  text, so one that is not UTF-8 cannot be given.
- How much of the type of N4 a prepared column asks for: asking for the exact type of the column is always accepted and
  asking for one of another class fails (PY2); whether the parameters of the type (the precision and scale of a
  `decimal`, the element type and the dimension of a vector, the order of a `uuid`) are part of what is asked is fixed
  by the implementation with the conventions of the column reads that already exist.
- How the schema to which a projection is tied is identified, with the rule of PY1: never the name of the table nor the
  version number.
- When and how what depends on each old version of the schema is prepared, and how much memory each projection takes.
- Whether the engine itself uses projections where it reads columns of rows, without changing anything observable.
- How the tests check that a projection was kept or not, and that W26 and W27 read with a projection, and how W27 is
  run without it for the measurement of AC-PY11: they are test and diagnostic hooks, not public contracts.

## Acceptance criteria

1. **AC-PY1** — Given a table with a column of each type of N4 that exists today (with vectors of each element type,
   `uuid` in both orders and `decimal` of several scales), with columns that admit nulls and a key of several columns,
   and rows with values and with nulls, when a projection with all its columns in an order different from that of the
   schema, one with a single column, one only with columns of the key and one with some columns are prepared, and rows
   are read with them by their key and by traversing ranges forwards and backwards, in a read transaction and in a
   write one that changed rows before, then each prepared column gives the same value as reading the column by its
   position, null where the row has null, and the text, the bytes and the vectors are returned without copying, at the
   same address; a `string` column prepared as bytes gives its bytes; and the tests pass the same compiled in debug and
   in optimized.
2. **AC-PY2** — Given a table with rows stored before adding a column with a default value, before adding one that
   admits nulls and before removing one, when they are read with a projection of the current schema, then they give the
   same values as reading their columns by their position, and the added columns give their default value or null; and
   a row of a schema that is missing from the history fails with damage in the read or in the step of the traversal,
   before tying it, as in AC-L3.
3. **AC-PY3** — When a projection is prepared with a name of 0 bytes, one of 1,025 bytes, one that the table does not
   have, that of a removed column, that of one added by a write committed after opening the transaction, a repeated
   column and an empty list, and from an opening of a table that the write deleted, then they fail with invalid size
   (with the data of B8), different schema, different schema, different schema, invalid value with the rule repeated
   column, invalid value with the rule projection without columns and tree not found, each with its data; a name of
   1,024 bytes of a live column is accepted. With several failures at once the one of PY1 that comes first is reported:
   a long name from an opening of a deleted table gives invalid size; an unknown name from that opening, tree not
   found; an unknown one and a repeated one, different schema; and with two unknown names, or two long ones, the first
   of the list is reported. The write transaction in which they fail is committed afterwards as if they had not
   happened; a write that only prepares projections has no changes for T10; and a row read before preparing stays valid
   afterwards.
4. **AC-PY4** — When a prepared column is obtained with a type of another class than that of the column (for example
   `i64` of a text, text of an `i64` and `f64` of a `decimal`), then it fails with different schema with the table, the
   column and the difference, and the write transaction is committed afterwards; with its type, it is obtained; and with
   a position outside the list it ends with a panic.
5. **AC-PY5** — Given a projection, when a row of another table with the same columns, one of a table created with the
   same name after deleting its own, one of another store opened from a copy of the same files, one of a transaction
   that sees the table after a schema change (in another write and in the same one) and one of a transaction prior to
   the change with a projection of the new schema are tied to it, then they fail with different schema with the table,
   and the write transaction is committed afterwards; a projection prepared with the schema that each row sees reads
   it; after renaming the table and after emptying it, the projection keeps reading its rows, and its errors carry the
   name the table had when it was prepared; and a projection of an abandoned write does not tie the rows of another
   write that reaches the same version number with another schema.
6. **AC-PY6** — Given a prepared column of another projection of the same table and one of the same list in another
   order, when a tied row is read with them, in a read transaction and in a write one, then they end with a normal
   panic of the thread; if the application catches it around that read, the transaction stays intact, and if it catches
   it outside, the transaction ends or is discarded as in AC-L5.
7. **AC-PY7** — Given a projection prepared in a transaction, when the same list is prepared in the same order over the
   same schema in another transaction, then the kept projection is obtained according to the test hook, and the same
   list in another order gives another projection; used at once from four threads and from two read transactions, it
   gives the same values as reading the columns by their position; after closing and opening the store, the same list
   is no longer kept until it is prepared; and using a projection after closing its store does not compile.
8. **AC-PY8** — Given 1,023 projections kept in a store, counting also those of an abandoned write and those of a
   previous schema, when two new lists are prepared, then the first is kept and the second is not, according to the
   test hook; the second reads the rows with the same values, and a prepared column of one preparation of the second
   serves for another preparation of the same list; and the 1,024 kept ones stay that way until the store is closed.
9. **AC-PY9** — Given a row with a text that is not UTF-8 in a page with correct integrity, when it is read with the
   projection, then its other columns give their values, reading that text with a column prepared as text ends with a
   panic and reading it with one prepared as bytes returns it as it is (L2); the same compiled in debug and in
   optimized.
10. **AC-PY10** — Given a row copied by the application, when it is tied to the projection and read after the next
    change of its transaction, then it gives the same values as reading the columns of that copy by their position; its
    text, its bytes and its vectors stay valid after releasing the copied row, like those of the read by position; and
    tied to a projection of another schema of its table, it fails with different schema.
11. **AC-PY11** — Given the full runs of the benchmark on Windows and on Linux, with W26 and W27 reading with the
    projection (according to the test hook), when they are compared with the last full runs (Windows of 2026-11-13,
    Linux of 2026-11-12), then the checksums of W26 and W27 match in both engines; the ratio of W27 to Voron goes up in
    both, unless a measurement of W27 alone, with its repetitions, on the same machine, with and without the
    projection, shows that the projection lowers its time; and no other load, W26 included, lowers its ratio to Voron
    by more than 0.05, unless the median of a measurement of that load alone, with its repetitions, equals that of the
    previous run within that margin. A K7 threshold that is still missed is not a failure of this PRD: K13 applies and
    it is reported at the close.

    *Annotation of 2026-11-16, from the code review of the execution (user's decision): the condition of the
    measurement of a load alone was not met in nine loads (W1, W9, W12, W13 and W23 on Windows; W9, W18, W21 and W26
    on Linux). In its place a comparison of Krummi alone, before and after this change, load by load, with the rounds
    in alternating order and the same start stores, is accepted; it did not show that the change lowered any load, and
    its checksums match. The checksums of W26 and W27 match in both engines, and the ratio of W27 to Voron went up on
    both systems.*
12. **AC-PY12** — Given a store written before this PRD, when it is opened and its tables are read with projections,
    then it gives the same values as reading their columns by their position, without migrating anything.
13. **AC-PY13** — Given E6, E17, E19, the *Out of scope* and the *Benchmark loads* of
    [tables-schema.prd.en.md](tables-schema.prd.en.md), N10 of [tables.prd.en.md](tables.prd.en.md) and K7 of
    [benchmark.prd.en.md](../benchmark/benchmark.prd.en.md), in the original and in its translation, when they are
    read, then they carry the annotation of this PRD.

## Annotations to other PRDs

This PRD changes rules of earlier documents, which carry these annotations of 2026-11-15 (in their translations too):

- [tables-schema.prd.en.md](tables-schema.prd.en.md), *Out of scope*, "Reading only some columns of a row
  (projections): module 17": *«Column projections come earlier, in
  [tables-projection.prd.en.md](tables-projection.prd.en.md): a list of columns prepared once per schema, kept in the
  store, with which the rows are read; the queries of module 17 will decide whether they use them.»*
- [tables-schema.prd.en.md](tables-schema.prd.en.md), E6: *«Also failing with different schema are preparing a
  projection with a name that is not that of a live column (with the table and the column), obtaining a prepared column
  with a type of another class (with the table, the column and the difference) and tying to a projection a row of
  another table, of another store or of another schema (with the table) (PY1–PY3 of
  [tables-projection.prd.en.md](tables-projection.prd.en.md)).»*
- [tables-schema.prd.en.md](tables-schema.prd.en.md), E17: *«The list of invalid value rules adds at the end
  projection without columns, and the precedence holds for preparing a projection as PY1 of
  [tables-projection.prd.en.md](tables-projection.prd.en.md) says.»*
- [tables-schema.prd.en.md](tables-schema.prd.en.md), E19: *«Preparing a projection, obtaining its prepared columns,
  tying a row to it and reading with it do not fail with damage: the damage of the row and of the history appears in the
  read or in the step of the traversal that returns it (PY3 of
  [tables-projection.prd.en.md](tables-projection.prd.en.md)).»*
- [tables-schema.prd.en.md](tables-schema.prd.en.md), *Benchmark loads*, W26 and W27: *«Krummi reads the columns of the
  rows with a projection, and preparing the projection counts in the measured time of each repetition; so will the
  loads of future tables that read column values, unless their PRD says otherwise. The loads, their checksums and their
  thresholds do not change (PY7 of [tables-projection.prd.en.md](tables-projection.prd.en.md)).»*
- [tables.prd.en.md](tables.prd.en.md), N10: *«Different schema also covers the uses of a projection that depend on the
  schema (PY1–PY3 of [tables-projection.prd.en.md](tables-projection.prd.en.md)).»*
- [benchmark.prd.en.md](../benchmark/benchmark.prd.en.md), K7: *«In W26 and W27, and in the loads of future tables that
  read column values unless their PRD says otherwise, Krummi reads its rows with a projection, the read with the real
  capabilities of the engine, as in W28, and preparing the projection counts in the measured time of each repetition;
  Voron reads its fields by their position. The loads and the thresholds do not change (PY7 of
  [tables-projection.prd.en.md](../tables/tables-projection.prd.en.md)).»*

## References

- Parent PRD: [tables-row-reads.prd.en.md](tables-row-reads.prd.en.md) (L1, L2, AC-L3, AC-L5, AC-L9); its parent,
  [tables-schema.prd.en.md](tables-schema.prd.en.md) (E2, E6, E11, E12, E16, E17, E18, E19, E20, *Benchmark loads*); its
  parent, [tables.prd.en.md](tables.prd.en.md) (N3, N4, N10, *Output*); the root,
  [krummi.prd.en.md](../krummi.prd.en.md) (D8, D11).
- Trees: [trees.prd.en.md](../trees/trees.prd.en.md) (A6, B7, B8).
- Transactions: [transactions.prd.en.md](../transactions/transactions.prd.en.md) (T6, T8, T10).
- Benchmark: [benchmark.prd.en.md](../benchmark/benchmark.prd.en.md) (K7, K13).
- Performance diagnosis of 2026-11-15,
  [informe-rendimiento.md](../../../performance/2026-11-15-diagnostic/informe-rendimiento.md), third round (shared
  prepared projection), with the A/B tests of W26 and W27, the costs of preparing and of checking, and the tests of the
  prototype.
