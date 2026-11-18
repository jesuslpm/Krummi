# Row Reads Without Checking Their Content — PRD

**Version**: 1.0
**Date**: 2026-11-11
**Parent document**: [Krummi Tables with Schema — PRD](tables-schema.prd.en.md) (v1.0)

> English translation of [tables-row-reads.prd.md](tables-row-reads.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Make reading rows of a table cost what it costs in Voron: when reading a row by its key or traversing a range, Krummi
stops checking the content of the row and relies on the integrity check of the pages (G6 of the root), which detects
the altered bytes of the file. Voron reads the fields of its rows directly from the page, without checking their
content.

**Why**: the `tabla-intervalos` load (W27) missed its K7 threshold on Windows and on Linux (0.15 and 0.19 at the close
of 7.1). With the later optimizations, which change nothing observable, it reaches 0.77 on Windows checking the rows,
and 0.83 without checking them. The user: «Y además ecribe un PRD que anote K7 para las cargas de tablas: Voron no
comprueba sus filas y Krummi sí (E19). Deja de comprobar las filas.» and «Como dije antes, las páginas ya se comprueban
con xxhash, así que ¿por qué más comprobaciones?». For the malformed row they chose «Pánico del proceso», made precise
later as «Pánico normal del hilo»; for writes, «Solo lecturas»; and for a transaction that survives a caught panic,
«Sigue intacta».

This PRD makes E19 of its parent precise (what 7.1 treated as damage to the page of a row when reading it), adds an
exception to G6 of the root and makes K7 of the benchmark precise; the three carry its annotation (*Annotations to
other PRDs*).

### Product decisions

| ID | Decision |
|---|---|
| L1 | **Reads do not check the content of the row** (makes E19 of the parent precise; exception to G6 of the root). Reading a row by its key and traversing a range, in both directions and in read and write transactions, no longer check the content of the row: its values (the type and the domain of E8, the precision of a `decimal`, that a `bool` is 0 or 1, the length of a vector, that a text is valid UTF-8), where each variable-length value starts and ends, the null bits, or the elements of its key. The following keep failing with damage, in the read or in the step of the traversal that returns it, as today: with the damage of the page of the row, a row of a schema version that the table never had, a row whose header is not one that Krummi writes (the width of its offsets, the reserved or compression bits) and a row that does not reach the end of its fixed part and of its offsets; with the damage of the page of the history that should have it, a row of a version that is missing from the history of the table. The columns added after the row was written are read with null or their default value, as today (N3), and the catalog, the default values stored in it and the history are checked as today. The order and the bounds of a traversal are those of the bytes of the stored keys, as today, also with a malformed key. |
| L2 | **A malformed row in a read** (its content is not what Krummi writes, in a page whose integrity passed G6: a failure of Krummi, a file altered with the store closed that recalculates the integrity codes, or a remembered page altered with the store open, already an exception to G6) is returned without error. Reading one of its columns, also one of the key, may return a value that is not the one written or end with a normal panic of the thread that reads it: it ends that thread, or the process if it is the main one, and the application can catch it (the user's decision). A text that is not UTF-8 read as text always ends with a panic; read as its bytes, it is returned as it is. The panic happens when reading the column, without changing the store or the transaction: if the application catches it, the transaction stays intact, read or write, and a write one can continue and commit (the user's decision); if the panic crosses the transaction, it is discarded like an abandoned one (T10) and, if it is a write one, its turn is left free. The same in all the builds of the engine that unwind the stack on panics; with one that aborts on panics, the panic ends the process. |
| L3 | **Update and insert or update keep checking** the row that already exists and that they replace, and fail with damage to its page if it is malformed, as today (E19, E17), so they do not rewrite it with values that are not its own. Inside a write transaction, reading by the key and traversing follow L1. Insert, the bulk load, delete, delete a range, count rows and the sequences do not read the content of existing rows and do not change (E19). A malformed row that another write does not read, in a page that this write rewrites, keeps its bytes with the new integrity code of the page, as the G6 annotation of 2026-11-06 already admits. |
| L4 | **K7 in the table loads that read rows** (makes K7 of the benchmark precise for W26 and W27): Voron reads its rows without checking their content and, from this PRD, the reads of Krummi do not check it either, so those loads compare the same work. The thresholds do not change. |

*Annotations of 2026-11-18 from the secondary indexes ([tables-indexes.prd.md](tables-indexes.prd.md), Spanish,
authoritative):*

- *L1 and L2: reads through an index follow L1 and L2, also in their entries (decision IX12).*
- *L2: exception: a panic caught when deleting a range of a table with indexes may arrive with part of the range
  deleted, and then the write can no longer be committed, as with damage (T11; decision IX12, user decision).*
- *L3: in a table with indexes, deleting a row, deleting a range and creating an index read the values of the rows that
  already exist without checking their content, as L1 and L2 do; update and insert or update keep checking the row they
  replace (decision IX12).*

### Starting state

- 7.1 treats as damage to its page a row that is read whose content is not what Krummi writes: each row is checked
  entirely in the read or in the step of the traversal that returns it (E19 speaks of the damage of a page; the check
  of the row belongs to the implementation of 7.1).
- W27 had ratios of 0.15 and 0.19 at the close of 7.1; after the optimizations with no observable change, 0.77 on
  Windows checking the rows and 0.83 without checking them (a single load). W26 already meets its threshold. W28 stays
  at 0.76 checking or not checking. They are context, not criteria: the acceptance is AC-L9.
- G6 of the root already has accepted exceptions in which an altered content may return data that is not the one
  written or end in a panic (annotations of 2026-10-30, 2026-11-05, 2026-11-06 and 2026-11-09).
- The backup, the compaction, the migration and the verification of the store belong to future modules (10–12 of the
  root); the parent of 7.1 asks them to treat the pages of the tables like those of the trees, without reading the
  content of the rows, and this PRD does not change it.
- No stored data changes: the format of the rows, of the keys and of the store is that of 7.1 (E20).

## Actors

- **The application** that reads rows of a table, by their key or traversing ranges, and that writes them.
- **The comparative benchmark**, whose table loads compare Krummi with Voron (K7).

## Flow

Reading, traversing and writing rows stay as in 7.1 (E10–E12); only what a read checks (L1) and what happens when
reading a malformed row (L2) change.

## Validations and messages

There are no new messages. In reads, the failure with damage of a row whose content is not what Krummi writes
disappears (L1, L2). The following keep failing with damage, with the data of E19: a page with damaged integrity, a
badly organized node, the cases of L1 that are still checked, the catalog or the history damaged, and, when updating
and inserting or updating, an existing malformed row (L3).

## Output

That of 7.1 for well-formed rows. A malformed row in a read follows L2.

## Reuse

- The integrity of the pages and their remembered integrity (G6 and its exceptions), which remain the defense against
  the altered bytes of the file.
- The layout check of the nodes of the trees and the check of the rows of 7.1, which remains valid for update and
  insert or update (L3).

## Out of scope

- The `tabla-actualizacion` load (W28), which misses its threshold through the write path: a separate change (K13).
  The loads W23, W24, W25 and W29 do not read rows and do not change.
- A specialized fixed-size key tree for single-column keys.
- Pinning the benchmark processes to the performance cores on hybrid processors: a separate change of the benchmark.
- A mode that checks the content of the rows again in reads, and a command that verifies the content of the rows of a
  store.
- The checks of the sequences, of the catalog, of the default values and of the history, which do not change.
- The row readers of future PRDs: secondary indexes (7.2), statistics (7.3), JSON columns (7.5), queries (17), the C
  interface and the C# binding (18) and import and export (19), which will decide whether they inherit L1 and L2.

## Left to the implementation

- Which malformed contents, apart from the text that is not UTF-8 (L2), return a value that is not the one written and
  which end in a panic, and in which column read it happens: the values, their offsets (also one that starts after it
  ends), the bytes left over after the last value, the null bits (also one set in a column that does not admit nulls)
  and the elements of the key (an element of another type, truncated or without its terminator).
- How the tests write a malformed row in a page with correct integrity: it is a test hook, not a public contract.

## Acceptance criteria

1. **AC-L1** — Given a row stored with a text that is not UTF-8 in a page with correct integrity, when it is read by
   its key and when a range that includes it is traversed forwards and backwards, in a read transaction and in a write
   one, then the read and the step of the traversal do not fail, its other columns are read with their values, reading
   that text as text ends with a panic and reading it as bytes returns them as they are; the tests pass the same
   compiled in debug and in optimized.
2. **AC-L2** — Given a row with a `bool` that is neither 0 nor 1, one with a malformed key element and one with an
   offset of a variable value outside the row, in pages with correct integrity, when they are read by their key or
   traversed, then the read and the step of the traversal do not fail, and the traversal returns the rows of the range
   in the order of the bytes of their keys (what reading the malformed column gives is left to the implementation,
   L2).
3. **AC-L3** — Given a row of a schema version that the table never had, one with a header that Krummi does not write
   and one that does not reach the end of its fixed part, in pages with correct integrity, when they are read by their
   key or traversed, then they fail with the damage class and the number of the page of the row; and a row of a
   version that is missing from the history of the table fails with the damage class and the number of the page of the
   history that should have it. All as today.
4. **AC-L4** — Given an existing malformed row in a page with correct integrity, when it is updated or inserted or
   updated, also after reading it in the same transaction, then it fails with damage to its page, without changing the
   row, and the transaction follows T11, as today (L3).
5. **AC-L5** — Given the panic of reading a text that is not UTF-8: when the application catches it around that read,
   then the transaction stays intact (in a read one, the other reads work; in a write one, it can keep writing and
   commit, and what it did is applied); and when it catches it outside the transaction, which the panic crosses, then a
   read one ends, a write one is discarded without applying anything and its turn is left free (T10), and in both cases
   other reads, the next write and the closing of the store work.
6. **AC-L6** — Given a page of rows altered with the store closed, when one of its rows is read or traversed, then it
   fails with damage (AC19 of 7.1 and AC10 of the root remain valid).
7. **AC-L7** — Given a store written by 7.1, when it is opened and its tables are read, then the same rows are read
   with the same values, without migrating anything.
8. **AC-L8** — Given E19 of 7.1, G6 of the root and K7 of the benchmark, in the original and in its translation, when
   they are read, then they carry the annotation of this PRD.
9. **AC-L9** — Given the complete runs of the benchmark on Windows and on Linux, when they are compared with those of
   the close of 7.1, then W26 and W27 meet their K7 threshold on both, and no other load drops its ratio with Voron by
   more than 0.05, unless the median of a measurement of that single load, with its repetitions, equals that of the
   close of 7.1 within that margin. A W26 or W27 threshold that is still missed is not a failure of this PRD: K13
   applies and it is reported at the close.

## Annotations to other PRDs

This PRD changes rules of earlier documents, which carry these annotations of 2026-11-11 (in their translations too):

- [tables-schema.prd.en.md](tables-schema.prd.en.md), E19: *«Reads by the key and traversals no longer check the
  content of the row (its values, its offsets, its null bits and its key); a malformed row in a page whose integrity
  passed G6 is returned without error, and reading one of its columns may return a value that is not the one written
  or end with a panic; a text that is not UTF-8 read as text always ends with a panic. The unknown schema version or
  one missing from the history, the header that Krummi does not write and the row that does not reach the end of its
  fixed part keep failing with damage; update and insert or update keep checking the row they replace (decisions L1–L3
  of [tables-row-reads.prd.en.md](tables-row-reads.prd.en.md)).»*
- [krummi.prd.en.md](../krummi.prd.en.md), G6: *«Exception to G6: reading by the key and traversing the rows of the tables
  do not check their content. A malformed row in a page whose integrity passed G6 (a failure of the engine, a file
  altered with the store closed that recalculates the integrity codes, or a remembered page altered with the store
  open) may return data that is not the one written or end in a panic instead of an error. Update and insert or update
  keep checking the row they replace; AC10 does not change (decisions L1–L3 of
  [tables-row-reads.prd.en.md](tables-row-reads.prd.en.md)).»*
- [benchmark.prd.en.md](../benchmark/benchmark.prd.en.md), K7: *«In W26 and W27, Voron reads its rows without checking their
  content and Krummi does not either since [tables-row-reads.prd.en.md](tables-row-reads.prd.en.md) (L4): the two loads
  compare the same work. The thresholds do not change.»*

## References

- Parent PRD: [tables-schema.prd.en.md](tables-schema.prd.en.md) (E10–E12, E17, E19, E20); its parent,
  [tables.prd.en.md](tables.prd.en.md) (N3); the root, [krummi.prd.en.md](../krummi.prd.en.md) (G6, D11, D12, AC10).
- Benchmark: [benchmark.prd.en.md](../benchmark/benchmark.prd.en.md) (K7, K13).
- Transactions: [transactions.prd.en.md](../transactions/transactions.prd.en.md) (T10, T11).
- Why a malformed text always ends in a panic: it is returned without copying (D11) and without unsafe code outside the
  pager (D12), so it is validated when it is read.
- Voron: `TableValueReader` in `src/Voron/Data/Tables/TableValueReader.cs` of RavenDB 7.2.7, which reads each field by
  its offset without checking it.
- Measurements: `odd/done/tables/table-row-reads.md`; close report of 7.1, `odd/done/tables/tables-schema-report.md`.
