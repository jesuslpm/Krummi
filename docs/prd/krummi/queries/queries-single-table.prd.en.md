# Single-table queries — PRD

**Version**: 1.0
**Date**: 2026-11-28
**Parent document**: [Krummi Queries — PRD](queries.prd.en.md) (v1.0)

> English translation of [queries-single-table.prd.md](queries-single-table.prd.md). The Spanish original is
> authoritative if they differ.

## Objective and context

Build **17.1, single-table queries**, the first child PRD of the [split of the queries](queries.prd.en.md#split-into-child-prds):
an application describes in a JSON document, or with a fluent Rust API, which rows of a table it wants, filtered,
ordered, limited, with some of their columns or with aggregates over all the filtered rows; Krummi validates it, plans
it by cost over the primary key, the indexes or a full scan, and returns the rows through a cursor. It is the first
time an application of Krummi reads without saying how to read.

This PRD inherits everything from its ancestors ([queries.prd.en.md](queries.prd.en.md) and
[krummi.prd.en.md](../krummi.prd.en.md)) and fixes what the *Fixes* cell of 17.1 assigns to it. The technical starting
state is design documents 4, 8, 9 and 11 ([README](../../../design/tables/README.md)); where they differ, 11 wins,
except where this PRD declares otherwise (CU24). What changes or refines earlier rules is annotated (see
[Annotations to other PRDs](#annotations-to-other-prds)).

### Product decisions

| ID | Decision |
|---|---|
| CU1 | **Surface**: in Rust, in any transaction, read or write (CS1). A query reads a single table. The fluent API produces a query document without `bind`, equal, byte for byte, to the one Krummi writes for the same query written in JSON (AC-CS2): each constant it receives is a parameter of its own, without a name, of the Rust type of the constant (an `i32` gives an `i32`, a text gives a `string`), numbered in the order in which it appears when building. Besides preparing and executing, 17.1 offers the description of the store, validating without executing, rewriting a document in Krummi's form without normalizing it, and the JSON Schema of each document type (CU20). The plans, the explanation, the bounded execution and the comparison arrive with 17.2. |
| CU2 | **Query document** (`query/1`, CS3): an object with `krummi`, `table` (the name of the table), `bind` (optional, CU5), `params` (the declarations of the parameters, in order; mandatory, also empty) and `root` (the root node). The nodes form a chain joined by `input`, from the bottom up: `scan` (no other members); `filter` (`predicate`), optional; and then **either** `aggregate` (`aggregates`), which ends the chain, **or**, in this order and each one optional, `sort` (`keys`), `limit` (`count` and `offset`, both optional but at least one) and `project` (`columns`). A chain in another order, with a repeated node, or with `aggregate` and any of `sort`, `limit` or `project` is an invalid document. Without `project` or `aggregate`, the result has all the columns of the table in the order of its schema. A document carries no literal values: every constant is a parameter (P15). |
| CU3 | **Expressions** of the predicate of `filter`: `eq`, `lt`, `le`, `gt` and `ge`, with `col` (a column name) and `value` (`{"param": i}`); `in`, with `col` and `values` (`{"param": i}` of a parameter of list type); `is_null`, with `col`; `and` and `or`, with `args` (two or more expressions); and `not`, with `arg`. There is no `ne` or `between` in the document: the fluent API writes them as `not` of `eq` and as `and` of `ge` and `le`. A comparison is always of a column with a parameter; between columns arrives with 17.5. |
| CU4 | **Execution document** (`execution/1`, CS3): `krummi`, `statement` (a query document), `args` (one value per declaration, in their order, in the form of CU6; as many as declarations) and `limits` (optional), an object with the execution budgets of CU16 by their name (`memory_bytes`, `spill_bytes`, `rows_examined`, `deadline_ms`, `in_list`), each one a `u64` in its text form. A missing budget takes the store's; one in another form or above its hard maximum fails with invalid options. The values of `args` are checked after validating `statement`: one in another form, also of another JSON kind, fails with invalid value and the path `/args/i`. |
| CU5 | **Parameters and `bind`**: a declaration is `{"type": ...}` with an optional `name`, unique in the document and which appears only in errors. The types are those of N4 except vector and JSON, with their descriptor (`{"type":"decimal","precision":12,"scale":2}`, `{"type":"uuid"}`), and the list for `in` (`{"type":"list","element":{...}}`, with an element that is not a list). A parameter, or the element of an `in` list, matches a column of its same type and, among numbers, any column of its family: an integer with any integer column, an `f32` or `f64` with any floating-point column, and a `decimal` with any `decimal`; the comparison is by the exact value, without converting or rounding (300 against a `u8` column is greater than all its rows). The other types match only their own: a `string` with any collation, a `uuid` with any order. User: «Por familia, exacto (Recomendado)». `count` and `offset` are `u64` parameters. A declared and unused parameter is an invalid document. `bind` is `{"table": <identity>, "columns": {<name>: <identity>, ...}}`, with the identity of the table and that of each column the query uses, all as 16 lowercase hexadecimal digits; Krummi writes `columns` in the order of the bytes of their names; validating without executing (CU20) and the explanation of 17.2 write it, not the fluent API. |
| CU6 | **Forms of values** (§2.4 of document 11): the [table of forms](#forms-of-values) says the form Krummi writes, the ones it accepts and the ones it rejects. They are strict: a parameter with more decimals than its scale, or an instant with more than six decimals, is rejected instead of rounded as when storing (E8), because a document has a single form per value. A value in another form, or outside the E8 domain of its type, fails with invalid value. A value of any size valid for its type is accepted, also one that would not fit in an index key: the result is the same with any plan (CS7). |
| CU7 | **Normalization** (document 4): when preparing, Krummi flattens nested `and` and `or`, pushes each `not` inward by De Morgan's laws down to an order comparison, which it inverts (`not` of `lt` is `ge`, and so on), or down to an `eq`, an `in` or an `is_null`, on which it stays, removes double negations, removes repeated arguments of an `and` or an `or`, replaces an `and` or an `or` left with a single one by its argument, and orders the arguments: first the comparisons, `in` and `is_null` (also under a `not`), by the name of their column (its bytes), then by operation in the order `eq`, `lt`, `le`, `gt`, `ge`, `in`, `is_null`, with its `not` right behind, and then by parameter number; then the nested `and` and `or`, `and` before `or`, and between two of the same operation comparing their already ordered arguments one by one with these same rules, up to the first one that differs; if one is a prefix of the other, the shorter goes first; it never rewrites the predicate in disjunctive or conjunctive normal form. The normalized form is the one returned by validating without executing and the one the explanation will carry (17.2); the fluent API writes the document as it was built (this changes document 4, which had it normalize). When executing, an `in` list is sorted and its repeated values removed, after normalizing `-0` and NaN (N4) and, in a `ci` or `ci_ai` column, by its folded form. |
| CU8 | **Comparisons**: with the order of N5 of the tables, that of the indexes and of the ranges: `-0` is `+0`, every NaN is equal to NaN and greater than +∞, `ci` and `ci_ai` text is compared by its folded form (IC2), and a `uuid` with the SQL Server order, in that order. With a null on either side, a comparison or an `in` is unknown; an `in` with an empty list is false, also over a null; a null list makes the `in` unknown on every row; `is_null` is true or false. `not` of unknown is unknown, and `and` and `or` follow three-valued logic (CS7). Vector and JSON columns admit `is_null`, projection and `count`; any other comparison, `ORDER BY`, `min`, `max`, `sum` or `avg` over them is an invalid document. |
| CU9 | **Projection and result columns**: `columns` is a non-empty list of column names of the table, without repeats; the result has those columns in that order, with their names, their types and whether they admit nulls according to the schema. It is possible to order by a column that is not projected. The cursor describes the columns of its result: name, type and whether it admits nulls. |
| CU10 | **Order**: `keys` is a non-empty list of `{"col": ..., "desc": bool, "nulls": "first" or "last"}`, without repeated columns; when reading, `desc` is missing as false and `nulls` as `"first"` in both directions, like Krummi's indexes in their forward traversal (N5), so an ascending order without more can come out of an ascending index created without options, and a descending one from a descending index created without options; Krummi always writes them. User: «Primero en las dos (Recomendado)». Rows tied in all the keys come out in an unspecified order (CS7). |
| CU11 | **`LIMIT` and `OFFSET`**: `offset` skips that number of rows of the result and `count` leaves at most that number of the following ones; a `count` of 0 gives an empty result; values beyond the number of rows, or whose sum exceeds `u64`, are not an error. A null in either of the two fails with invalid value when executing. |
| CU12 | **Aggregates**: `aggregates` is a non-empty list of `{"fn": ..., "col": ..., "as": ...}`, where `fn` is `count`, `min`, `max`, `sum` or `avg`; `col` is missing only in `count`, which then counts all the rows (`count(*)`); and `as` is the name of the result column, unique and with the rules of the column names of 7.1. The result is always a single row. `count` is `u64` and never null, and with a column it counts its non-null values. `min` and `max` admit the types that are ordered (all except vector and JSON), are of the type of their column and admit null; the other aggregates ignore nulls. Over zero rows, or if all its values are null, `count` gives 0 and the others give null. `min` and `max` of a `ci` or `ci_ai` column return the stored text of one of the rows with the minimum or maximum folded form, unspecified which. `sum` and `avg` admit integers, floating-point types and `decimal` (not `duration`): `sum` of signed integers is `i64`, of unsigned integers `u64`, of `f32` or `f64` is `f64` and of `decimal(p, s)` is `decimal(38, s)`; if the exact sum of all the integer or `decimal` values exceeds the range of its type, the execution fails with invalid value, naming the aggregate, whatever the order in which they are added. Floating-point `sum` is the exact sum of all the values rounded a single time to the nearest `f64`, ties to even, so it does not depend on the plan or on the order of summation: it gives ±∞ only if that exact result does not fit in an `f64`; a NaN among the values gives NaN, and +∞ with −∞ too. `avg` of integers and of floating-point is the exact sum divided by the count, rounded a single time to the nearest `f64`, ties to even, like `sum`, and with the same rules for NaN and the infinities; of `decimal(p, s)` it is `decimal(38, s)` with the rounding of E8; `avg` never overflows. User, on the precision of floating-point sums: «Exacta y sin depender del plan (Recomendado)». User: «i64/u64, error al desbordar (Recomendado)». |
| CU13 | **Names and stores**: preparing with a table that does not exist fails with not found, and with a name that belongs to a tree or a sequence, with different type, like opening the table (7.1); with a column the table does not have, with invalid document. A document with `bind` whose identities do not match, also if its table no longer exists, fails with different schema ([invalidation](queries.prd.en.md#invalidation-of-statements)). Executing a statement in a transaction of another store fails with misuse. Closing the store fails with misuse while statements or sets of values live; a cursor borrows its transaction and cannot outlive it. Queries do not occupy the projections that the store keeps for the application (PY5). |
| CU14 | **Executing**: the call that executes checks, in this order, that the statement belongs to the store of the transaction, that all the parameters have a value (misuse), the options of the execution (invalid options), the catalog the transaction sees (CS5: it binds again or fails with different schema), the values of `count` and `offset` (invalid value) and the budget of the `in` lists (limit exceeded); with the first failure it returns no cursor. If it must replan, the call that executes waits for the sampling (ST3); that wait counts in the time of the execution, and if when the wait ends the time ran out or the execution was cancelled, the call fails with its class and returns no cursor. An already cancelled token makes the call fail with cancelled. A query inside a write sees the changes that write made before executing it. |
| CU15 | **Cursor**: returns the rows in batches of up to the size the application asks for when executing, from 1 to 65,536 rows, 256 by default; a batch can have fewer rows without being the last, and the end is told by a read without rows. The errors of the execution appear in the reads; the read that fails returns no row of its batch, and the rows of the previous batches were valid. After the end, every read again says it finished; after an error or a cancellation, every read fails with misuse. Dropping the cursor without reading it entirely ends the execution (CS9). |
| CU16 | **Limits** (CS8): those of the [table of limits](#limits), with their default value and their hard maximum. All are configured when opening the store; those of the documents and the planning also when preparing, and those of the execution (memory, spilling, rows examined, time and `in` lists) also when executing. An absent budget takes the store's value; `0` is zero, not «no limit». Rows examined are the rows and index entries that the execution decodes from the store (a shortcut such as `count(*)` by the number of rows examines none); time is wall-clock, from the call that executes until the cursor ends, including the time between reads. They are checked, with cancellation, in every batch and every 1,024 rows that the execution examines or its sort moves, so an execution notices them at most 1,024 rows later. A time that the clock cannot represent counts as no limit. Reaching the limit exactly does not fail; exceeding it does. If several are met at once, cancellation wins, then time, and then the budget. The application cancels with a token it gives when executing, from any thread; one same token can be given to several executions and cancels them all; cancelling after the end or after dropping the cursor does nothing. A budget asked for above its hard maximum, or a batch size outside its interval, fails with invalid options, naming the budget and its maximum. |
| CU17 | **Spilling** (CS9): the sort spills to disk, with files of its own for each execution, in `krummi.sort`, the reserved name that creating an index already uses (P3 of the Pager): each execution deletes its own when it ends, fails, is cancelled or its cursor is dropped; the folder is deleted when it has no files left, and also when closing and opening the store; it does not count towards the maximum size or the store's information. What was spilled is checked when read back, and a damaged or short spill fails with input/output, like creating an index (IX2): it is not damage to the store (G6). A failure when spilling or reading it back makes only that execution fail, also inside a write, which can keep committing (CS9). In temporary mode there is no spilling nor is the spill budget counted: the sort uses memory and, past its budget, fails with limit exceeded. |
| CU18 | **Write transactions**: a query does not change the write it runs in; if it fails because of a budget, the time, a cancellation or its spilling, the write can keep committing. Damage or an input/output failure when reading the store follows T11 and T12, like any read of the write. |
| CU19 | **Errors** (CS10): new classes **limit exceeded**, for an exhausted budget of an execution (memory, spilling, rows examined, `in` list), and **cancelled**. The exhausted time uses the timeout class (T4), which now also covers the time of an execution. A document of a version later than the reader's is an invalid document. The error object of CS10 carries `class` (the names of the [table of classes](#classes-in-the-error-object)), `code` (only in invalid document), `path`, `expected`, `found` and `message`; a malformed JSON carries `line` and `column` instead of `path`. The [table of codes](#invalid-document-codes) fixes the phase of each code and what its path points to; all carry `expected` and `found` except `malformed_json` and `too_large`. Outside invalid document: limit exceeded carries in `found` the name of the budget and in `expected` its value; different schema, in `expected` the fixed identity and in `found` the current one, or null if it no longer exists; invalid value, in `found` the value or the `as` name of the aggregate that overflows. Validation follows the phases of §2.5 of document 11 (syntax, structure, binding, types; admission is that of CU14): the first failure in the order of the phases wins and, within a phase, that of the value that appears first when reading the document. The paths are those of the document sent (from `/statement/...` in an execution one; of the document the fluent API wrote, not normalized). An execution error carries the path of the query node whose work failed: the `scan` for rows examined, damage or an input/output failure when reading the store; the `sort` for its memory and its spilling; the `in` expression for its list; the entry of `aggregates` for an overflow; time and cancellation carry no path. |
| CU20 | **Functions for agents** (CS12): the **description of the store** is a `store/1` document, a fifth document type that only Krummi writes: `{"krummi": "store/1", "tables": [...]}`, with the tables in the order of the bytes of their name, each one `{"name", "id", "schema_version", "schema_state", "index_state", "columns", "primary_key", "indexes", "vector_indexes"}`; each column, in schema order, `{"name", "id", "type", "nullable"}`, with `type` the descriptor of the column: that of CU5 for types that have no more attributes; `{"type": "uuid", "order": "rfc9562" or "sql_server"}`; `{"type": "string", "collation": "binary", "ci" or "ci_ai"}`; `{"type": "vector", "element": "f32", "f16", "i8" or "bit", "dim": ..., "cosine": true or false}`; and `{"type": "json"}`. These column descriptors belong to `store/1`; those of the parameters remain those of CU5, where a `uuid` or a `string` matches any order or collation; `primary_key`, the names of its columns; each index, in name order, `{"name", "id", "unique", "keys": [{"col", "desc", "nulls"}], "include": [...]}`; and each vector index of 7.6, `{"name", "id", "col"}`. Identities and marks are 16 lowercase hexadecimal digits and the schema version an integer. **Rewriting** reads a query or execution document and writes it in Krummi's form, without normalizing it. **Validating without executing** reads a query, validates it against the catalog the transaction sees and returns its normalized form with `bind`, or the error. The JSON Schema of each type that exists in 17.1 (`query/1`, `execution/1`, `store/1`) is returned by a function of the library; it is not distributed as files (question 4 of document 11). |
| CU21 | **Planner** (CS5, CS7, CS8): chooses, by the estimates of 7.3, between reading by the primary key, by an index (also only its entries, when they have all the columns the query needs) or scanning the whole table, and can combine several paths; avoids sorting when a path already gives the requested order. Its result is admissible with any plan. Preparing waits for a sampling without a limit, like the estimation (ST3); the wait when explaining is fixed by 17.2. The reservations M2 and M7 of CS13 are checked by the design review. |
| CU22 | **Measurement** (CS14): three new loads of the benchmark, each with its direct operation as reference and the threshold of 0.80, over the store and the data of that operation: **W33** `consulta-lecturas`, like W26 with a statement that filters by the primary key; **W34** `consulta-intervalos`, like W27 with a statement that filters from a key, sorts by the primary key and limits to 100; **W35** `consulta-intervalos-de-indice`, like W32 with a statement that filters from a `ts`, sorts by `ts` and by the primary key and limits to 100. The statement is prepared once outside the measurement; each measured operation gives its values, executes and reads all its rows, with the same checksum as its reference. They enter the reduced execution with the quantities of their reference and without a threshold; Voron and the other engines say `no aplica`. The closing report compares W33 to W35 and a query that filters by a range of `n`, sorts by `ts` descending and limits to 20 with the same ones in SQLite, with the same data and indexes loaded before measuring, prepared statements and the write-ahead log (WAL), without a threshold. |
| CU23 | **Tests**: AC-CS1 with 10,000 generated queries with seed 17, written in the test; the fuzzing of AC-CS3 is a property test with 100,000 mutated documents, which fixes by number of cases what the parent asked for by time; AC-CS7 and AC-CU9 simulate versions with a test hook that changes the reader's version; AC-CU17 stops the sampling with the hook of 7.3 (AC-N8); AC-CU8 alters the spilling with a test hook. |
| CU24 | **Differences with the design**: compared with document 11, `count` and `offset` are `u64` (its example used `u32`); dates and instants only have the form of the years 0001 to 9999 and `duration` the domain of E8, not that of an `i64`; a `string` or some `bytes` are accepted of any size, not only up to the maximum of their column (CU6); the order keys carry `nulls`; an unused parameter is an error; and there is a fifth document type, `store/1` (it changes CS3, which said four). Compared with document 4, the fluent API does not normalize (CU7). |

### Forms of values

| Type | Form Krummi writes | Also accepts | Rejects (everything else; for example) |
|---|---|---|---|
| `bool` | `true`, `false` | — | `0`, `"true"` |
| `i8`–`i32`, `u8`–`u32` | Integer number | — | `1.0`, `1e2`, `-0`, out of range |
| `i64`, `u64` | Decimal text, `"-9223372036854775808"`, `"18446744073709551615"` | — | Numbers, `"+1"`, `"007"`, `"-0"`, out of range |
| `f32`, `f64` | The shortest number that returns to the same bits; `"NaN"`, `"Infinity"`, `"-Infinity"` | Any JSON number, rounded to the nearest value of the type with ties to even, also among the subnormals and zero: subnormals are kept, and only what falls below half of the smallest subnormal, or at exactly that half, gives zero (`1e-45` in an `f32` gives the smallest subnormal) | Other texts; a number that overflows when rounded (`1e400`) |
| `decimal(p, s)` | Text with exactly `s` decimals (no point if `s` is 0), without exponent, without `+`, without leading zeros or sign on zero | — | `"1234.5"` with scale 2, `"1.2e3"`, `"-0.00"`, more than `p` digits |
| `string` | JSON text, with escapes only where JSON requires them | Any JSON escape of the same characters | Lone surrogates |
| `bytes` | Base64 with the alphabet of RFC 4648 §4, with padding and padding bits at zero | — | base64url, without padding, non-zero padding bits, spaces |
| `date` | `"2026-11-10"` | — | `"2026-1-1"`, `"+2026-11-10"`, outside E8 |
| `time` | `"13:45:07.123456"`, with six decimals | — | `"13:45:07"`, more or fewer than six decimals, second 60 |
| `timestamp` | `"2026-11-10T13:45:07.123456Z"`, with uppercase `T` and `Z` and six decimals | — | With a zone, lowercase, more or fewer than six decimals, outside E8 |
| `duration` | Decimal text of microseconds, within E8 | — | `"PT1M30S"`, outside E8 |
| `uuid` | RFC 9562 text in lowercase, in the RFC byte order, also in columns with the SQL Server order | — | Uppercase, braces, without hyphens |
| `ulid` | 26 uppercase Crockford Base32 characters, the first at most `7` | — | Lowercase, `I`, `L`, `O`, `U`, first greater than `7` |
| list | Array of values of the type of its element | — | Nested lists, null elements |
| null | `null`, for any type | — | — |

### Limits

| Budget | Name | Default | Hard maximum |
|---|---|---|---|
| Size of a document | `document_bytes` | 1 MiB | 64 MiB |
| JSON nesting of a document (ceiling, not a budget) | — | 128 | 128 |
| Depth of nodes and expressions | `depth` | 64 | 128 (the JSON nesting ceiling may be reached first) |
| Nodes and expressions of a document | `nodes` | 4,096 | 65,536 |
| Parameters of a document | `params` | 1,024 | 65,536 |
| Planning work | `planning` | 65,536 | 1,048,576 |
| Entries of the plan cache (per store only) | `plan_cache` | 256 | 65,536 |
| Elements of an `in` list | `in_list` | 10,000 | 1,000,000 |
| Memory of an execution | `memory_bytes` | 64 MiB | 16 GiB |
| Spilling to disk of an execution | `spill_bytes` | 1 GiB | 1 TiB |
| Rows examined of an execution | `rows_examined` | no limit | — |
| Time of an execution | `deadline_ms` | no limit | — |

### Invalid document codes

The phases are those of CU19. The path points to the value that fails, except where otherwise stated.

| Code | Phase | Situation | Path |
|---|---|---|---|
| `malformed_json` | Syntax | It is not well-formed JSON or not UTF-8 | No path: `line` and `column` |
| `too_large` | Syntax | Exceeds `document_bytes` | `""` (the document) |
| `too_deep` | Syntax | Exceeds the JSON nesting ceiling, or `depth` (in the structure phase); `found` says which | The first value that is too deep |
| `duplicate_member` | Syntax | A repeated member; `found` names it | The object |
| `unsupported_version` | Structure | A `krummi` that is not a type and a version, a type that is not accepted here (`plan/1` and `explain/1` in 17.1, `store/1` always), or a version the reader does not know or is later | `/krummi` |
| `removed_construct` | Structure | A removed construct; `found` names it with the version that removed it | The construct |
| `unknown_member`, `missing_member` | Structure | A member that does not belong, or a mandatory one is missing; `expected` names it | The member; the object if missing |
| `unknown_op` | Structure | An operation the version does not have | `op` |
| `wrong_kind` | Structure | A JSON value of another kind than expected, except in `args` (CU4) | The value |
| `invalid_option` | Structure | A value of `fn` or `nulls` that does not exist | The value |
| `invalid_type` | Structure | An invalid type descriptor (precision or scale outside N4, list of lists, vector or JSON) | The descriptor |
| `invalid_arity` | Structure | An `and` or `or` with fewer than two arguments, or empty `columns`, `keys` or `aggregates` | The list |
| `invalid_chain` | Structure | The chain of nodes does not meet CU2 | The misplaced node |
| `too_many_nodes`, `too_many_params` | Structure | Exceeds `nodes` or `params` | `""` and `/params` |
| `invalid_param_ref`, `unused_param` | Structure | A reference to a parameter that does not exist, or one declared and unused | The reference; `/params/i` |
| `duplicate_name` | Structure | A repeated name of a parameter, projected column, order key or `as` | The second occurrence |
| `invalid_args` | Structure | `args` of a different length than `params` | `/args` |
| `invalid_name` | Structure | An empty `table` or longer than B4, or an `as` that does not meet the rules of the column names of 7.1 | The name |
| `invalid_bind` | Structure | A `bind` with an identity that is not 16 lowercase hexadecimal digits, or that does not have exactly the columns the query uses | The value or `bind` |
| `unknown_column` | Binding | A column the table does not have, without `bind` (CU13) | The name |
| `type_mismatch` | Types | A parameter that does not match its column, `count` or `offset` that are not `u64`, or an unsupported operation over its column (CU8) | The expression or the node |

### Classes in the error object

| Class | `class` |
|---|---|
| Access | `access` |
| Store in use | `store_in_use` |
| Not a store | `not_a_store` |
| Damaged | `damaged` |
| Different format | `different_format` |
| Disk full | `disk_full` |
| Input/output | `io` |
| Invalid options | `invalid_options` |
| Maximum size | `max_size` |
| Out of memory | `out_of_memory` |
| Timeout | `timeout` |
| Misuse | `misuse` |
| Invalid size | `invalid_size` |
| Different type (different tree type) | `different_type` |
| Not found | `not_found` |
| Already exists | `already_exists` |
| Invalid value | `invalid_value` |
| Duplicate key | `duplicate_key` |
| Different schema | `different_schema` |
| Invalid schema | `invalid_schema` |
| Invalid document (new, CS10) | `invalid_document` |
| Limit exceeded (new) | `limit_exceeded` |
| Cancelled (new) | `cancelled` |

### Starting state

Verified on 2026-11-28 in the code and in the design documents:

| Aspect | Situation |
|---|---|
| Queries | They do not exist. No crate depends on serde in its normal dependencies; `serde_json` is only a test dependency of the `krummi` crate (AC-JS11). |
| Errors | Classes of D8 in the code: access, store in use, not a store, damaged, different format, disk full, input/output, invalid options, maximum size, out of memory, timeout, misuse, invalid size, different tree type, not found, already exists, invalid value, duplicate key, different schema and invalid schema. There are no cancellation or limit exceeded classes. |
| Transactions | A read can be passed to other threads and shared; a write stays in its thread (T6). Closing the store with an open transaction does not compile. Traversals borrow their transaction; in a write, they live until the next operation that can change it. |
| Reads | By primary key; ranges of the primary key and of an index, forward and backward; the entries of an index without reading the row. A range of an index can ask only for the nulls of a column, but not the nulls and a range of values together; a bound of more than 2,048 bytes fails with invalid size (IX9). |
| Order | Ascending or descending indexes per column, with nulls first by default in the forward traversal; the primary key is ascending and without nulls (N5). |
| Estimation | Equality (with null), range and equality over several columns, in reads and writes, with values of any size; without statistics, it samples and waits (ST2, ST3). |
| Sort with spilling | Creating an index already sorts with 64 MiB of memory before spilling into `krummi.sort`, which opening deletes; in temporary mode it does not spill and keeps sorting in memory without that cap, until the system cannot give it more and it fails with out of memory (IX2). |
| Projections | The store keeps in memory up to 1,024 projections for the application (PY5). |
| Benchmark | 32 loads, W1 to W32, with a threshold against Voron; those of tables are W23 to W32. |

## Actors

- **Integrating developer**: writes queries with the fluent API or in JSON, prepares them, executes them with their
  values and reads the cursor; configures the limits and cancels executions.
- **Agent or tool**: reads the description of the store and the JSON Schemas, validates documents without executing
  them and fixes the errors by their code and their path (CU20).
- **Maintainer**: runs the tests of CU23 and the loads and the comparison of CU22.

## Flow

1. The application builds a query with the fluent API or writes its document (CU1–CU5).
2. It prepares it in a transaction: Krummi reads it, validates it, binds it and plans it (CU7, CU13, CU19, CU21).
3. It gives the values of its parameters and executes it with its limits and its cancellation token (CU6, CU14, CU16).
4. It reads the cursor in batches and, if needed, cancels from another thread (CU15, CU16).

## Validations and messages

Those of the ancestors apply. This PRD fixes these, for the library only:

| Situation | Error class |
|---|---|
| A document that does not meet CU2–CU5 or CU8–CU12, or of a later version | Invalid document, with its code |
| A table that does not exist | Not found |
| A table name that belongs to a tree or a sequence | Different type |
| A document with `bind` whose identities do not match, also with its table dropped | Different schema |
| A value of a parameter in another form or outside its domain, or a null in `count` or `offset` | Invalid value |
| Executing in another store or without all the values, closing with live statements or values, or reading a cursor after an error or a cancellation | Misuse |
| A budget above its hard maximum or a batch size outside 1 to 65,536 | Invalid options |
| An execution that exhausts its memory, its spilling, its rows examined or the budget of an `in` list | Limit exceeded (new) |
| An execution that exhausts its time | Timeout |
| A cancelled execution, also with an already cancelled token when executing | Cancelled (new) |
| When executing, a catalog with identities that no longer match | Different schema (CU14) |
| When preparing, a document or planning budget above its hard maximum | Invalid options (CU16) |
| A `sum` whose exact sum exceeds the range of its type | Invalid value (CU12) |
| A damaged or short spill | Input/output |

## Output

The module writes nothing to the screen. A query for the customers of Spain created since a date, the 20 most recent,
with two columns, in Krummi's form with spaces added to read it:

```json
{ "krummi": "query/1", "table": "customers",
  "params": [ { "type": "string", "name": "country" },
              { "type": "timestamp", "name": "since" },
              { "type": "u64", "name": "n" } ],
  "root": { "op": "project", "columns": [ "id", "created" ],
    "input": { "op": "limit", "count": { "param": 2 },
      "input": { "op": "sort", "keys": [ { "col": "created", "desc": true, "nulls": "first" } ],
        "input": { "op": "filter",
          "predicate": { "op": "and", "args": [
            { "op": "eq", "col": "country", "value": { "param": 0 } },
            { "op": "ge", "col": "created", "value": { "param": 1 } } ] },
          "input": { "op": "scan" } } } } } }
```

Krummi writes the members in the order of CU2 to CU5 (`krummi`, `table`, `bind`,
`params`, `root`; in each node and expression, `op`, its members in the order of CU2 and CU3 and `input` last; in a
declaration, `type` and then `name`), without spaces, and the type and the version as
`"query/1"` for 1.0 and `"query/1.2"` for the others; when reading it accepts `"query/1.0"`, any order of members and
any spacing. Its execution with values, as the tests and tools use it:

```json
{ "krummi": "execution/1", "statement": { "krummi": "query/1", "...": "la consulta de arriba" },
  "args": [ "ES", "2026-01-01T00:00:00.000000Z", "20" ],
  "limits": { "memory_bytes": "67108864" } }
```

An aggregate, `{"op": "aggregate", "aggregates": [{"fn": "count", "as": "n"}, {"fn": "avg", "col": "total", "as":
"media"}], "input": ...}`, returns one row with the columns `n` and `media`.

An invalid document error:

```json
{ "class": "invalid_document", "code": "unknown_column", "path": "/root/input/input/input/predicate/args/1/col",
  "expected": "a column of table customers", "found": "creatd", "message": "unknown column creatd in table customers" }
```

The values of the rows follow *Output* of the parent: without a copy those of `string`, `bytes`, vector and JSON read
from a row, and in a copy of their own those of the keys of index entries, the aggregates and what went through a
spill.

## Reuse

- The reads by key, by ranges of the primary key and of the indexes, and the entries of the indexes (7.1,
  7.2).
- The estimation and sampling of 7.3 for the planner.
- The sort with spilling of creating an index and its reserved name `krummi.sort` (P3 of the Pager, IX2).
- The folded form of 7.7 to compare and sort `ci` and `ci_ai` text.
- The table loads of the benchmark and their data (W26, W27, W32).

## Out of scope

- What belongs to the following child PRDs (17.2 to 17.13): plans, explanation, bounded execution and comparison;
  prefix, `LIKE` and JSON; vectors; expressions, comparisons between columns, computed columns and renaming columns in
  the projection; partial indexes; writes; grouping, `count` of distinct values and aggregates with columns, order or
  limits; joins; subqueries; sets; windows; profile.
- Parameters of vector or JSON type, and therefore their forms (this changes the *Fixes* cell of the parent, which
  asked for those of `json`: they arrive with 17.3 and 17.4).
- Ordering by the position of a column or by an aggregate.
- The `plan/1` and `explain/1` documents and the wait for a sampling when explaining (17.2); reading `store/1` as
  input.
- The C interface and the C# binding (module 18) and the command line (module 12).
- Cancelling or limiting the time of preparing (*Out of scope* of the parent).
- Distributing the JSON Schemas as files.

## Left to the implementation

- The names of the Rust types and functions of the fluent API, the cursor, the token and the limits, and the English
  text of the messages and of `expected` and `found` (D8).
- How the spilling sorts and how many files it uses, as long as it meets CU17 and the budgets.
- How the exact floating-point sum of CU12 is computed, as long as it gives its result.
- The cost model, when some values ask for another plan, the organization of the cache and what counts as the memory
  of an execution (CS5, CS8).
- The exact list of crates of CS11: it is the tree of normal dependencies of `serde`, `serde_derive` (with the crates
  it uses when compiling) and `serde_json` 1.0 with `float_roundtrip`,
  which gives the exact reading of the floating-point numbers of CU6, and without `arbitrary_precision`, `raw_value` or
  `unbounded_depth`; the execution annotates it in D12 of the root parent when adding them.
- The unit of `line` and `column` of `malformed_json`.

## Acceptance criteria

Those of the ancestors apply; this PRD checks with its rules those of its parent that name 17.1 (AC-CS1 to AC-CS13,
AC-CS14 in what concerns 17.1 and AC-CS17), and adds these.

1. **AC-CU1** — Given the reference documents of each node, expression and value type, saved in Krummi's form, when
   they are prepared, executed and rewritten (CU20), then they give the expected result and the same bytes; a document
   with `"query/1.0"`, with another order of members or with spaces is accepted and rewritten in Krummi's form; and the
   fluent API writes, for each document without `bind` or parameter names, the same document (CU1–CU5).
2. **AC-CU2** — Given the examples of each row of the table of forms and, for each type, a value at the E8 limit and
   one outside it, when they are bound, then they are accepted with their value or fail with invalid value as the table
   says (CU6).
3. **AC-CU3** — Given a query with nested `and` and `or`, `not` over each operation, repeated and unordered arguments,
   when validated without executing, then it returns the normalized form of CU7, with the bytes
   its rules give, the same for predicates equivalent by those rules, and an `and` left with one argument
   replaced by it; and an `in` list with repeats, `-0`, NaN and `ci` texts that fold
   the same gives the rows of the normalized list (CU7).
4. **AC-CU4** — Given a table with nulls, `-0`, NaN, ±∞, `ci` and `ci_ai` text and `uuid` in both orders, when
   filtering with each comparison, `in` (also empty and with the null list) and `is_null`, and ordering by each column
   in both directions, with nulls first and last and without saying so, then the rows are those of a reference model
   that compares by the order of N5 and three-valued logic (CU8, CU10).
5. **AC-CU5** — Given a projection, one without `project` and one that orders by a non-projected column, then the
   cursor describes and returns the columns of CU9; and a repeated name fails with `duplicate_name` (CU9).
6. **AC-CU6** — Given queries with `count` and `offset` of 0, 1, the number of rows, more, `u64` maximum both and a
   null, then they return the expected windows or fail with invalid value (CU11).
7. **AC-CU7** — Given aggregates over zero rows, over only nulls, over `ci_ai`, over vector and JSON columns
   (`count`) and over each numeric type, also at the extremes of their type, when executed, then they give the
   values, the types and the nulls of CU12, also the floating-point ones over the cases of cancellation, intermediate
   overflow, NaN and infinities, equal with any plan; for `ci_ai`, a minimum or maximum folded
   form; and a `sum` that exceeds `i64`, `u64` or 38 digits fails with invalid value (CU12).
8. **AC-CU8** — Given an execution whose sort spills, when it is read entirely, when it is cancelled, when it is
   dropped halfway, when it runs at the same time as another that spills and as an index creation, and when a crash is
   simulated and it is reopened, then each one deletes only its own files and `krummi.sort` does not remain when all
   finish; with the spilling altered by the hook of CU23 it fails with input/output and, inside a write, the write
   commits its other changes; and in temporary mode, past the memory, it fails with limit exceeded without creating
   files (CU17, CU18).
9. **AC-CU9** — Given each code of the table of codes, with the versions hook of CU23 for `removed_construct` and
   `unsupported_version`, when a document that causes it is prepared, then it carries that code, its path and
   `expected` and `found` except where CU19 says otherwise; for `invalid_chain`, the path of the first node, from the
   root, that breaks the order of CU2; and documents with two failures of different phases
   and with two of the same phase give the one CU19 fixes (CU19).
10. **AC-CU10** — Given the cases of CU13 and CU14 (table that does not exist and name of a tree; column that does not
    exist with and without `bind`; `bind` of a dropped table; another store; values not given; invalid options; null
    `count`; `in` list over its budget; several of them at once; closing with live statements or values; a query in a
    write after inserting), then each one gives the class, the order and the rows of CU13 and CU14; and creating and
    executing queries does not reduce the 1,024 projections that the application can keep (CU13, CU14).
11. **AC-CU11** — Given a cursor, when read with batches of 1, 256 and 65,536 rows, after the end, after an error,
    after a cancellation and after a late cancellation, and a batch of 0 or 65,537 is asked for, then it behaves as
    CU15 and CU16 say (CU15, CU16).
12. **AC-CU12** — Given an execution with an already expired time, another cancelled and, over a table without indexes
    of N rows, which can only be scanned entirely, another with the rows examined budget at N and at N − 1, and one that
    meets time, cancellation and budget in the same check, then each one ends or fails as CU16 says, with its path of
    CU19; and a filter that discards all the rows notices the cancellation within 1,024 examined rows (CU16, CU19).
13. **AC-CU13** — Given a write that changes a row and then executes a query that fails because of a budget, the time,
    a cancellation and a failure of its spilling, then the write commits its change; and when the query finds damage or
    an input/output failure when reading the store, then the write can no longer commit (T11) (CU18).
14. **AC-CU14** — Given a store with tables, columns of each type and indexes, when its description is asked for, then
    the `store/1` document has the members and the order of CU20, with
    the values of the catalog; validating without executing returns the normalized query with its
    `bind`, and each JSON Schema validates the reference documents of its type (CU20).
15. **AC-CU15** — Given a table with statistics and another without them, when preparing queries that can read by the
    primary key, by an index, by only the entries of an index, by two combined indexes or by the whole table, and
    with an order that an index gives and a `sort` with `limit`, then the test hook of AC-CS1 shows that the
    planner considered each of those paths and whether it avoided the sort; an equality over the whole primary key
    chooses to read by it, with statistics and without them; and the result is admissible; and a text parameter of
    4,096 bytes over an indexed column gives the same result with each path (CU6, CU21).
16. **AC-CU16** — Given the loads W33 to W35, when the benchmark is run, then the report shows them with their ratio
    against their direct operation, `no aplica` in the other engines, and counts their thresholds with the others; and
    the closing report has the comparison with SQLite of CU22 (CU22).
17. **AC-CU17** — Given a prepared statement, when the statistics of its table change so that it must be planned
    again and the sampling is stopped at the hook of 7.3, then the call that executes waits; and if during the wait
    the time runs out or it is cancelled, when the sampling is released the call fails with timeout or with
    cancelled, without a cursor (CU14).

## Annotations to other PRDs

This PRD refines rules of earlier documents, which carry the annotations of 2026-11-28 (in their translations too),
except the one on D12 of the root parent, which the execution will make when adding the crates:

- Root parent, D8 and *Validations and messages*: new classes limit exceeded and cancelled, and the names of the
  classes in the error object (CU19).
- Root parent, G6: a damaged or short spill of a query fails with input/output, like that of creating an
  index; it is not damage to the store (CU17).
- Transactions, T4: the timeout class also covers the time of a query execution
  (CU19).
- Root parent, D12: the list of crates of CS11, when adding them (*Left to the implementation*).
- Queries, CS3 and CS12: a fifth document type, `store/1`, which only Krummi writes, and the rewrite function
  (CU20). CS8: the depth of nodes is a budget apart from the JSON nesting ceiling (*Limits*).
- Queries, CS3, CS8, CS9, CS10 and the *Fixes* cell of 17.1: what they left to 17.1 is fixed here; a table that does not
  exist fails with not found and not with different schema when the document carries no `bind` (CU13); the forms of
  `json` move to 17.3 and 17.4.
- Pager, P3: `krummi.sort` is also used by the spilling of queries, with files of its own for each execution
  (CU17).
- Indexes, IX2: creating an index deletes only its files of `krummi.sort`, and the folder when it is left empty (CU17).
- Benchmark, K7 and *Output*: three new loads, W33 to W35, with their threshold (CU22); «Krummi cumple N de 32 umbrales»
  now counts 35. K4 and K11: they start in the store of their reference (W26, W27, W32) and their reduced execution has
  the quantities of that reference.
- Projections, «las consultas del módulo 17 fijarán si las usan»: they do not occupy them (CU13).

## References

- Parent: [queries.prd.en.md](queries.prd.en.md) v1.0 (CS1–CS14, AC-CS1–AC-CS17); root: [krummi.prd.en.md](../krummi.prd.en.md)
- Tables: [tables.prd.en.md](../tables/tables.prd.en.md) (N4, N5), [tables-schema.prd.en.md](../tables/tables-schema.prd.en.md) (E1,
  E8), [tables-indexes.prd.en.md](../tables/tables-indexes.prd.en.md) (IX2, IX9),
  [tables-statistics.prd.en.md](../tables/tables-statistics.prd.en.md) (ST2, ST3),
  [tables-collation.prd.en.md](../tables/tables-collation.prd.en.md) (IC2),
  [tables-projection.prd.en.md](../tables/tables-projection.prd.en.md) (PY5)
- Pager: [pager.prd.en.md](../pager/pager.prd.en.md) (P3); transactions: [transactions.prd.en.md](../transactions/transactions.prd.en.md) (T4, T6, T11, T12)
- Benchmark: [benchmark.prd.en.md](../benchmark/benchmark.prd.en.md) (K4, K6, K7, K11, K13, K14)
- Design: [docs/design/tables/](../../../design/tables/README.md) documents 4, 8, 9, 10 and 11

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| 77da4411 | 2026-11-28 09:18 UTC+00:00 | 6 h 2 min | 3 h 17 min | 2 h 45 min | 1 h 39 min | 1 h 6 min | 24 |

<!-- prd-time: [{"session": "77da4411-edaa-49ca-96e6-3afe464b2b23", "start": "2026-11-28T09:18:54.972000+00:00", "wall": 21717, "discarded": 11796, "agent": 5944, "human": 3976, "interventions": 24, "idle_minutes": 10}] -->
