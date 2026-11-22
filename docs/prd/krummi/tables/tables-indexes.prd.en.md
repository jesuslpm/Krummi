# Secondary Indexes of Tables — PRD

**Version**: 1.0
**Date**: 2026-11-16
**Parent document**: [Krummi Tables — PRD](tables.prd.en.md) (v1.0)

> English translation of [tables-indexes.prd.md](tables-indexes.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Build child PRD **7.2** of the [split of the tables module](tables.prd.en.md#split-into-child-prds): create and drop
unique and non-unique indexes on a table, also one that has rows; maintain them on every write; traverse a table by an
index, returning its rows or only the entries of the index; included columns, descending columns and the order of nulls
(N5, N7 of the parent); and the benchmark loads that compare the indexes with those of Voron (N11).

**Why now**: 7.1 and its two children (reads without checking the content, and projections) are closed, and N2 of the
parent builds the child PRDs in order: statistics (7.3), JSON columns (7.5), vector indexes (7.6) and queries (module
17) rest on the secondary indexes.

The technical design is already decided in the design documents of the tables, which are the technical starting point
of this PRD: [document 3](../../../design/tables/03-keys-and-indexes.md) (key encoding, structure of the index,
uniqueness probe), [document 2](../../../design/tables/02-row-storage.md) (creating an index over rows that already
exist, H15) and [document 11](../../../design/tables/11-query-contract-reconciliation.md) (identity of each index and
state of the indexes, P12 and P17).

This PRD inherits everything from its ancestors. It makes precise or extends rules of its parent, of the root, of
[tables-schema.prd.en.md](tables-schema.prd.en.md) (7.1), of its two children, of the pager and of the benchmark, which
carry its annotation (*Annotations to other PRDs*).

Decisions of the user of 2026-11-18, on the proposals of the draft: sort the entries of a new index in «Memoria y
archivos temporales», in the «Carpeta del almacén» (memory and temporary files, in the store folder); the limits «32
columnas, 64 índices» (32 columns, 64 indexes); «Rechazar los dos» (reject both) redundant indexes (the one that repeats
the primary key or a prefix of it, and the one that repeats the key of another); return the key of an entry
«Decodificadas en una copia» (decoded into a copy); «Los dos modos» (both modes) of creating an index; for malformed
rows when deleting and when creating an index, «No comprobar, como las lecturas» (do not check, like reads); the format
«Incompatible, formato 12» (incompatible, format 12); in a temporary store, «Solo memoria en modo temporal» (memory
only in temporary mode), to meet G9; and a panic caught while deleting a range leaves the write unable to be committed:
«Ya no puede confirmarse» (it can no longer be committed). And, after the Codex review, a column of the primary key
among the included ones of an index: «No falla, pero no hace nada, la clave primaria siempre está incluida» (it does
not fail, but it does nothing, the primary key is always included).

In this document, the **key of the index** is the ordered list of the columns by which it is sorted and searched, each
with its direction and its order of nulls; its **included columns** are those it stores in addition so as to return them
without reading the row; and an **entry** is what the index stores for each row of the table: the values of its key,
those of its included columns and the primary key of the row.

### Product decisions

| ID | Decision |
|---|---|
| IX1 | **Definition of an index**: a name, whether it is unique, its key (from 1 to 32 distinct columns, each one ascending or descending and with its nulls first or last; by default, ascending and with nulls first, N5) and its included columns (from 0 to all the other live columns, distinct from each other and from those of the key, in the order they are given). The order of nulls is part of the definition also in a column that does not admit nulls. The name of an index follows the rules of column names (E4): UTF-8 text from 1 to 1,024 bytes, compared by its bytes; it is unique within its table and takes no name of the catalog (N1). A table has at most 64 indexes. A column of the key may be of any type of N4 except vector (and JSON from 7.5, annotation of 7.5 to the parent), with the order of N5 for its type, also that of SQL Server in a `uuid` that declares it; an included column, of any type, vectors included (makes N5 of the parent precise). The columns of the primary key may be in the key of the index. Among its included columns they are admitted, but do nothing, because every entry already carries the whole primary key: the definition is stored without them, Describe does not show them among the included ones (IX6), Create if it does not exist compares the definitions without them (IX2) and an entry returns them as columns of the primary key (IX8); the rules of IX3 are checked before, on the definition as given. The values of an entry are those of its row as E11 would read them, so a row stored before adding a column has in the index its default value or null. The stored form of the definition reserves the condition of a partial index (N7), which the API of this PRD does not offer; the design review checks it, not a test. Indexes work the same in a temporary store, except where their entries are sorted when creating them (IX2). |
| IX2 | **Create an index**: only in a write (in a read the operation does not exist), on an open table, with its definition, in one of two modes. **Create new** fails with tree already exists, with the table and the index, if the table already has an index with that name. **Create if it does not exist** does nothing if there already is an index with that name and the same definition (IX1: uniqueness, columns of the key with their direction and their nulls, and included columns in the same order, without those of the primary key) and the given definition meets IX3; if it is the same but the given one breaks IX3 (for example, with a column of the primary key repeated among its included ones), it fails with invalid schema, as in IX3; and it fails with **different schema** if its definition is another, with the table, the index and the first difference, with what was expected and what was found as in E6, in this order: the uniqueness; the columns of the key, position by position (the column, its direction and its nulls), and their number; and the included ones, position by position, and their number; within each part, the positions that both have before their number. It is called this, and not Open or create as in A1, because an index is not opened. Neither mode returns anything. Creating an index on a table with rows reads all its rows, sorts its entries and writes the index inside the same write, like any operation of a write (G4). The entries are sorted in the memory of the process and, when they do not fit in what the implementation reserves for it, also in temporary files inside the **`krummi.sort`** subfolder of the store folder, which is created only when needed. `krummi.sort` follows the rules of `krummi.temp` (J2, J8, J14): it is a reserved name (P3), so what is inside belongs to the store and P7 does not apply; a folder with `krummi.sort` and without `krummi.data` is not a store; the creation of the index deletes it when it finishes, also when it fails or a panic interrupts it; if it cannot delete it, it stays, without changing the result of the creation; closing the store, or ceasing to use it without closing it, deletes it; and the one left by a crash, or by a panic with a build that aborts, is deleted whole by the next open in its step 8, after checking the journal, with everything it contains and also if it is a file, whereas an open that fails does not delete it and one that cannot delete it fails with the access class. Its files do not count toward the maximum size (P10) nor in the information of the store (P12), and are never read to recover. A failure creating, writing or reading them makes the creation fail with its class (access, disk full of G7, out of memory of P13, or input/output of G12), with T11, without changing the store (IX11); an input/output failure when writing them also follows T12, as in J7. In a temporary store no file is created (G9): the entries are sorted only in memory and, if the system does not give it, the creation fails with out of memory (P13), with T11. Writing the pages of the index may fail like any write (G7, P10, P13, G12), with T11. A created index is seen from that moment in all the openings of the table of that write (B9) and, on commit, in later transactions. The definition of a table when it is created (E1) carries no indexes: they are created afterwards, in the same write or in another. An index can be created with the name of one dropped earlier, also in the same write and with another definition, and it is a new index, with another identity (IX6). |
| IX3 | **Invalid definitions**: creating an index fails without effect with **invalid schema** when the definition breaks a rule, with the table, the index, the column if there is one and the rule, which is one of these, in this order: no key columns; a column that does not exist; a vector (or JSON, from 7.5) column in the key; a column repeated in the key, in the included ones or in both; more than 32 key columns; a key that is the whole primary key, or, in a non-unique index, a prefix of it, with its columns in the same order and all ascending, whatever nulls and included columns it has (a unique index on a strict prefix is admitted: it is a constraint); a key equal to that of another index of the table (the same columns in the same order, with the same direction and the same nulls), whether it is unique or not and whatever included columns it has; more than 64 indexes; and the schema too large of the annotation of 2026-11-11 to E5, which from this PRD counts also the definitions of the indexes, naming the index that passes it and no column. If several are broken, the first in the order of this list is reported and, within it, the first column in the order in which the definition gives them (the key and then the included ones), not in the order of the schema as in E5; in the repeated column, the first appearance that repeats an earlier one in that order, like step 1 of E17. The schema too large counts, for each index, its name and its definition as they are stored. An index name that is not 1 to 1,024 bytes long fails with invalid size (B8), with the names (E17). As the definitions count toward the schema too large, adding a column or renaming it to a longer name may fail with that rule in a table with indexes (E5). |
| IX4 | **Drop an index**: only in a write, on an open table, by its name; it removes the index and frees its pages, and returns nothing. If the table has no index with that name, it does nothing, like Delete in A3. Deleting the table, also when overwriting it when renaming another, deletes its indexes (E2); emptying it, with Empty or with Create or empty, removes its entries and keeps their definitions, their identities and the state of the indexes (E3); renaming it keeps them. An index cannot be renamed nor its definition changed: it is dropped and created again. |
| IX5 | **Indexes and schema changes**: an index names its columns by their identity (E2), so renaming a column does not affect it and Describe shows the new name. Removing a column from the key or from the included columns of an index fails without effect with invalid schema and the rule that E5 already has, «removing a column of the primary key or, from 7.2, of an index», with the table and the column, and in addition, if the column is not of the primary key, the index with that column that comes first in the order of B5. Allowing nulls in a column of an index is admitted (N3); its nulls follow the order of nulls of that column of the index and, in a unique one, do not collide with each other (N7). Creating or dropping an index changes neither the version nor the schema state (E2), so projections stay valid (PY4). |
| IX6 | **Describe** (extends E2): it returns in addition the list of the indexes of the table as its transaction sees them, empty if it has none, in the order of their names according to B5, each one with its identity, its name, whether it is unique, the columns of its key (each one with its identity, its name, its direction and its order of nulls, as they were defined) and its included columns (each one with its identity and its name), in their order; and the **state of the indexes** of the table (design document 11, P12). The identity of an index and the state of the indexes are random 64-bit values drawn like those of E2 and drawn again if they match one that the store knows. The state of the indexes is drawn when the table is created and again each time an index of it is created or dropped, and with nothing else: not with an attempt that fails or does nothing, nor when renaming the table or a column, nor when emptying it; an aborted write leaves none of its draws. The columns are named with the name they have in the schema that the transaction sees. List does not change: the indexes are not objects of the catalog. |
| IX7 | **Maintenance**: every write of a row (insert, update, insert or update, delete, delete a range, bulk load) leaves all the indexes of the table in agreement with their rows in the same transaction (N7, G1), also in an index created earlier in the same write. All the checks (values, sizes, primary key and each unique index) are made before changing anything, also when updating. An identical update (E10) changes no index nor is it a change for T10 (E16). In a unique index, two rows collide when their columns of the index key have the same values, compared after the normalization of N4 (N5), and none is null; with some null, they never collide (N7). Storing a row that collides with another in a unique index fails without effect with **duplicate key** (IX10). A bulk load on a table with indexes follows E13, with all its rules: a row that collides, also with an earlier row of the same load, aborts the whole load. |
| IX8 | **Traverse an index**: in a read or write transaction, a range (IX9) of an index is traversed forwards, in the order of its key (N5, with the direction and the nulls of each column; entries with the same key, also those with nulls, in the ascending order of their primary key), or backwards, in the exact inverse order. It is the only operation that reads by an index: looking for the rows of a value is traversing the range with that value in all the columns of the key, which in a unique index without nulls gives at most one row. The traversal returns, at the choice of the application, **rows** or **entries**. The rows are those of the table, with the form of E11, read as in L1 and L2 and tied to a projection like those of a primary key traversal (PY3). The entries are read without reading the rows: each one gives the values of the columns of the index key, of its included ones and of the columns of the primary key that are not in the index key, accessible by their name or by their position in that order (the index key in its order, the included ones in their order and the rest of the primary key in its order), each column only once. The values have the normalization of N4 and E7 (a `uuid` in the RFC 9562 byte order also in a column with the SQL Server order). Those of the included columns are returned like those of a row: without copying the text, the bytes, the vectors and the JSON (D11); those of the columns of the index key and of the primary key, decoded into a copy owned by the application, the text and the bytes too (exception to D11 and to the *Output* of the parent). An entry is not tied to a projection, and there are no copies of entries that outlive the reference. Traversing an index is not a change for T10 nor does it end references (E16); the rows and entries it lends stop being valid at the next operation that can modify the transaction (B9, E16, IX13). |
| IX9 | **Ranges of an index** (like E12): a range gives values for the first k columns of the index key (the prefix, from 0 to all of them) and, if k is smaller than the number of columns of the key, optionally a lower bound and an upper bound on the next column, each one included, excluded or absent. A value of the prefix may be null if the column admits nulls, and then it asks for the entries whose column is null. The bounds are of values, in the order of N5 of the type of the column, whatever its direction in the index: the lower is the smaller value and the upper the greater, and a forward traversal of a descending column goes from the upper to the lower. With some bound, the entries whose column is null are not in the range, as in an SQL comparison; with no bound, they are. The values of the prefix and of the bounds are normalized (N4, E7) and rounded (E8) like those of a row, so `-0` finds `+0` and any NaN, the stored NaN. An empty or inverted range with valid values returns nothing and is not an error. The following fail with **invalid value**: a value of a type that is not that of its column (rule type different from that of the column), outside its domain (rule outside the domain), a null in the prefix of a column that does not admit nulls (rule null not admitted), and a null as a bound, a prefix with more columns than the index key or a bound when the prefix already gives all the columns (rule invalid range); NaN is a valid value (N5). With several failures, a prefix with more columns than the key or a bound with the complete prefix are reported before the values; then, the first value that fails (the prefix in the order of the key, the lower bound and the upper), with the first rule it breaks in the order of E17. A prefix with any of its bounds that, encoded as in an entry, passes 2,048 bytes fails with **invalid size**, with the data of B8 (what breaks its size: a prefix; the size received; `1` and `2048`), the table and the index, even if the range is empty. Traversing an index that does not exist fails with **different schema**, with the table and the index. There are no bounds on more than one column at a time nor on the columns of the primary key of the entries. |
| IX10 | **Errors: data and precedence** (extends E17): a repeated primary key fails as in 7.1, with the table and the primary key and without an index. A repeated unique index fails with **duplicate key** with the table, the primary key of the row being stored, the index and the values of the columns of its key that repeat; when creating a unique index over rows that repeat values, the primary key is that of the second of the first two rows that repeat them in the order of the index, and in a bulk load, that of the row of the load that collides, the first in the order of the load. When storing a row, the repeated primary key is reported before an index, and among indexes, the first in the order of their names according to B5. An entry larger than its maximum fails with **invalid size**, with the data of B8 (what breaks its size: an index entry; the size received; `1` and `2048`), the table and the index; the size of an entry is that of the values of the index key followed by the primary key of the row, in the form in which Krummi stores keys (E18, design document 3), in which a descending column or one with nulls last may take some more bytes; the included columns do not count. When storing a row, the invalid size of its entries comes after that of the primary key and that of the row, among indexes in the order of their names, and in the place of E17 for the size that the schema needs; in an update or an insert or update of a row that exists, after reading it, like that of the row (the exception of E17). Every operation on indexes follows the precedence of E17: first the invalid size of the names (that of the table, if there is one, and that of the index); then tree not found of an opening of a deleted table; then tree already exists (Create new), or different schema (Create if it does not exist with another definition, or traversing an index that does not exist); then invalid schema (IX3); then invalid value and the invalid size of a prefix (IX9); then the failures reading pages and, when creating an index, those of the temporary files (IX2), the first that occurs; and when creating an index, after reading all the rows, the invalid size of the entries, that of the first row in the order of the primary key, and last duplicate key. |
| IX11 | **Creating a unique index over rows that repeat values** fails without effect with duplicate key (IX10) before changing anything in the store (its pages; the temporary files of IX2 are not the store's): the catalog, the rows, the other indexes, the used and free pages and the information of the store (P12) stay as without the attempt, and the transaction can commit its other changes (H15 of the design). The same when it fails because of the size of an entry. |
| IX12 | **Content and damage** (extends E19, L1–L3 and the exception of G6 of the root for rows): traversing an index does not check the content of the entries nor of the rows: a malformed entry in a page whose integrity passed G6 is returned without error, and reading one of its values may give a value that is not the one written or end with a normal panic of the thread, like a row (L2). When asking for rows, an entry whose row is not in the table fails with damage of the index page that has the entry. When maintaining an index, an entry that the write must remove and is not there fails with damage of the index page where it should be, and the uniqueness check decides with the entries, without reading the row of the other. Update and insert or update keep checking the row they replace (L3). Deleting a row, deleting a range and creating an index, in a table with indexes, read the values of the rows that already exist to derive their entries without checking their content, like reads (L1, user decision), although they keep failing with damage in the cases that L1 maintains (an unknown schema version or one missing from the history, a header that Krummi does not write, a row that does not reach the end of its fixed part). A malformed row may give entries that are not those of its written values, make the entry of another row be removed without error, make the entry to be removed missing (and then it fails with damage, above) or end with a normal panic of the thread (L2); what each malformed content gives is left to the implementation, as in L2. A panic when deleting a row or creating an index arrives before changing anything, and the transaction goes on as L2 says; one when deleting a range may arrive with part of the range deleted, and then, if the application catches it, the write can no longer be committed, as with damage (T11; exception to L2, user decision). Describe and dropping an index that does not exist never fail with damage in the pages of the indexes; traversing fails with damage in a page it needs; creating an index may fail with damage in any page of the table; dropping an index, in any page of that index. In a write, every failure due to damage follows T11: the write can no longer be committed, even if part of a range was deleted before finding the damage. |
| IX13 | **Changes and references** (extends E16): creating an index and dropping one that exists are changes for T10, also when it is created and dropped in the same write. They are not: create if it does not exist on an equal index, dropping one that does not exist, traversing an index and every operation that fails with a public validation error. Creating and dropping an index end the earlier references, even if in the end they change nothing or fail. |
| IX14 | **Format**: the catalog stores the definitions of the indexes, their identities and the state of the indexes, which an engine of format 11 does not know how to read, so the change is incompatible (B12) and the format version becomes 12: a store of format 11, even without indexes, is of an earlier format and this engine does not open it (F11). The information of the store (P12) does not change its form: the pages of the indexes count in its totals. |
| IX15 | **Benchmark loads** (N11, K7): the three loads of *Benchmark loads* (below), against the Voron tables with their indexes, with the threshold of K7; the other engines say `no aplica`. The report counts 32 thresholds. |

#### Benchmark loads

They use the table of the *Benchmark loads* of 7.1, with two non-unique indexes: `ix_n` on `n` and `ix_ts` on `ts`,
ascending and without included columns. The rules of those loads apply (the data of the seed of K4, the equivalent
schema in Voron, without inheriting changes from another load) and so does PY7: Krummi reads the columns of the rows with
a projection. In Voron, the two indexes are defined in its `TableSchema` and store their fields so that the order of
their bytes is that of Krummi, so that both engines return the same rows in the same order.

| ID | Name | Starts in | What it does | Operations | Reduced |
|---|---|---|---|---|---|
| W30 | `tabla-insercion-con-indices` | Empty store with the table and its two indexes | Inserts 1,000,000 rows with `ulid` keys in order, in writes of 1,000, like W23. | 1,000,000 | 10,000 rows |
| W31 | `tabla-busqueda-por-indice` | The table of W30 | In a read transaction, looks up in `ix_n` the value of `n` of each row, in a random permutation of the rows (K4), with the range of that value (IX8), and reads all the columns of the row it returns; each lookup checks that it returns exactly one row. | 1,000,000 | 10,000 lookups |
| W32 | `tabla-intervalos-de-indice` | The table of W30 | In a read transaction, 100,000 forward ranges of `ix_ts`, each one from an existing `ts` value chosen at random, included, that read up to 100 rows with all their columns (fewer if they reach the end), like W27; rows with the same `ts` come in the order of their primary key (IX8). | The rows read | 1,000 ranges |

W31 and W32 start in the store that W30 leaves, each one in its own copy (K4). The size of the store (K9) is also
measured after W30. W31 and W32 compute in both engines the checksum of W26 and W27 and compare it with the expected one.

*Annotations of 2026-11-22 from the temporal values in microseconds ([tables-temporal-micros.prd.md](tables-temporal-micros.prd.md), Spanish, authoritative):*

- *IX9: a temporal value of a bound of an interval is already in microseconds and is not rounded (TU3).*

### Starting state

Verified on 2026-11-16 in the code and in the design documents:

- There are no secondary indexes: the entry of a table in the catalog stores its identity, the root and the number of
  rows of its row tree, the root of its schema history, the version and the state of the schema, its columns and its
  primary key, with no fields reserved for indexes; Describe returns nothing about indexes; there are no index
  identities nor state of the indexes.
- The rule of E5 «removing a column of an index» exists only in the documentation of the error: the code checks only the
  columns of the primary key.
- Duplicate key carries the table and the primary key, without an index. Invalid size distinguishes name, key, value,
  prefix, column name, primary key and row, without an index entry.
- Insert, update and insert or update check everything before changing anything; update knows which columns it gives and
  decodes the row it replaces; deleting a row and deleting a range do not decode it.
- The bulk load inserts row by row in a write of its own, fast when the keys arrive in order; there is in the engine no
  external sort nor temporary sort files.
- The ranges of the primary key have a prefix and bounds (E12), forwards and backwards; the traversal lends its rows,
  which are tied to a projection (PY3).
- The format version is 11. The information of the store counts pages, not trees.
- The benchmark has the table loads W23–W29 and counts 29 thresholds; the Voron harness defines only the primary key of
  its table, without indexes.

## Actors

- **Application developer (Rust)**: creates and drops indexes, stores rows in tables with indexes and traverses them by
  an index.
- **Whoever runs the benchmark**: compares loads W30–W32 with Voron.

## Flow

1. The application creates the table (E1) and, in the same write or afterwards, its indexes with Create if it does not
   exist (IX2).
2. It inserts, updates and deletes rows as in 7.1; Krummi maintains the indexes (IX7).
3. It traverses a range of an index and obtains its rows, which it reads with a projection, or only its entries (IX8,
   IX9).
4. To load many rows, it loads the table without indexes and creates them afterwards (N6 of the parent), or loads with
   them.

## Validations and messages

Those of the ancestors apply. This PRD adds, only in the library, in the order of their precedence (IX10):

| Situation | Error class |
|---|---|
| An index name that is not 1 to 1,024 bytes long | Invalid size (B7, B8) |
| An operation on indexes through an opening of a table that the write deleted | Tree not found (A6, E17) |
| Create new an index with a name that the table already has | Tree already exists (A6), with the table and the index |
| Create if it does not exist an index with a name that the table has with another definition | Different schema (E6), with the table, the index and the difference |
| Traversing an index that the table does not have | Different schema (E6), with the table and the index |
| An index definition that breaks a rule of IX3; removing a column of an index | Invalid schema (E5) |
| A malformed range of an index (IX9) | Invalid value (E17) |
| A prefix with its bound that does not fit in an entry; an index entry larger than its maximum | Invalid size (B7, B8), with the table and the index |
| Storing a row that repeats, without nulls, the values of a unique index of another row; creating a unique index over rows that repeat them | Duplicate key (N10), with the index (IX10) |
| A failure creating, writing or reading the temporary files of the sort, or memory that falls short in a temporary store (IX2) | Access, disk full (G7), out of memory (P13) or input/output (G12), with T11 and, when writing, T12 |
| Opening a store with a `krummi.sort` that cannot be deleted | Access (J8) |

Dropping an index that does not exist and traversing an empty range are not errors.

## Output

The module writes nothing on screen. Describe returns in addition the list of indexes, empty if there are none, and the
state of the indexes (IX6), in a copy owned by the application. Traversing an index returns rows with the form of E11 or
entries with that of IX8. Creating and dropping an index return nothing. Duplicate key and invalid size add the index to
their data when the failure is of an index. The reads, traversals and writes that already exist do not change their
output. The benchmark report adds rows W30–W32 with their thresholds, with `no aplica` in the engines that are not
Voron, and its table of sizes adds W30.

## Reuse

- The key encoding, the ranges of the primary key and their semantics (E12), and the two-phase check of the writes of
  7.1 (E10, E17).
- The B+ trees, their traversals and their rule of large values outside the leaf (B6) for the entries and their included
  columns.
- The random identities of the catalog and their draw (E2, P12, P20).
- The reads without checking the content (L1, L2) and the projections (PY3, PY7).
- The comparative benchmark, its seed, its table of 7.1 and its report (module 6, *Benchmark loads* of 7.1).

## Out of scope

- Partial indexes (reserved, IX1), on expressions, on JSON paths, multi-entry and inverted ones (design document 14),
  and approximate vector indexes (7.6).
- Declaring the indexes in the definition of the table when creating it (IX2).
- Renaming an index or changing its definition: it is dropped and created again (parent).
- Creating an index in several transactions or without occupying the write while it is built; creating several indexes
  in a single read of the table.
- Configuring or reading the memory or the folder of the sort of a new index, and sorting on disk in a temporary store
  (IX2).
- Canceling the creation of an index, reporting its progress or giving it a time limit of its own.
- Rebuilding an index: it is dropped and created again.
- A copy of an entry that outlives its reference (IX8).
- Encrypting the temporary files of the sort: confidentiality at rest is delegated to the file system (annotation of
  2026-11-09 to the root).
- A lookup operation by an index other than traversing a range (IX8).
- Deleting rows by a range of an index; counting the entries of an index or of a range; the size of an index in Describe
  or in the information of the store; estimates (7.3).
- Bounds on more than one column at a time, or on the columns of the primary key of the entries, and choosing an index
  automatically: module 17.
- Tying the entries of an index to a projection (IX8).
- Checking that an index agrees with its table: the verification of module 12 treats its trees as trees (N1).
- Benchmark loads that update or delete rows of a table with indexes or that use a unique index.
- The SQL Server layout for unique indexes (the primary key only in the leaves), which design document 3 leaves as an
  optimization to measure.
- Collations and case-insensitive text comparison (parent); descending primary key columns (7.1).

## Left to the implementation

- How the indexes and their definitions are stored in the catalog and in trees, with the starting point of design
  documents 2 and 3; the form of an entry that measures its size (IX10) is that of document 3.
- The names of the operations, the types and the error classes, in English and with the conventions of Rust (D8).
- How the entries are sorted when creating an index, how much memory it uses before moving to the temporary files, and
  the names and the content of the files inside `krummi.sort`.
- How the Voron harness stores the indexed fields so that their byte order is that of Krummi.
- How the test hooks of the criteria are built: they are test mechanisms, not public contracts.

## Acceptance criteria

Those of the ancestors apply. This PRD meets AC-N7 of the parent, the cases with indexes of AC-N3 and of AC10 of 7.1,
and AC-N9 and AC-N10 with what these criteria fix.

1. **AC-IX1** — Given a table with rows, columns of each type of N4 that can be sorted (`uuid` in both orders, `-0`,
   NaN with different bits and the extremes of each domain of E8), columns added after storing rows, with a default
   value and without it, and keys at the size limit, when unique and non-unique indexes are created, ascending and
   descending, with nulls first and last, with each combination of direction and order of nulls in consecutive columns,
   also behind `bytes` columns with the zero byte and `string` columns with U+0000, and with included columns (also a
   vector), then each index has the entries of the rows that were already there, in the order of N5, with the default
   value or null in the added columns (AC-N7).
2. **AC-IX2** — Given a table with several indexes, one created in the same write, when a write inserts, updates (giving
   only columns that are in no index, only one of an included column and of the key of an index, also on a row of an
   earlier version of the schema), inserts or updates, deletes rows, deletes a range and bulk loads, and is committed,
   and in other cases is aborted or a process crash is simulated and it is reopened, then each index has exactly the
   entries of the rows of the table; an identical update is not a change for T10 and writes no page of the indexes,
   according to the test hook; and two rows with nulls in the columns of a unique index do not collide (AC-N7).
3. **AC-IX3** — Given the indexes of AC-IX1, with rows that repeat the index key, also with nulls, when they are
   traversed by each index forwards and backwards, asking for rows and asking for entries, then the rows, also read with
   a projection, or the entries, with the values of IX8 accessible by name and by position, come back in the order of
   IX8 (the repeated ones in the order of their primary key, and the reverse backwards), also with a column of the
   primary key in the index key and another, a text one, given among the included ones (which the entry returns only
   once, in its position of the primary key and in a copy), the text, the bytes and the vectors of the included columns
   without copying, those of the key in a copy, and the `uuid` of SQL Server order in the RFC 9562 byte order (AC-N7).
4. **AC-IX4** — Given indexes on ascending and descending columns with nulls first and last, when ranges are traversed
   with each prefix, with a null in the prefix, with `-0` and NaN in the prefix and in the bounds, and with each
   combination of included, excluded and absent bounds, forwards and backwards, then exactly the entries of IX9 come
   back; an inverted range returns nothing; each malformed range of IX9 fails with invalid value and its rule, and a
   prefix that does not fit, with invalid size and its data, even if the range is empty; and traversing an index that
   does not exist fails with different schema (AC-N7).
5. **AC-IX5** — Given the property test of AC-N1, with random indexes of one to four columns of any direction and order
   of nulls, unique and non-unique, when rows are stored, updated and deleted at random, then the order of the
   traversals of each index, the entries of each range and the duplicate keys are those of the reference model of N5
   (AC-N7).
6. **AC-IX6** — Given a table with several unique indexes, when a row is inserted and updated that repeats, without
   nulls, the values of each one in turn, of two at a time, and its primary key and one index at a time, then it fails
   without effect with duplicate key and the data of IX10, the one that IX10 says when there are several; when the entry
   of a row is too large, because of a `string` at the limit that fits in an ascending index and not in a descending
   one, then it fails with invalid size and its data, also if the row also repeats a unique index; and in all cases the
   row, the indexes and the number of rows stay as they were and the transaction can commit its other changes (AC-N7).
7. **AC-IX7** — Given a table with rows that repeat the values of a column, when a unique index is created on it and the
   write commits other changes, then it fails with duplicate key and the data of IX10, the index does not exist, the
   table and its other indexes do not change, and the used and free pages and the information of the store (P12) are
   those there would be without the attempt; and when a row has an entry that is too large, then it fails with invalid
   size, equally without effect (AC-N7, IX11).
8. **AC-IX8** — Given a table with indexes, when a bulk load has, before committing, a row that collides in a unique
   index with a row of the table or with an earlier one of the same load, one with an entry that is too large, one that
   fails being the last, a source that fails, a cancellation or a panic, then the table, its indexes, the number of rows
   and the sequences stay as before and another write gets its turn afterwards; and when 100,000 rows are loaded out of
   order, then the table and its indexes have the same rows and entries as if they had been inserted one by one (AC10 of
   7.1, AC-N3).
9. **AC-IX9** — Given each case of IX3, and an index name of 0, 1, 1,024 and 1,025 bytes, when the index is created, then
   those inside work and the others fail without effect with invalid schema or invalid size and their data; with two
   rules broken, the one that IX3 says is reported; a table with 64 indexes does not admit another, and one close to
   the maximum of its catalog entry, according to the test hook that measures it, fails with schema too large when
   creating an index and when adding a column; Create new on a name that exists fails with tree already exists; Create
   if it does not exist on an equal one does nothing, also when one of the two gives among its included columns columns
   of the primary key, which Describe does not show (IX1), and on one that differs in each part of IX2, separately and
   in two at once, it fails with different schema and the first difference; and the one that IX10 says wins in each of
   these pairs: a long index name from an opening of a deleted table (invalid size); a name that exists with an invalid
   definition, in Create new (tree already exists) and in Create if it does not exist (different schema, if it differs
   from that of the index without the columns of the primary key among its included ones; if it does not differ, as with
   the included `[id, id]` over an index without included columns and the primary key `id`, invalid schema with the
   repeated column); an index that does not exist and a malformed range (different schema); a malformed range and a
   prefix that does not fit (invalid value); an entry that is too large and a duplicate, when storing and when creating
   (invalid size); the primary key and an index repeated (the primary key); and two indexes repeated (the first by its
   name).
10. **AC-IX10** — Given a table with indexes, when a column of an index is renamed and its nulls are allowed, then its
    traversals return the same entries in the same order and Describe shows the new name with the same identity; when a
    column of its key, another of its included ones and one of the primary key that is in an index are removed, then it
    fails without effect with invalid schema and the rule of IX5; when an index is dropped, then it disappears, the table
    does not change and another index can be created with its name and another definition, also in the same write; when
    the table is emptied with Empty and with Create or empty, renamed, deleted and overwritten when renaming another,
    then its indexes are left empty with their definitions, are kept, disappear and disappear; and another opening of
    the table in the same write sees a created index and stops seeing a dropped one.
11. **AC-IX11** — Given a table, when indexes are created and dropped in a write that commits and in another that
    aborts, also one created and dropped in the same write, then Describe returns those of IX6 as each transaction sees
    them, in a read and in a write, and an empty list in a table without indexes; the state of the indexes changes with
    each creation and each drop and not with an attempt that fails, one that does nothing, renaming the table or a
    column nor emptying it; the identity of an index does not change when renaming the table or its columns, and one
    created again has another; with a test generator that repeats a value, the identity and the state are drawn again;
    the schema state and version do not change; and a projection prepared earlier keeps reading the rows. Creating and
    dropping indexes in a read is not possible.
12. **AC-IX12** — Given the operations of IX13, when they are run in a write that does nothing else, then exactly those
    that IX13 says are changes for T10, and creating and dropping an index end the earlier references.
13. **AC-IX13** — Given a malformed entry in a page with correct integrity, when it is traversed, then the step does not
    fail, the other entries come back with their values and reading the malformed value gives a value or ends with a
    panic (L2); given an entry whose row is not in the table, when rows are asked for, then it fails with damage of the
    index page; given a row whose entry is missing in an index, when it is deleted, then it fails with damage of the
    index page; given a malformed row in a table with indexes, when it is updated, then it fails with damage of its page
    and the write follows T11; given a row whose altered bytes change only a column that is in no index, when it is
    deleted, a range that includes it is deleted and an index is created on other columns, then they do not fail and
    the indexes agree with the other rows; given a row whose altered bytes change the value of a column of an index to
    one that has no entry, when it is deleted, then it fails with damage of the index page; given a row of a schema
    version that is missing from the history, when it is deleted or an index is created, then it fails with damage, as in
    AC-L3; given a panic caught when deleting a range, the write cannot be committed and applies nothing; and given a
    damaged index page, Describe does not fail and traversing the index fails with damage (IX12).
14. **AC-IX14** — Given a table whose entries do not fit in the memory of the sort, reduced with the test hook, when a
    non-unique index and a unique one are created, then the index has the same entries as with all the memory, and a
    duplicate is detected the same (IX11); with all the memory, `krummi.sort` does not appear; with the test hook
    stopped after writing the first temporary file, `krummi.sort` exists, and on finishing successfully, on failing
    because of a duplicate and on catching a panic that interrupts the creation, it does not remain; when a process
    crash is simulated with the hook stopped and the store is reopened, then `krummi.sort` does not remain (not even
    with a foreign file inside, nor when `krummi.sort` is a file), the index does not exist and the foreign files of the
    folder remain (P7); an open that fails does not delete it, and a folder with `krummi.sort` and without `krummi.data`
    is not a store; closing the store with a `krummi.sort` that could not be deleted deletes it; in a temporary store, no
    file or folder is created (AC14 of the root still holds); with a simulated full disk when writing them, the creation
    fails with the class of G7 and the write follows T11; and with a simulated input/output failure when writing them, it
    fails with that class and the store stops accepting writes (T12).
15. **AC-IX15** — Given a store of format 11 written by 7.1, when this engine opens it, then it fails with the format
    version (B12, F11); a new store has version 12; and the information of the store does not change its form.
16. **AC-IX16** — Given loads W30–W32, when the full benchmark is run on Windows and on Linux, then the report shows 32
    thresholds, `no aplica` in the engines that are not Voron, the table of sizes W30, and the checksums of W31 and
    W32 match in both engines; the reduced run uses the quantities of its column; and the closing report includes the
    measurements of a single run, on Windows and on Linux, of W1–W29 before and after the change and of W30–W32 after
    it (AC-N9, AC-N10). A threshold that is missed follows K13.

## Annotations to other PRDs

This PRD changes or makes precise rules of earlier documents, which carry these annotations of 2026-11-18 (in their
translations too):

- [krummi.prd.en.md](../krummi.prd.en.md): D11 and AC20 (the values of the columns of the key of an index and of the
  primary key read from an entry are returned in a copy owned by the application, text and bytes too, IX8); G6 (the
  exception for rows also holds for the entries of the indexes, and for the rows that are deleted or from which an index
  is created in a table with indexes, IX12).
- [journal.prd.en.md](../journal/journal.prd.en.md): J2, J8 and J14 (`krummi.sort` is another reserved subfolder, with
  the rules of `krummi.temp`: it is not a store without `krummi.data`, the open deletes it in its step 8 and one that
  fails does not, and closing deletes it, IX2).
- [pager.prd.en.md](../pager/pager.prd.en.md): P13 (creating an index fails with out of memory when the system does not
  give the memory of the sort, IX2).
- [tables.prd.en.md](tables.prd.en.md): N5 (a vector may be an included column of an index; it cannot be a column of its
  key, IX1); N10 (different schema also covers create if it does not exist an index with another definition and
  traversing an index that does not exist, IX2, IX9); *Output* (the exception of IX8).
- [tables-schema.prd.en.md](tables-schema.prd.en.md): E2 (Describe returns the indexes and the state of the indexes,
  IX6); E5 (the rules of the index definitions of IX3, the schema too large counts the definitions and the rule of
  removing a column of an index, IX5); E6 (the new cases of different schema); E16 (IX13); E17 (the data of duplicate key
  in an index, the invalid size of an index entry and the precedence of IX10); E19 (IX12).
- [tables-row-reads.prd.en.md](tables-row-reads.prd.en.md): L3 (in a table with indexes, deleting a row, deleting a
  range and creating an index read the content of the rows that already exist, without checking it, like L1 and L2,
  IX12); L2 (a panic caught when deleting a range of a table with indexes prevents the write from being committed,
  IX12); reads by an index follow L1 and L2.
- [pager.prd.en.md](../pager/pager.prd.en.md): P3 (`krummi.sort` is a reserved name of the store folder, for the
  temporary files of creating an index, which opening the store deletes, IX2).
- [tables-projection.prd.en.md](tables-projection.prd.en.md): PY3 (the rows of an index traversal are tied to a
  projection; the entries are not, IX8).
- [benchmark.prd.en.md](../benchmark/benchmark.prd.en.md): the table of loads adds W30–W32; K4 (W31 and W32 start in the
  store of W30); K7 and *Output* (32 thresholds); K9 (W30).

## References

- Parent: [tables.prd.en.md](tables.prd.en.md) (N1, N5, N7, N10, N11, AC-N3, AC-N7, AC-N9, AC-N10); its parent,
  [krummi.prd.en.md](../krummi.prd.en.md) (D8, D11, AC20, G1, G4, G6).
- Siblings: [tables-schema.prd.en.md](tables-schema.prd.en.md) (E1–E8, E10–E13, E16–E20, *Benchmark loads*),
  [tables-row-reads.prd.en.md](tables-row-reads.prd.en.md) (L1–L3),
  [tables-projection.prd.en.md](tables-projection.prd.en.md) (PY3, PY4, PY7).
- Trees: [trees.prd.en.md](../trees/trees.prd.en.md) (B4–B9, B12),
  [trees-keys.prd.en.md](../trees/trees-keys.prd.en.md) (A3, A6, A7, A9, A10).
- Transactions: [transactions.prd.en.md](../transactions/transactions.prd.en.md) (T10, T11).
- Pager: [pager.prd.en.md](../pager/pager.prd.en.md) (F11, P12).
- Benchmark: [benchmark.prd.en.md](../benchmark/benchmark.prd.en.md) (K4, K7, K9, K11, K13).
- Design: `docs/design/tables/` (documents 2, 3 and 11; README, H15).

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| bdd88ebe | 2026-11-16 10:50 UTC+00:00 | 55 h 16 min | 52 h 29 min | 2 h 46 min | 1 h 49 min | 57 min | 14 |

<!-- prd-time: [{"session": "bdd88ebe-15fe-4364-8a18-4410e2ba4ed3", "start": "2026-11-16T10:50:01.910000+00:00", "wall": 198936, "discarded": 188965, "agent": 6533, "human": 3438, "interventions": 14, "idle_minutes": 10}] -->
