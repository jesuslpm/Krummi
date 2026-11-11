# Krummi Tables with Schema — PRD

**Version**: 1.0
**Date**: 2026-11-10
**Parent document**: [Krummi Tables — PRD](tables.prd.en.md) (v1.0)

> English translation of [tables-schema.prd.md](tables-schema.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Build the child PRD **7.1** of the [split of the tables module](tables.prd.en.md#split-into-child-prds): create, open,
delete, rename, empty, describe and list tables; their column types, their primary key and the changes to their
schema; insert, update, insert or update, read and delete rows by the primary key; traverse and delete by ranges of
the primary key; the bulk load; the `ulid` and `uuid` generators; the sequences; and the table workloads of the
comparative benchmark. Secondary indexes belong to 7.2, statistics to 7.3 and JSON columns to 7.5.

The decisions were made with the user on 2026-11-10, on the table design documents (`docs/design/tables/`), whose
decisions P1–P20 of document 11 were already approved. This PRD changes rules of its ancestors, which carry its
annotation (*Annotations to other PRDs*):

- **Sequences instead of autonumeric columns** (N4, N6, AC-N3, AC-N4 and *Out of scope* of the parent). User: «Yo
  preferiría no tener autonuméricos y en su lugar tener secuencias para generarlos. Los autonuméricos tienen muy
  mala fama.»
- **Update changes only the columns that are given** (N6 and *Out of scope* of the parent, which said that update
  replaces the whole row), so that an earlier version of the application does not erase the columns it does not know.
- **A fourth error class, invalid schema** (N10 of the parent).
- **The domains of dates and times are those of .NET and SQL Server** (P7 of design document 11 left it to 7.1 to
  decide whether they were limited).
- **The catalog has three object classes**: trees, tables and sequences (N1 and AC-N5 of the parent).
- **The invalid size that depends on the schema loses to the catalog and schema failures** (A7 and B7, which put it
  ahead of everything). User: the sizes that need the schema are checked «en la fase correspondiente, sin forzar
  artificialmente la precedencia».

### Product decisions

| ID | Decision |
|---|---|
| E1 | **Opening modes** (A1, with tables): a table is opened by its name in one of the modes of A1. **Open** receives only the name and never compares the schema; if the table does not exist, it fails with tree not found (A6). **Create new**, **Open or create** and **Create or empty** also receive the schema: they create the table empty with it if it does not exist; if it exists, Create new fails with tree already exists (A6), and the other two check that the given schema is compatible (E6) before opening or emptying it. The modes that receive a schema validate it with E5 before anything else about the schema, whether the table exists or not. **Empty** receives only the name and empties the table (E3); if it does not exist, it fails with tree not found. In a read transaction only Open exists. The classes of A6 carry the name of the table as data. A name that belongs to another object class of the catalog fails with different type (N1), which wins over different schema and over already exists. |
| E2 | **Other catalog operations** (A3, with tables): **Exists**, **Delete** (deletes the table with its rows, its indexes and its statistics and frees its pages; if it does not exist, it does nothing) and **Rename**, with the rules of A3. Rename overwrites a destination that exists only if the application explicitly asks for it; without asking, it fails with tree already exists; renaming to its own name does nothing. When overwriting, the destination table disappears as with Delete. The renamed table keeps its schema, its rows, its indexes, its identity and its openings. **Describe** receives the name of a table, in both classes of transaction, and returns its current schema as its transaction sees it (a write sees its own changes): for each live column, in the order of the schema (E4), its identity, its name, its type, whether it admits nulls and its default value; the columns of the primary key in their order; the schema version; and the schema state (design document 11, P12). If it does not exist, it fails with tree not found; if the name belongs to another object class, with different type. A column keeps its identity when it is renamed. The schema state changes with each schema change that gives a new version (E4), and not when renaming or emptying the table nor when storing rows. The version is informative: the state is identified by the identities and the schema state, random values that are drawn again if they match one that the store knows (P12). Like List in A3, Describe returns a copy owned by the application. **List** returns each name of the catalog with its class, in the name order of B5: tree of each type, table or sequence (N1, E14). |
| E3 | **Emptying a table** (modes Create or empty and Empty, A2) removes all its rows and the entries of its indexes and frees its pages; it keeps its schema, the definitions of its indexes and its identity; the other openings of the table remain valid and see it empty. It resets its statistics and adds the rows it removes to the modification counter (N8), both from 7.3. It does not touch any sequence. User: «No quiero que vaciar una tabla equivalga a eliminar sus índices.» |
| E4 | **Schema**: a table has from 1 to 1,024 live columns (removed ones do not count). The **schema order** is that of the columns when the table was created, with each added column at the end; removing one does not move the others. The name of a column follows the rules of tree names (B4): UTF-8 text from 1 to 1,024 bytes, compared by its bytes. The primary key is one or several distinct columns, which do not admit nulls, are not vectors and have no default value, and all of them sort ascending. The last two are deliberate simplifications of this version, not technical limits: a column of a composite key could have a constant default value (for example, a fixed tenant). A column that is not part of the key can have a default value when the table is created and when the column is added: a constant of its type, never an expression; declaring null as the default value is the same as having none. A default value and a generator are different things: the default value is a constant that the schema stores and Krummi sets when inserting; the generators of E15 and the sequences of E14 are functions that the application calls and whose value it puts in the row, and none of them can be the default value of a column. Schema changes are requested inside a write, with four operations: **add** a column, **remove** one, **rename** one and **modify** one, giving its type and whether it admits nulls. Modify only admits allowing nulls in a column that is not part of the key (N3); changing the type, forbidding nulls or modifying a column of the key (which is changing the primary key) fail with invalid schema. Removing, renaming or modifying a column that does not exist fails with invalid schema. A column added with the name of a removed one is new: it has another identity, goes at the end of the schema order and never recovers the values of the earlier one (N3). Adding a column that does not admit nulls requires a default value, whether the table has rows or not: N3 only admits adding a column that admits nulls or one with a default value. A vector column with the cosine metric is added like any other. A table is created with version 1 of its schema, and each admitted change adds 1, also if there are several in the same write; a change that changes nothing (renaming a column to its own name, modifying a column with the same type and the same nullability, allowing nulls in a column that already admits them) gives no new version. All the openings of the table in the write that changes its schema see the new schema from that moment (B9). |
| E5 | **Invalid schema** (new class, D8 of the root parent): a public validation error with the rule of B7 and the data of T18. Creating a table or changing its schema fails without effect with **invalid schema** when the schema that would result breaks a rule: no primary key; a column of the key that admits nulls, is a vector or has a default value; a key that names a column that does not exist or repeats a column; two columns with the same name (also when adding one or renaming one to a name that already exists); more than 1,024 live columns; a vector outside the limits of N4; a `decimal(p, s)` outside 1 ≤ p ≤ 38 and 0 ≤ s ≤ p; the cosine metric over `bit` elements; a default value that does not meet the rules of its column (E8); an added column that does not admit nulls without a default value; a change that N3 does not admit (changing the type, changing the primary key, forbidding nulls); removing, renaming or modifying a column that does not exist; removing a column of the primary key or, from 7.2, of an index (AC-N7); or a schema version beyond 65,535, which is detected before anything is changed (P19). It carries as data the table, the column if there is one and the rule broken, which is one of the cases of this list separated by semicolons; if several are broken, the first in the order of this list is reported and, within it, the first column in the schema order. The sizes of names keep failing with invalid size (B7). |
| E6 | **Different schema**: a schema given to an opening mode (E1) is **compatible** with the table that exists when each column it names exists in it with the same type, the same nullability and the same default value, and its primary key has the same columns in the same order; the table may also have columns that the schema does not name (those added by a later version of the application), which stay in it and are read with the others. If it is not compatible, the opening fails without effect with **different schema**, with the table, the column and the difference (what was expected and what was found) as data. Storing a row that brings a column that the table does not have also fails with different schema, naming the column. |
| E7 | **Input values**: a `uuid` is always given in the byte order of RFC 9562, also in a column with the SQL Server order, because the order is a property of the key; a `uuid` given in the .NET byte order cannot be told apart and is stored as another value, so converting it is up to the caller (the C# binding of module 18 will do it). An `f16` element is given as IEEE binary16 little-endian. In a `bit` vector, element i is bit i mod 8, counting from the least significant, of byte i div 8. A `string` is any valid UTF-8 text, including the character U+0000. Empty text and empty bytes are valid values, also in the primary key. A `-0` inside an `f32` or `f16` vector is stored as `+0`, like scalars (N4). |
| E8 | **Domains and rounding**: `date` goes from 0001-01-01 to 9999-12-31; `timestamp`, from 0001-01-01T00:00:00Z to 9999-12-31T23:59:59.999999Z; `time`, from 00:00:00 to 23:59:59.999999, with no leap second; `duration`, from −922,337,203,685,477,580 to 922,337,203,685,477,580 microseconds, the extremes of .NET `TimeSpan` truncated to the microsecond. A `decimal` with more decimals than its scale is rounded to its scale, with ties going away from zero, like SQL Server (1.235 in a `decimal(10, 2)` gives 1.24; −1.235 gives −1.24). An instant, a time or a duration with a fraction below the microsecond is rounded to the nearest microsecond, with the tie going to the greater value. A value outside its domain, also when rounding takes it there (99.995 in a `decimal(4, 2)` gives 100.00, outside its precision; 9999-12-31T23:59:59.9999995Z goes past the maximum), fails with invalid value (N4). They apply equally to the values of a row and to the bounds of a range (E12). |
| E9 | **`ulid` and `uuid` text**: there is a conversion of each one to text and from text, with a single form: `ulid` in 26 uppercase Crockford Base32 characters, the first one at most `7`; `uuid` in the lowercase RFC 9562 text with hyphens (8-4-4-4-12), in the RFC 9562 byte order also for columns with the SQL Server order. A text that is not that form (lowercase in a `ulid`, uppercase, braces or no hyphens in a `uuid`) fails with invalid value. |
| E10 | **Storing rows** (changes N6 of the parent): each operation receives the row as column and value pairs, with all the columns of the primary key; a column of the key that is missing or a column that is given twice fail with invalid value. **Insert**: a column that does not come takes its default value if it has one and, if not, null; if the primary key already exists, it fails without effect with duplicate key, even if the row is identical. **Update** looks for the row by its primary key and changes only the columns that are given; the others keep their value; if the row does not exist, it does nothing and reports that it did not find it. **Insert or update** updates if the row exists and, if not, inserts, with those same rules. An explicit null stores null, never the default value. A column that does not admit nulls and is left without a value, or with a null, fails with invalid value. An update is **identical** when the row that would result is equal, as E11 would read it, to the one that is there, for example the one that gives only the key. Insert or update reports whether it inserted or updated. The primary key of a row does not change: to change it, the row is deleted and inserted with the new key in the same write. |
| E11 | **Read, delete and count**: **Read** returns the row of a complete primary key, or that it is not there. A row that is read or traversed has all its live columns, in the schema order, each one accessible by its name or by its position among the live columns, with null or with its default value in those added after it was stored (N3); a removed column never appears. **Delete** removes the row and returns whether it was there. The number of rows that an opening reports follows N6. |
| E12 | **Ranges of the primary key**: a range gives equal values for the first k columns of the key (the prefix, from 0 to all of them) and, if k is smaller than the number of columns of the key, optionally a lower bound and an upper bound on the next column, each one included, excluded or absent. With all the columns equal, the range is a single row. Rows are traversed forwards and backwards in the order of N5. An empty or inverted range with valid values returns no rows and is not an error. The following fail with invalid value: a value of a type that is not that of its column or outside its domain, a null (the key does not admit nulls), a prefix with more columns than the key and a bound when the prefix already gives all the columns; NaN is a valid value (N5). A prefix value or a bound that does not fit in the maximum of the key fails with invalid size, even if the range is empty, as in the traversals of trees. **Deleting a range** takes the same bounds and returns how many rows it deleted. There are no bounds on more than one column at a time. |
| E13 | **Bulk load**: it is atomic and forms its own write transaction, with all its rules (G1, T4, T5, T12): it receives the name of the table and a sequence of rows that can fail, and loads the rows with the rules of insert. If the table does not exist, it fails with tree not found, and if the name belongs to another object class, with different type. It waits for its write turn with the time limit it is given and fails like a write when it expires (T4), before looking at the catalog. While it loads rows, before committing, if a row fails, if the sequence fails, if the application cancels or if there is a panic, the load aborts entirely, and the table, its indexes, the sequences and the number of rows stay as before; if the process ends abruptly in that phase, the load was never committed and the recovery of the store discards it (G5). Its commit follows the rules of any write: past the point of no return of the commit, the load is committed even if there is a panic afterwards or the process ends, and the recovery keeps it when it was already durable (G1, G2, G5; E2 and E4 of the group journal). It admits a table with rows, and from 7.2 with indexes, and rows in any order, faster in primary key order. On commit, it reports how many rows it loaded; an empty sequence changes nothing. A thread that already has the write turn cannot start it: it fails at once with misuse (T5); a read open in the same thread does not prevent it. |
| E14 | **Sequences** (they change the autonumeric columns of N4 and N6): a sequence is a named object of the catalog: it is not a tree type, but it shares the catalog and the namespace with trees and tables, and a name belongs to a single object class. It is opened by its name in the **Open**, **Create new** and **Open or create** modes of A1; the modes that empty do not exist for it. When it is created, its first value is set, 1 if none is given; Open or create on a sequence that exists opens it without looking at the first value given. **Exists**, **Delete**, **Rename** and **List** follow the rules of tables (E2, A3, A6); Exists and List apply to any object class and never fail because of it (N1). Any other sequence operation on a table or a tree, or the other way around, fails without effect with different type, also when renaming with overwrite; without overwrite, a destination of another class fails with tree already exists (N1). **Asking for the next value**, through the opening, is only possible in a write transaction and returns a signed 64-bit integer, one more than the previous one. It is transactional: if the write aborts, its values are available again, and a value of a committed write is never given again, even if rows are deleted, a table is emptied or the process crashes. At the maximum of the signed 64-bit integers, asking for another value fails without effect with invalid value, naming the sequence. An opening of a sequence that the same write deleted fails with tree not found (A6, B9). A sequence is not tied to any table. |
| E15 | **`ulid` and `uuid` generators** (N6): they are public functions that the application calls and whose value it puts in the row; they do not depend on any store. There are three: **`ulid`**, 48 bits of Unix milliseconds and 80 random bits; **`uuid` version 7**, for columns with the RFC 9562 order, with the version and variant bits of RFC 9562 and 74 random bits; and **sequential `uuid`**, for columns with the SQL Server order, with the user's algorithm: 48 bits of Unix milliseconds in the bytes that SQL Server compares first and 80 random bits in the others, in the layout of its .NET generator. In the .NET byte order (that of `Guid.ToByteArray`, which `SqlGuid` uses): bytes 10–15, the millisecond in big-endian; bytes 8–9, the high 16 bits of the random part in big-endian; and bytes 6–7, 4–5 and 0–3, its low 64 bits from most to least significant, each group in big-endian. So SQL Server, which compares the groups 10–15, 8–9, 6–7, 4–5 and 0–3, orders them by the millisecond and then by the 80 bits. Krummi gives it in the RFC 9562 order (E7), which reverses the groups 0–3, 4–5 and 6–7 with respect to the .NET one. The random bits are drawn at each new millisecond, and cannot be predicted from other generated values (design document 11, P20). Each generator is strictly increasing in the process, also across threads, with three rules: (1) in a millisecond later than the last one used, it draws new bits; (2) in the same millisecond, or if the clock gives an earlier one (the clock went backwards), it continues with the last millisecond used and adds 1 to its random part, taken as a single number, with carry; (3) if that addition overflows the random part, it uses the millisecond following the last one used and draws new bits. In the `uuid` version 7 the random part is its 74 random bits: the 4 version bits and the 2 variant bits never change, and the carry skips them. A millisecond that does not fit in 48 bits (after the year 10889) fails with invalid value. If the system fails to give the random bits, the function fails with input/output. There is no promise of order between processes or between restarts. |
| E16 | **Changes and references** (A10, T8, T10, B9): the following are changes for T10: creating a table or a sequence, emptying a table, deleting one that exists, renaming to another name, a schema change that gives a new version, insert, update or insert or update when it is not identical (E10), deleting a row that was there, deleting a range with rows, a bulk load with rows and asking for a value of a sequence. The following are not: Open or create on an object that exists, Delete of one that does not exist, renaming to its own name, a schema change that changes nothing, update without finding the row, an identical update and every operation that fails with a public validation error. As in A10, the earlier references end with every operation that can modify, even if in the end they change nothing or fail: the modes other than Open, Delete, Rename, the four schema changes, insert, update, insert or update, delete a row, delete a range, create a sequence and ask it for a value. Open, Exists, Describe, List, Read, traverse and the number of rows do not end them. |
| E17 | **Errors: data and precedence** (N10, B7): duplicate key carries the table and the primary key (from 7.2, also the index); invalid schema and different schema, those of E5 and E6; invalid value, the table and the column when there are any, the sequence when it is one, and the rule broken, one of these, in this order: repeated column, missing key column, null not admitted, type different from that of the column, malformed text, outside the domain, precision of a `decimal`, dimension of a vector, NaN or infinity in a vector, zero vector with cosine metric, invalid range, exhausted sequence or millisecond outside 48 bits. If a row breaks several, only one is reported, in three steps: (1) if some column is given more than once, the first that is repeated in the order in which they were given, with repeated column; (2) if not, if some key column is missing, the first in the key order, with missing key column; (3) if not, the first column in the schema order that breaks any of the other rules, with the first one it breaks in the order of this list. The precedence extends A7 and B7: first misuse (T5, which fails at once) and the timeout of a bulk load (E13); then the invalid size of what is checked without reading the store (names; in Rename, the source one before the destination one); then an operation through an opening of a deleted object, which fails with tree not found without reading anything else; then the failures reading the catalog, different type, tree not found and tree already exists; then invalid schema and, after it, different schema; then invalid value and, after it, the invalid size of what the schema needs (a primary key or a row larger than its maximum); then the failures reading the pages of the table or of the sequence; and last duplicate key. Two exceptions, because they need to read pages: the size of the row that would result from updating or from inserting or updating a row that exists is checked after reading it, and an exhausted sequence is detected after reading its page. |
| E18 | **Limits**: a row measures at most 256 MiB, the maximum of a value (B6), and the primary key, 2,048 bytes (B5), both measured in the form in which Krummi stores them, which design documents 1 (rows) and 3 (keys) set; a column name, from 1 to 1,024 bytes (B4), also the one given in a row or in a schema change, which is checked with the names, before the catalog (E17). Exceeding them fails without effect with invalid size, with the data of B8 (the size received, the minimum and the maximum). |
| E19 | **Damage** (A9, with tables and sequences): the Open, Create new and Open or create modes, Exists, Describe, List, Rename without overwrite or to its own name and the number of rows never fail with damage in the pages of the object itself; read, store, delete, traverse, load and ask for a value fail with damage in a page that they need; delete, empty and rename with overwrite can fail with damage in any page of the affected object. In a write, every failure due to damage follows T11. |
| E20 | **Format**: the change is incompatible (B12), so the format version becomes 11. The format of the rows reserves what dictionary compression will need (N9); the design review checks it. The information of the store (P12) does not change shape: the pages of tables and sequences already count in its totals. |
| E21 | **Benchmark loads** (N11, K7): the seven loads of *Benchmark loads* (below), against the tables of Voron, with their threshold of K7; the other engines say `no aplica`. The report counts 29 thresholds. |

*Annotations of 2026-11-11, after Codex's code review (decisions of the user):*

- *E5: a case is added at the end of its list: a schema whose stored form, with the names and default values of its columns, passes 256 MiB, the maximum of a catalog entry, fails without effect with invalid schema and the rule "schema too large", naming the column with which it passes it, also when adding a column or renaming one to a longer name (the column of the change). The user, about a default of almost 256 MiB that fitted in a row but not in the catalog: «Aceptar el límite» (accept the limit).*
- *E8: the domain is checked on the value already rounded to the microsecond, which is the stored one: a value given outside its domain that rounding brings inside is accepted (a time of −500 ns is stored as 00:00:00), and one that rounding takes outside fails with invalid value. The user: «Se acepta» (it is accepted).*

#### Benchmark loads

All of them use a table with primary key `id` and the columns `n` (`i64`), `x` (`f64`), `ts` (`timestamp`), `flag`
(`bool`) and `name` (`string` of 75 ASCII characters), about 100 logical bytes per row, like the values of K4. The
fixed seed of K4 generates all the values: `n` uniform among all the `i64`; `x` uniform in [0, 1), never NaN, infinite
or `-0`; `ts` uniform between 2000-01-01T00:00:00Z and 2030-01-01T00:00:00Z; `flag` at random; `name`, ASCII letters
and digits; and the keys, with the layout of E15 but with bits from the seed, not with the public generators, so that
all engines and repetitions receive the same bytes (K4). Voron uses its `Table` API with an equivalent `TableSchema`,
primary key on `id`, the same data, in the same order and with the same durability. Generating the data stays outside
the time measured in both engines. Each load starts from its initial state, without inheriting changes from another.

| ID | Name | Starts at | What it does | Operations | Reduced |
|---|---|---|---|---|---|
| W23 | `tabla-insercion-en-orden` | Empty store | Inserts 1,000,000 rows with `ulid` keys in order, in writes of 1,000. | 1,000,000 | 10,000 rows |
| W24 | `tabla-insercion-al-azar` | Empty store | Inserts 1,000,000 rows with random `uuid` keys, in the RFC 9562 order, in writes of 1,000. | 1,000,000 | 10,000 rows |
| W25 | `tabla-carga-masiva` | Empty store | Bulk loads 1,000,000 rows with `ulid` keys in primary key order; the time includes the commit. In Voron, a write transaction with all the rows. | 1,000,000 | 10,000 rows |
| W26 | `tabla-lecturas` | The table of W23 | Reads all the rows by their primary key, at random, in a read transaction, reading all their columns. | 1,000,000 | 10,000 reads |
| W27 | `tabla-intervalos` | The table of W23 | In a read transaction, 100,000 forward ranges, each one from an existing key chosen at random, that read up to 100 rows with all their columns (fewer if they reach the end), like W11. | The rows read | 1,000 ranges |
| W28 | `tabla-actualizacion` | The table of W23 | Changes `n` in all the rows, at random, in writes of 1,000: Krummi gives only `n` (E10); Voron, which does not update a single column, reads the row and stores it whole with the new `n`. | 1,000,000 | 10,000 rows |
| W29 | `tabla-borrado` | The table of W23 | Deletes all the rows at random, in writes of 1,000. | 1,000,000 | 10,000 rows |

W28 compares the same logical operation, changing `n` in one row, done with the real capabilities of each engine,
even though the physical operations are different. W26, W27 and W28 start in the store that W23 leaves and W29 in the one that W23 leaves, each one in its own copy, like the loads of the
base store (K4). The size of the store (K9) is also measured after W23, W24, W25 and W29, without a compaction that
the other engine does not do; the physical size includes the key, the row format and its metadata, so it is not 100
bytes per row. W26 and W27 compute in both engines the same checksum of the values of all the columns read, inside the
measured time because it is the way of reading the values, and when they finish they compare it with the expected one.
The reproducibility rules of the benchmark apply (K4, K8, K13): durable operations, the same conditions, three
repetitions and the median.

### Starting state

- There are no tables or sequences in the code: the catalog stores normal, multi-value and fixed-size trees, each
  entry with its identity, its type, its root and its number of entries, and an unknown type is damage. Tree
  identities are a counter.
- The public API has the opening modes of A1 and the operations of A3 for trees; List returns the tree type of each
  name, so listing tables and sequences needs an object class in the catalog.
- The benchmark has 22 workloads (W1–W22) and no engine measures tables; the Voron harness does not use its `Table`
  API yet.
- The information of the store (P12 of the pager, F10) does not count trees, only pages.
- The format version is 10 (B12).
- There are no `ulid` or `uuid` generators, nor a function that reads the operating system's random generator.

## Actors

- **Application developer (Rust)**: defines tables, stores and reads rows, asks the generators and the sequences for
  values.
- **Whoever runs the benchmark**: compares the loads W23–W29 with Voron.

## Flow

1. The application opens a write and creates the table with its schema, or opens it with Open or create and the
   schema it knows.
2. It asks for a `ulid` or a value of a sequence, puts it in the row and inserts; updates, reads, traverses or deletes
   by the primary key.
3. To load many rows, it calls the bulk load with the name of the table, outside any write.
4. To change the schema, it requests the change in a write; the stored rows are read with the new schema.

## Validations and messages

Those of the ancestors apply. This PRD adds, only in the library:

| Situation | Error class |
|---|---|
| Opening, emptying, describing or loading a table that does not exist; renaming from an object that does not exist; using the opening of an object that the write deleted | Tree not found (A6) |
| Create new on an object of the same class that exists; renaming without overwrite to a name that exists, of any class | Tree already exists (A6) |
| A table or sequence operation, except Exists and List, on another object class of the catalog, including Create new and rename with overwrite on it | Different type (B3, N1) |
| A schema that breaks a rule of E5, when creating or changing it | Invalid schema (new) |
| Opening with a schema that is not compatible; storing a row with a column that the table does not have | Different schema (N10) |
| A value that breaks a rule of E17 (of another type, outside its domain also after rounding it, a null or a column without a value that does not admit nulls, a key column that is missing or a repeated one, a malformed `ulid` or `uuid` text, a malformed range, an exhausted sequence) | Invalid value (N10) |
| Inserting a primary key that already exists | Duplicate key (N10) |
| A column name, a primary key or a row larger than its maximum; a range bound that does not fit in the key | Invalid size (B7) |
| Starting a bulk load from a thread that already has the write turn | Misuse (T5) |
| A bulk load that does not obtain its turn before its time limit | Timeout (T4) |
| The operating system does not give random bits to a generator | Input/output (D8) |

Updating a row that does not exist, reading it, deleting it or traversing an empty range are not errors (E10–E12).

## Output

The module writes nothing to the screen. Read and traverse return rows with the shape of E11 and the values with the
normalization of N4 and E7, as the *Output* of the parent says. Describe and List return copies owned by the
application (E2). Delete returns whether the row was there; deleting a range, how many rows it deleted; update,
whether it found the row; insert or update, whether it inserted or updated; the bulk load, how many rows it loaded;
asking a sequence for a value, the value. The benchmark report adds the
rows W23–W29 with their thresholds, with `no aplica` in the engines that are not Voron, and its table of sizes adds
W23, W24, W25 and W29.

## Reuse

- The catalog of trees, its modes, its operations, its error classes, its precedence and its damage rule (A1–A10,
  B3–B12), extended to tables and sequences.
- The B+ trees and their traversals (5.1–5.4) as storage of the rows.
- Transactions, the journal and recovery (G1, G5, T4, T5, T8, T10–T12).
- No function that reads the operating system's random generator: 7.1 adds it to the pager (design document 11, P20)
  and the generators and the catalog identities share it.
- The comparative benchmark, its seed, its rules and its report (module 6).

## Out of scope

- Secondary indexes (7.2), statistics (7.3) and JSON columns (7.5), except what E3, E5 and E17 say about them.
- Insert or ignore, and any other variant of insert that does not fail with a duplicate key.
- An operation to change the primary key of a row (E10).
- Reading only some columns of a row (projections): module 17.
- Generated or autonumeric columns, computed default values (such as the current time), generators as default value,
  and default values in the primary key (simplification of this version, E4); changing the default value of a column
  (as in the parent).
- In sequences: reading their current value without consuming it, resetting them or setting their value after
  creating them, a step other than 1, descending, cyclic or value-caching sequences.
- Descending primary key columns (the indexes of 7.2 do have them).
- Ranges with bounds on more than one column and filters: module 17.
- Dates and times outside the domains of E8.
- Promises of generator order between processes or between restarts.
- The C interface and the C# binding (module 18); the test of the SQL Server order uses C# only to generate its
  vectors.

## Left to the implementation

- How tables and sequences are stored in the catalog and in trees, with the starting point of the design documents;
  the shapes of the row and of the key that the limits of E18 measure are those of documents 1 and 3.
- The names of the operations, the types and the error classes, in English and with the conventions of Rust (D8).
- How the test hooks of AC4, AC9, AC12 and AC13 are built: they are test and diagnostic mechanisms, not public
  contracts; the criteria set what they prove.
- The value of the first millisecond of a process for the generators and how threads share the sequence.

## Acceptance criteria

Those of the ancestors apply. This PRD fulfills AC-N1 to AC-N6, AC-N9 and AC-N10 of the parent with what these
criteria set.

1. **AC1** — Given the modes of A1 on tables, when a table that exists and one that does not are opened, created new,
   opened or created, created or emptied and emptied, then each mode does what E1 says, and in a read only Open
   exists; when opened or created with a compatible schema that lacks columns that the table has, then it opens it
   and its rows are read with all their columns; when opened or created, or created or emptied, with a schema that
   differs in the type, the nullability or the default value of a column, names a column that does not exist or
   changes the primary key, then it fails without effect with different schema and its data; and when opened or
   created with any schema a name that belongs to a tree or to a sequence, then it fails with different type.
2. **AC2** — Given a table with rows and another table, when one is renamed to the other without overwrite, then it
   fails with tree already exists; with overwrite, the destination disappears with its rows and the renamed one keeps
   its rows, its identity and its openings; Describe returns the schema of E2 in a read and in a write, the write
   with its own changes, with the same identity of a column before and after renaming it and with another schema
   state after each change that gives a new version; Describing a name that does not exist fails with tree not
   found, and that of a tree or a sequence, with different type; Exists says that the names of the three classes
   exist; and List shows the tables, the trees and the sequences with their class,
   in the order of B5.
3. **AC3** — Given a table with rows and another opening of it, when it is emptied and committed, then it has no rows,
   keeps its schema and its identity, the other opening sees it empty, and a sequence that it used does not change.
4. **AC4** — Given each invalid schema of E5, when the table is created with it or the change that would produce it is
   requested, then it fails without effect with invalid schema, with the table, the column and the rule, and the
   transaction can commit its other changes; when a table created in version 1 receives 65,534 changes and another is
   requested, then it fails without effect with invalid schema; a change that changes nothing gives no new version;
   another opening of the table in the same write sees the new schema; and each schema change writes the same pages
   with 10 as with 100,000 rows, according to its test hook (AC-N2, AC-N6).
5. **AC5** — Given a table with default values, when a row is inserted without those columns, then their default
   values are read, and null in those that have none; with an explicit null, null; and with neither a value nor a null
   in a column that does not admit them, it fails with invalid value; when a row is updated giving only some columns,
   then the others keep their value, also a column added after the application defined its schema; when a row is
   stored with a column that the table does not have, then it fails with different schema; and when it lacks a key
   column or repeats a column, with invalid value.
6. **AC6** — Given values at the extremes of each domain of E8 and one beyond each extreme, when they are stored, then
   those inside are read equal and those outside fail with invalid value; given a `decimal` with more decimals than
   its scale in each case of tie and of sign, and an instant with a fraction of 100 ns, when they are stored, then
   they are read rounded as E8 says; 0001-01-01T00:00:00.0000005Z is read 0001-01-01T00:00:00.000001Z; and 99.995 in
   a `decimal(4, 2)` and 9999-12-31T23:59:59.9999995Z fail with invalid value.
7. **AC7** — Given `bit` vectors with a single bit set in each position of a byte, a known `f16`, a vector with `-0`
   and `string` with U+0000 and empty, when they are stored and read, then the bits are where E7 says, the `-0` is
   read `+0` and the texts are read equal; and the empty text and the empty bytes work as the primary key.
8. **AC8** — Given `ulid` and `uuid` from each generator, when they are converted to text and read back, then they
   give the same value in the form of E9; and each form not admitted by E9 fails with invalid value.
9. **AC9** — Given a table with a two-column primary key, when ranges are traversed with each prefix and each
   combination of included, excluded and absent bounds, forwards and backwards, then the rows of E12 come back
   (AC-N1); an inverted range returns none; each malformed range of E12 fails with invalid value, and a bound that
   does not fit in the key, with invalid size even if the range is empty; when a range is deleted, then it returns
   how many rows it deleted and those outside remain (AC-N3); reading a row that does not exist returns that it is
   not there, and deleting it returns false; and asking for the number of rows reads the same pages with 10 as with
   100,000 rows, according to its test hook (AC-N3).
10. **AC10** — Given a table with rows, when a bulk load has, before committing, a row with a duplicate key, an
    invalid one, one that fails being the last, a source that fails, a cancellation or a panic,
    then the table, the number of rows and the sequences stay as before and another write then obtains its
    turn (7.2 adds the case of a table with indexes); when
    100,000 rows are loaded out of order, then the table has the same rows as if they had been inserted one by one and
    it reports 100,000; an empty load changes nothing; when a thread with the write turn starts
    a load, it fails with misuse; and a load that does not obtain its turn before its time limit fails with
    timeout (AC-N3).
11. **AC11** — Given a sequence, when a write asks for values and commits, another asks for values and aborts, a
    table is emptied, rows are deleted and a process crash is simulated and it is reopened, then the next values
    continue from the last committed one and none repeats a committed one; a sequence created with first value 1,000
    gives 1,000, and Open or create with another first value does not change it; one at the maximum fails without
    effect with invalid value; asking for a value in a read is not possible; the sequence operations on a table or a
    tree, and the other way around, fail with different type; and renaming a table without overwrite to the name of a
    sequence fails with tree already exists (AC-N4, AC-N5).
12. **AC12** — Given 100,000 values from each generator of E15 from a single thread, with the test clock that stops,
    goes backwards and advances, and with the test random part close to being exhausted within a millisecond, when
    they are sorted with the order of their column (that of SQL Server, checked with `SqlGuid`, for the sequential
    `uuid`), then they are in the order in which they were generated; when several threads generate at once, then all
    the values are distinct, those of each thread grow, and a value generated after another thread finished its own is
    greater than all of them; given a simulated failure of the system generator, the function fails with input/output; and given
    test vectors stored in the repository, each with a fixed millisecond and fixed random bits and the expected
    bytes, when each generator produces them with the test clock and bits, then it gives exactly those bytes: those of
    the sequential `uuid`, generated once with the layout of the user's .NET generator and sorted
    with `SqlGuid`, also in the order of the vectors; those of the `uuid` version 7, with their version and variant
    bits intact after a carry that crosses them; and those of each backwards and overflow rule; and with the test clock
    at the last 48-bit millisecond and the increment exhausted, the function fails with invalid value (AC-N4).
13. **AC13** — Given the operations of E16, when they are run in a write that does nothing else, then exactly those
    that E16 says are changes for T10; an earlier reference to any operation that can modify
    stops being valid; and listing the catalog shows no internal tree of a table, which no name reaches as a tree
    according to its test hook (AC-N5).
14. **AC14** — Given two consecutive levels of the precedence of E17, and its two exceptions, when an operation causes
    a failure of each one at once, then the one E17 says wins; when a row or a schema breaks two
    rules, in two columns or in the same one, then the one that E5 and E17 say is reported: for example, with the schema
    `id, a, b`, `a` and `b` without nulls, the row `[(b, 2), (b, 3), (id, 1), (a, null)]` gives repeated column in `b`, the
    row `[(n, null), (n, 5)]` with `n` without nulls gives repeated column in `n`, and a row without `id` and with `a` null gives
    missing key column in `id`; and the errors carry the data of E17.
15. **AC15** — Given a primary key of exactly 2,048 bytes and another of 2,049, a row of exactly 256 MiB and another
    one byte larger, in the form of E18, and column names of 0, 1, 1,024 and 1,025 bytes, when they are used,
    then those inside work and those outside fail without effect with invalid size and its data.
16. **AC16** — Given the loads W23–W29, when the complete benchmark is run, then the report shows 29 thresholds,
    `no aplica` in the engines that are not Voron, the table of sizes the four loads of E21, and the checksums of W26
    and W27 match in both engines; and the reduced run uses the quantities of its column (AC-N9). The closing report
    includes the single-workload measurements of AC-N10.
17. **AC17** — Given SQL Server order test vectors stored in the repository, generated once with .NET `SqlGuid` as a
    reference independent of Krummi, when the rows of a column with that order are sorted, then they are in the order
    of the vectors (AC-N1). They test the semantics of .NET, not the results of a SQL Server instance.
18. **AC18** — Given a store with tables, when an engine with format 10 opens it, then it fails with the format
    version (B12); and the information of the store does not change shape.
19. **AC19** — Given a table and a sequence with a page of their own damaged, when Open, Create new, Open or create,
    Exists, Describe, List, Rename without overwrite or to its own name and the number of rows are requested, then
    none fails with damage; and reading, storing, traversing or asking for a value that need that page fail with the
    damage class and its page number (E19).

## Annotations to other PRDs

This PRD changes rules of earlier documents, which carry the annotation of 2026-11-10 (in their translations too):

- [tables.prd.en.md](tables.prd.en.md): N1 and AC-N5 (three object classes in the catalog, with sequences); N4 (there
  are no autonumeric columns); N6 (update changes only the columns that are given, E10; sequences instead of
  autonumeric columns, E14; generators as functions, E15); N10 (fourth class, invalid schema, E5); AC-N3 (the second
  insert or update updates the columns given; the sequences stay as before a failed load, instead of the autonumeric
  counter, and the cases of a generated key that collides and of an exhausted counter disappear); AC-N4
  (sequences instead of the autonumeric column); AC-N7 (removing a column of an index fails with invalid
  schema); *Out of scope*: «Updating part of a row: update replaces the whole row (N6)» and «Resetting or setting the
  counter of an autonumeric column» are removed, the latter becoming resetting or setting a sequence after creating it.
- [trees-keys.prd.en.md](trees-keys.prd.en.md): A1, A3, A6, A9 and A10 hold also for sequences (E14, E19, E16), and
  the classes of A6 name a table or a sequence; A7 (the precedence of E17: misuse and the timeout of a bulk load come
  first, and the invalid size that the schema needs comes after the failures of the catalog, of the schema and of
  invalid value).
- [trees.prd.en.md](trees.prd.en.md): B3 (sequences share the catalog like tables); B7 (the same precedence of E17).
- [benchmark.prd.en.md](benchmark.prd.en.md): the workload table adds W23–W29; K4 (W26, W27, W28 and W29 start in the
  store of W23, each one in its own copy); K7 and *Output* (29 thresholds); K9 (W23, W24, W25 and W29).
- Table design documents (not PRDs): documents 1 and 11 (P7) follow the domains of E8; 2 and the README change
  the autonumeric columns for sequences; 3 generates the SQL Server order vectors with `SqlGuid`, not from a SQL
  Server instance.

## References

- Parent: [tables.prd.en.md](tables.prd.en.md); ancestors: [krummi.prd.en.md](krummi.prd.en.md).
- Reused rules: [trees.prd.en.md](trees.prd.en.md), [trees-keys.prd.en.md](trees-keys.prd.en.md),
  [trees-scan.prd.en.md](trees-scan.prd.en.md), [transactions.prd.en.md](transactions.prd.en.md),
  [benchmark.prd.en.md](benchmark.prd.en.md), [pager.prd.en.md](pager.prd.en.md).
- Design: `docs/design/tables/` (README, documents 01, 02, 03 and 11).
