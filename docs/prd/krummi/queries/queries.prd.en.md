# Krummi Queries — PRD

**Version**: 1.0
**Date**: 2026-11-28
**Parent document**: [Krummi, an embedded storage engine inspired by Voron — PRD](../krummi.prd.en.md) (v1.1)

> English translation of [queries.prd.md](queries.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Build the **queries**, module 17 of the [parent's split](../krummi.prd.en.md#split-into-child-prds) (annotation of
2026-11-10): read the tables of module 7 with filters, ordering, limits, aggregates and, later in the module, with
expressions, joins, grouping, subqueries, set operations and window functions, and modify them with writes by query.
Today an application reads a table by its primary key, by a range of the key or of an index, or with the neighbor
search of 7.6, and combines the results itself. With this module it describes what it wants as a **query tree**,
Krummi chooses how to read it with a **cost-based planner** that uses the statistics of 7.3, and returns the rows
through a **cursor**. The application can also ask for the **execution plan** that Krummi chose, inspect it, change it
and send it back.

The goal of the module is that, when it is closed, Krummi offers most of the queries of standard SQL (design document 8:
the plans «grow later towards most of standard SQL by adding operators»), expressed as a tree and not as text: a text
query language and SQL remain out of scope (annotation of 2026-11-10 to *Out of scope* of the parent). User: «yo
subdividiría este PRD en varios y al final tener la mayoría de las consultas que los sistemas relacionales
proporcionan». The module is too large for a single execution, so this document is its umbrella, as the one of the
[tables](../tables/tables.prd.en.md) is of module 7: it sets the rules common to all queries and splits the module into
child PRDs, each with its own execution and its own closing (see [Split into child PRDs](#split-into-child-prds)). This
document has no execution of its own. The child PRDs add operators without changing the meaning of earlier ones (CS3).

**Why this way**: as with the tables, the technical design was written before this PRD. Design documents 4 (query tree
and planner), 8 (execution plans), 9 (interchange format), 10 (tools for agents), 11 (the contract of the queries, which
reconciles the earlier ones), 13 and 14 (JSON) and 15 (query profile) record, in the user's own words, each decision of
the user: U1–U9 and P1–P20 of document 11 have been decided since 2026-11-10. This PRD takes from them what is
observable; the how stays in those documents, which are the technical starting state of each child PRD.

**Why now**: module 7 is closed (N2 of the tables, annotation of 2026-11-23: 7.1, 7.2, 7.3, 7.5, 7.6 and 7.7), and the
parent builds 17 right after 7.

This PRD inherits everything from the parent; here only what is common to the module is added. What changes or refines
earlier rules is declared in its decisions and annotated in the affected documents (see
[Annotations to other PRDs](#annotations-to-other-prds)).

In this document, a **query** is a document that describes which rows and which values are wanted, without saying how
to read them; a **plan** is a document that says how to read them, with physical operators (for example, a scan of a
range of an index, a filter or a sort); a **statement** is a query or a plan prepared to be executed many times; a
**parameter** is a value that the statement declares with its type and that the application gives on each execution;
a **cursor** is the sequence of rows of an execution, which the application reads in batches. To avoid confusing them
with the error classes (D8 of the parent), the four of CS3 are **document types**.

### Product decisions

| ID | Decision |
|---|---|
| CS1 | **Public surface**: from this module on, applications query tables in Rust, in any transaction, read or write (P3 of design document 11). A query is written as a typed JSON document or with a fluent Rust API that produces the same document (U1 of document 11). The C interface and the C# binding (module 18) use the same documents. Up to 17.6, a query does not change the application's data; from 17.7, writes by query change it only in write transactions. User, on including them: «Sí, tras 17.5 (Recomendado)». Planning a query uses the estimation of 7.3 with its rules: if the statistics of a column are missing, it waits to sample them in its thread (ST3), and if they are stale it asks to renew them in the background; it never answers with a guess (ST2). Each child PRD extends the operations and operands of the documents (a column, a JSON path, an expression, a distance) with minor versions (CS3), without changing what already existed. |
| CS2 | **Split and closing**: the module is split into the child PRDs of the [table](#split-into-child-prds), which are built in that order; this document has no execution of its own. Each common rule is implemented and tested by the first child PRD that needs it, and the following ones keep meeting it and extend it to their operators without changing it: the validation of plans (17.2) follows the phases, the error classes and the precedence that 17.1 fixes for queries, and each later child PRD adds its operators to the plan document, to its validation and to the explanation. Each child PRD leaves Krummi usable: what it adds works from end to end, from the document to the rows. Module 17 is closed when all its child PRDs are. The design approved for «the first version» of the query tree also the prefix, the distance as a threshold and the `k` nearest, and that the public interface accepts plans from the first version of the module (design README); with this split, that first version is 17.1 to 17.4, and the prefix enters in 17.3, the plans in 17.2 and the distance in 17.4. User, on keeping the split: «Mantener el reparto (Recomendado)». Partial and expression indexes enter as 17.6 (user: «Sí, PRD hijo tras 17.5 (Recomendado)»). Module 18 is built when 17 is closed entirely, with the whole query language; user: «Tras todo el módulo 17». |
| CS3 | **Document types and versions** (P1, P6 and P8 of document 11): there are four types: **query**, **plan**, **execution** (a query or a plan with the values of an execution and its limits, for tests and tools) and **explanation** (a plan, the normalized query it came from, if it came from one, and the diagnostics apart). Each document names its type and its version, with semantic versioning per type: a minor version only adds constructs, and a major one may remove them. A single reader, the one of the current version, accepts a document of any earlier version; one that uses a removed construct fails with invalid document (CS10), naming the construct and the version that removed it, and one of a version later than the reader's fails with the class that 17.1 fixes. **No construct ever changes meaning**: a new meaning takes a new name. User: «en vez de cambiar el significado de like por ejemplo, podríamos añadir cs_like como case sensitive like». Before version 1.0 of the product there is no promise, as D7 for the stores. The queries saved by the application are a public contract; saved plans have no promise beyond a rejection with an error of their class; statements and compiled plans are never saved (U4). An explanation cannot be prepared; its plan can. Each type has a JSON Schema published with the library, but Krummi's validator is the authority (U6). |
| CS4 | **Structure and values apart** (P15, U5): a query or plan document carries no values, only parameters declared with their type; the numbers of `LIMIT`, `OFFSET` and `k` are parameters too. The fluent API turns the constants it receives into parameters. Each type of value has a single form that Krummi writes and a fixed set of forms that it accepts; Krummi rewrites a document it wrote byte for byte, and reading it returns the same typed values (§2.4 of document 11). A null is a valid value of any declared type, except as an element of an `IN` list. |
| CS5 | **Statements** (P2, P3, P4, P12, P14, P16 and P17 of document 11): preparing, binding the values and executing is the path of every query, in Rust in this module and in C and C# in module 18. A statement is prepared in a transaction against the catalog that transaction sees at that moment, belongs to the store and is executed in any transaction, also from several threads at once; each set of values and each cursor is used by one thread at a time, and an execution inside a write runs in the thread of that write (T6). Executing requires all parameters to have a value. A statement fixes the identities of the tables and columns it bound (the random identities of 7.1 and 7.2, P12 of document 11, which the documents write as 16 lowercase hexadecimal digits); a query document may carry them too (Krummi writes them; when reading they are optional), and then it fails with different schema if a name resolves to another identity, while without them it is bound by name when prepared. If the transaction that executes sees another catalog than the statement's, a query resolves its names again and requires the same identities, and a plan is validated again, as the [invalidation table](#invalidation-of-statements) says. Validity depends only on what the executing transaction sees, never on how the one that prepared ended. A query is replanned by itself when the catalog changes, the statistics are rebuilt or the values ask for another plan; a plan is never replanned: whoever wants to fix a plan prepares the one of the explanation (CS6). The starting policy for replanning is the adaptive one of document 4 (P4 of document 11). The only exception is 17.13, which makes a query return by itself to its last good plan, with the explicit limits that the user approved in document 15 («Automático, límites explícitos»). A cursor keeps its plan and a copy of the values it was executed with: changing the values, replanning or closing the statement does not affect it. A cursor does not survive a change of its own write transaction: in Rust, the code that tries it does not compile. Closing the store with live statements fails with misuse and the store stays open. |
| CS6 | **Explicit plans** (U3, P5 of document 11): the explanation of a statement, with values or without them (without them, the plan that Krummi would choose without knowing the values, with the estimates that 17.2 fixes), returns the plan that Krummi would execute, which the application can save, change and prepare as a statement. A plan carries only what changes what it does; estimates and metrics go only in the diagnostics of the explanation, and the limits of an execution are arguments of the execution, never members of the plan. Every plan sent passes the full validation, with no trust shortcuts: a malformed or incoherent plan fails with an error of its class, never ends in a panic nor gives an incorrect result silently. A plan that Krummi writes for a transaction passes its validation unchanged in it and in every transaction that sees the identities it fixes (the table, its schema state mark, the columns and the indexes it uses; not the mark of the indexes, so a new index does not invalidate it), and executed with the same values it returns an admissible result (CS7) for the statement it came from; in the others it fails with different schema. Krummi guarantees that a plan written by the application is executed safely with its own meaning; it does not guarantee that it expresses what the application wanted. |
| CS7 | **Common semantics**: a query reads the snapshot of its transaction (G3). The logic of nulls is the three-valued logic of SQL: a comparison with a null is unknown, and a filter lets through only what is true. Without a requested ordering, the order of the rows is unspecified. There are no implicit type conversions; the comparisons between different numeric types that are admitted are fixed by 17.1. Text is compared with the collation of its column (IC1–IC3 of the tables). A query returns an **admissible result** under any valid plan of that same statement, with the rule of document 10: R being the complete result without `LIMIT` or `OFFSET`, in the order of its ordering, without them the result is R as a multiset, in the requested order if one was requested; with them and an ordering, it has the size of the window of R, the same sequence of values of the ordering keys as the window and is a sub-multiset of R; with them and without ordering, it has the size of the window and is a sub-multiset of R. Grouping, removing duplicates, set operations and join keys compare values like the keys of N5 of the tables: two nulls are equal to each other, `-0` is `+0`, two NaN are equal and `ci` or `ci_ai` text is compared by its folded form; which value of the group is returned is fixed by the child PRD of each operation. There are two exceptions: an approximate path that the query expressly allows (17.4) returns what its child PRD fixes, and a CS8 budget that counts work (rows examined, time) may run out with one plan and not with another. |
| CS8 | **Limits** (P9 of document 11): each execution has budgets of memory (64 MiB by default), of spilling to disk (1 GiB by default), of rows examined and of time (no limit by default), and can be cancelled. Documents have budgets of size, depth, nodes and parameters; planning has one of work, after which the planner keeps the best plan found, which is still correct, and the explanation says so; values, the one of the elements of an `IN` list (10,000 by default); plans, the one of their ranges (10,000 by default); the store's plan cache, the one of its entries (256 by default), which when full discards the least used; `k` does not exceed 10,000 and the nesting of a document does not exceed 128, which are ceilings and not budgets (design README, §2.5 of document 11). Budgets are configured per store and per call. Those that protect memory or size (those of documents, planning, the cache, `IN` lists, ranges, memory and spilling) also have a **hard maximum**: asking for more fails with an error that names the budget and its maximum, it is never silently clipped nor does it admit «no limit». Krummi does not reserve memory in proportion to a size it has not checked. There is no limit for the whole store: the memory of simultaneous executions is the sum of their budgets, and the application, which decides how many it launches at once, owns it. An operator that can spill to disk spills before failing for memory; the others fail when they run out of it. An execution that exhausts its budget fails alone; the others continue. |
| CS9 | **Temporary structures** (design README, *Plans*): the temporary structures of an execution live outside the journal and are not durable. What spills to disk goes unencrypted to a reserved name in the store folder, which opening deletes, like `krummi.sort` (P3 of the Pager), and which the file system encryption covers when there is one (annotation of 2026-11-09 to the parent); 17.1 fixes whether it is the same name. It is deleted when the execution ends, fails, is cancelled or its cursor is dropped without reading it entirely. A failure when spilling (disk full, I/O failure) makes only that execution fail, with its class (G7, G12), and does not prevent the store's writes, because it is not part of it; nor does it count towards its maximum size. |
| CS10 | **Errors** (D8 of the parent, P10 of document 11): a new class, **invalid document**, for a document that is not well-formed JSON or does not meet the grammar, the members, the types or the invariants of its document type and its version, or uses a removed construct; its error carries the path of the failure inside the document (a JSON Pointer, RFC 6901), what was expected and what was found, or the line and the column if it is not well-formed JSON. A document or a statement whose identities, names or state marks do not match the catalog that the transaction sees fails with **different schema** (N10 of the tables), naming the object and the two identities. A bound value that does not meet the declaration of its parameter fails with **invalid value** (N10 of the tables). Execution errors carry the path of the operator that failed. The error is one same object in Rust, in C and in the tools. The classes of a document of a later version, of a parameter without a value, of an exceeded budget and of cancellation are fixed by 17.1. The validation of documents and plans never ends in a panic; the reading of malformed rows keeps the exception to G6 of the parent (annotations of 2026-11-11 and 2026-11-18). |
| CS11 | **Dependencies** (P11 of document 11, under the rule of the annotation of 2026-11-10 to D12 of the parent): only the queries crate depends on `serde`, `serde_derive` and `serde_json`, and on the crates these bring. They have unsafe code, but they are widely used and maintained, Rust only, and the safe alternative, a reader of the documents of our own, would be new code at a security boundary; the user preferred it this way. It differs from what 7.5 decided for the text stored in JSON columns, which is read by a reader of our own (JS14): here the query documents are read, not the data. The engine crates do not depend on them except in their tests. An automatic check compares the tree of normal dependencies of the queries crate with the list that 17.1 fixes in the annotation to D12 and fails if another crate appears. No engine or queries crate depends on an MCP server, an async runtime or an AI SDK (U8). |
| CS12 | **AI-assisted development** (P13 of document 11): the library offers what the tools and agents that developers use need, without depending on them: the JSON Schema of each document type; the description of the store's schema with its identities, state marks and schema versions (P12 of document 11); validating a document without executing it and returning its normalized form; the explanation; execution bounded to a number of rows with a mark that it was cut off; comparing, in the same snapshot, the result of a candidate statement with that of a reference query, with the rule of CS7 and one of three verdicts: admissible, not admissible or engine defect (when the result of the query itself is not admissible); the error object of CS10, and a canonical output for comparing (C1–C8 of document 10). The command line and the skill of document 10 are adapters over those functions and are built with the command-line tool (module 12), not in this module, although document 10 placed its phase 1 «with module 17». User: «En el módulo 12 (Recomendado)». The MCP server, which document 10 conditions on its necessity being shown (phase 2), is out of this module and will be decided separately. |
| CS13 | **Query profile** (requirements M1–M7 of design document 15): the explanation with execution reports, per operator, the actual rows with a completion mark, false when an upper operator stopped it early (M1); each execution counts its logical pages and its CPU time (M3); each index counts its seeks, scans and rows read, and each table its inserts, updates, deletes and bulk-loaded rows, with the entries written in each index (M4); and an execution can deliver its record, with the identity of its transaction, its sequence number within it and the values of its parameters, to a receiver that the application provides; without a receiver nothing is built (M6). The shape of a query and the bytes of a plan hold in the copies of the store (M5, already met by P12 of document 11). The planner receives as inputs the indexes, the directives and the accepted plans (M2), and the catalog reserves a place for directives and accepted plans (M7); both reservations are checked by the design review of 17.1, not by a test. The rest of stage 1 of document 15 is built by 17.13, at the end of the module; sampling, the tuner and the directives stay out of scope. User: «PRD hijo al final del 17 (Recomendado)». |
| CS14 | **Measurement**: Voron has no queries, so the threshold of K7 does not apply to them. Each child PRD measures before and after its change with the rule of N11 of the tables and its annotation of 2026-11-20, on Windows and on Linux. From 17.1, the comparative benchmark has **query overhead** loads, each one a prepared statement that does the same as a direct operation of the tables (read a row by its primary key, scan a range of the primary key, scan a range of an index), with the threshold that the statement performs at least 0.80 times the direct operation; a missed threshold follows K13 of the benchmark: it does not prevent closing the child PRD and becomes a separate change. In addition, the closing report of each child PRD compares its queries with the same queries in SQLite, on the same machine and without a threshold, like 7.6 with PostgreSQL (VI15). SQLite enters only in the benchmark, a maintainer tool whose dependencies do not reach the library (K14). User: «API directa + SQLite (Recomendado)». |

### Invalidation of statements

What happens when executing a statement in a transaction that sees a catalog different from the statement's (§2.2 of
document 11):

| What the executing transaction sees | Query | Plan |
|---|---|---|
| A renamed column | Fails with different schema: the name no longer resolves | Fails with different schema |
| A column that the statement uses, removed | Fails with different schema: the name no longer resolves | Fails with different schema |
| A column removed and another added with its name | Fails with different schema: same name, another identity | Fails with different schema |
| Another schema change of N3 of the tables | It is bound and planned again | Fails with different schema, with the table, the two schema versions and the two marks |
| The table renamed or dropped | Fails with different schema: the name no longer resolves | Fails with different schema |
| The table dropped and created with the same name | Fails with different schema | Fails with different schema |
| An index created | It is planned again | It is executed without using it |
| An index that the plan uses, dropped or created again | It is planned again | Fails with different schema |
| Statistics rebuilt | It is planned again in the next execution | It is executed the same; only the estimates of the explanation change |
| A non-durable commit lost in a crash (G11) and then other changes | Fails with different schema | Fails with different schema |
| A restored or compacted copy (modules 10 and 11, when they exist) | Same as in the original until one of the two changes | Same |

Emptying the table or changing its rows does not change the catalog: the statement is executed the same.

Inside a write, preparing and explaining see their own uncommitted changes, and each schema or index change of that
write counts as a new catalog. A document bound to a committed state that the write did not change remains valid in it,
after committing it and after aborting it; one bound to a state that only that write created is valid in it while that
state lasts, in the others only after committing it, and fails with different schema in all of them after aborting it.
A plan that uses an index fails in every transaction that no longer sees the index, and is executed in one that still
sees the earlier state.

*Annotations of 2026-11-28 from the single-table queries ([queries-single-table.prd.md](queries-single-table.prd.md), 17.1; Spanish
authoritative):*

- *CS3 and CS12: there is a fifth document type, `store/1`, the description of the store, which only Krummi writes, and
  a function that rewrites a document in Krummi's form without normalizing it (CU20).*
- *CS8: the depth of nodes and expressions is a budget apart from the JSON nesting ceiling of 128 (Limits of 17.1).*
- *CS3, CS8, CS9, CS10 and the Fixes cell of 17.1: what they left to 17.1 is fixed in 17.1; a table that does not exist
  fails with not found, not with different schema, when the document carries no `bind` (CU13); the forms of `json`
  values move to 17.3 and 17.4.*

### Starting state

Verified on 2026-11-28 in the code, in the PRDs of the ancestry and in the design documents. *Annotation of
2026-11-28: this section was lost when draft 0.2 was rewritten and is restored without changing any rule.*

| Aspect | Situation |
|---|---|
| Queries | They do not exist: there is no query tree, planner, plans, statements or query cursors. |
| Table reads | Reading a row by its primary key; scanning a primary key range forward and backward; scanning an index range, returning the rows or only their entries; reading only some columns of a row with a projection (7.1, 7.2, projections, row reads). No read filters. |
| Statistics | Public estimate of an equality (also with null), a range, or an equality over a prefix of an index's columns; statistics per column and per index prefix; analyze on request (7.3). There is no estimate of `IN`, of text prefixes or of disjunctions. |
| Vectors | Exact and approximate search for the `k` nearest neighbours, with a filter of equalities, ranges, `IN` and `NOT IN` joined by AND (7.6). |
| JSON | Navigation by members and positions and typed reading of a property (7.5); there are no paths as an expression and no conditions over the elements of an array. |
| Text | Collations `binary`, `ci` and `ci_ai` per column, with their folded form (7.7); there is no text prefix search and no `LIKE`. |
| Catalog | Tables, columns and indexes have random 64-bit identities, and each table its schema and index state tokens, visible through the table description (P12 of document 11, 7.1, 7.2). |
| Format | The current format version is `16`. |
| Tool | The command-line tool does not exist (module 12). |
| Design | Design documents 4, 8, 9, 10, 11 and 15; document 11 replaces the binary format of documents 4 and 8 with typed JSON and fixes the contract. The operators for joins, grouping, `DISTINCT`, set operations, windows and spools are reserved by name in document 8 and have no design. |

## Actors

- **Integrating developer**: writes queries with the fluent API or as JSON documents, prepares them, executes them with
  their values and reads their rows through a cursor; asks for the explanation of a statement and, if they want to fix a
  plan, prepares it as a plan.
- **Agent or tool**: generates documents with the published JSON Schemas, validates them, reads the errors by their
  path and corrects the document (CS12).
- **Maintainer**: runs on Windows and on Linux the differential tests, the fuzzing and the measurements of CS14.

## Flow

1. The application builds a query with the fluent API or as a JSON document (CS1, CS3, CS4).
2. Inside a transaction it prepares it: Krummi validates it, binds it to the catalog that the transaction sees and plans
   it (CS5, CS10).
3. In any transaction, the application gives the values of its parameters and executes it; it receives a cursor and
   reads its rows in batches (CS5, CS7, CS8).
4. If it wants to see or fix the plan, it asks for the explanation, inspects it or changes it, and prepares its plan as
   a statement (CS6).
5. When the executing transaction sees another catalog, the statement is bound again or fails with different schema
   ([invalidation](#invalidation-of-statements)).

## Split into child PRDs

Each submodule is a child PRD of this one, with its own execution. The order is the building order: each one relies on
the earlier ones. The *Fixes* column names what that child PRD must decide, in addition to its contents and its own
errors.

| # | Child PRD | Contents | Fixes |
|---|---|---|---|
| 17.1 | **Single-table queries** | The query and execution documents (CS3, CS4); the fluent Rust API; preparing, binding, executing and the cursor (CS5); filters with `=`, `<`, `≤`, `>`, `≥`, between, `IN`, is null and `AND`, `OR`, `NOT`; projection; `ORDER BY`, `LIMIT` and `OFFSET`; `count(*)`, `count` of a column, `min`, `max`, `sum` and `avg` over all the filtered rows; the cost-based planner, which reads by the primary key, by the indexes (also only their entries, without reading the row) or scanning the whole table, and can combine several of those paths; sorting with spilling to disk (CS9); the limits (CS8); the errors (CS10); the dependencies (CS11); the store description and validating without executing (CS12); the reservations of CS13. | **Documents**: the members and the names of the operations of the query and the execution, from §2.1 of document 11, fixed with reference documents; the written and accepted forms of each value type, from §2.4 of document 11, also those of `json`, which §2.4 does not have; the class of a bound value larger than the maximum of its column; the normalization of a query (for example, `IN` lists sorted and without duplicates) and whether the fluent API writes the normalized form; whether the JSON Schemas are also distributed as files (question 4 of document 11). **Semantics**: which comparisons between different numeric types are admitted; whether comparisons, `IN` and `ORDER BY` over JSON and vector columns are admitted; where nulls go in `ORDER BY` by default; how aggregates treat nulls, the result type of `sum` and `avg` of each type and what happens on overflow; what aggregates return over zero rows; which types admit `min` and `max`; whether aggregates can be combined with non-aggregated columns, with `ORDER BY` or with `LIMIT`; the names and the order of the result columns, and whether one can order by a column that is not projected; the type of `LIMIT` and `OFFSET` and what a null, a negative value or a very large one do in them. **Statements**: what happens with a table or column name that does not exist; whether a statement of one store can be executed in a transaction of another; whether closing the store also waits for the sets of values and the cursors; whether queries use the stored projections (*Out of scope* of 7.1); which budget, time or cancellation covers the wait for an ST3 sampling when preparing, explaining or executing, and with which classes it fails; what a query inside a write that changed rows before creating the cursor sees. **Cursor**: the size of the batches; whether the execution starts when the cursor is created or when it is read, and therefore when the execution errors appear; what it returns after the end, an error or a cancellation. **Limits**: the default values that CS8 does not fix and the hard maximums of each budget; what the rows examined count, whether time is wall-clock, whether it counts between batch reads and how often it is checked; how it is cancelled and from which thread; where the temporary mode spills (G9), which creates no files; what opening does with the spill files that a crash left; whether what was spilled is checked when read back (G6). **Errors**: the classes that CS10 leaves to it, the data and the precedence of each error, and which document the path of a query execution error points to. **Dependencies**: the list of crates of the annotation to D12 (CS11). **Tests**: the number of cases and the seed of AC-CS1, the fuzzing time of AC-CS3 and the hook that simulates versions of AC-CS7. **Measurement**: the data of its overhead loads (number of rows, columns, key distribution), their numbers and names and their place in the report and in the reduced run of the benchmark (K4, K11); which queries it compares with SQLite and how SQLite is configured (CS14). |
| 17.2 | **Plans and explanation** | The plan document and preparing a plan as a statement (CS6); the explanation document, with the estimates, their confidence, the cost and whether planning ran out of budget, and, after executing within the limits of the execution, the actual rows, batches, time, memory and spilling per operator; the explanation as text for people, output only; the bounded execution and the comparison of CS12; the counters and the receiver of CS13. | The members and operators of the plan document, from document 8 and §2.1 of 11; which validations each operator has and their error data; the shape of the text explanation; how the application reads the counters of M4, whether they survive reopening the store and whether they can be reset to zero; where the receiver is registered (per store, per statement or per execution), whether it is called in the execution's thread and what happens if it fails or is slow; which operators can report a false completion; the shape of the record that the receiver gets; the effect of the M4 counters on the loads of the tables against Voron (K7), which it measures with the rule of N11. |
| 17.3 | **Conditions on text and JSON** | Text prefix and `LIKE` with the collation of the column (IC3 of the tables); paths inside a JSON as lists of steps, each one a property name or an array position, with no text to parse, where a step that expects an array or an object and finds something else gives «not there»; their existence, their equality and their typed reading, also as date and instant, in filters and projections; «any» and «all» over the elements of an array with a condition on their properties, met by one same element, where «all» is true on an empty array and an element whose condition is unknown does not count for «any» and makes «all» unknown unless another element makes it false; the multi-entry index over several properties of one same element (design document 14); a benchmark load that filters by a path, with which it is decided whether to read a property without scanning the document text (*Out of scope* of 7.5). Requirements approved in the tables (annotation of 2026-11-12, «sí»). | The names of the text operations with and without case distinction (CS3); what `LIKE` does with escape characters; whether JSON equality compares by meaning or by text; whether JSON paths have statistics and what the planner estimates without them; what the multi-entry index does with an entry that is too long or with a value of another type in the path, and whether it is maintained in the same write (questions 1, 2 and 5 of document 14); the data of its benchmark load. |
| 17.4 | **Vectors in queries** | The distance as an operand: distance threshold, ordering by distance, projecting the distance and the `k` nearest inside a query with any filter of 17.1 and 17.3, with the exact or approximate search of 7.6 as a planner path. Distances are bit-identical on every platform, so a threshold returns the same rows with any plan, and ties among the `k` nearest are broken by the primary key (design README). | When the planner may use the approximate index, how the query asks for or allows it and what result is required of it (CS7), also when the search of VI6 turns approximate by itself; which part of the filter is applied before searching (the conditions of VI7) and which after; what a change of state of the approximate index does to a statement; how the explanation reports it. |
| 17.5 | **Expressions and scalar functions** | Arithmetic, `CASE`, `COALESCE`, explicit type conversions and functions on text, numbers and dates, in filters, projections and orderings; comparisons between columns. | The list of functions and their errors (division by zero, overflow, impossible conversion); whether an expression can use an index. |
| 17.6 | **Partial and expression indexes** | Indexes of 7.2 with a condition of 17.1 or 17.3 (they hold only the rows that meet it) and with columns that are expressions of 17.5 or paths of a JSON of 17.3, maintained on each write; the planner uses a partial index only when the condition of the query implies the one of the index, and one over an expression when the query uses the same expression. | Which other kinds of JSON index of design document 14 enter (for example, inverted ones), if any; which conditions and expressions an index admits (for example, only deterministic ones); how they are declared in the index definition that 7.2 reserved (N7 of the tables); what uniqueness of a unique partial index does; with which class an invalid definition fails; whether they have statistics of their own. |
| 17.7 | **Writes by query** | `UPDATE` with a filter and with `SET` of expressions of 17.5, `DELETE` with a filter and `INSERT` from a query, only in write transactions, with the rules of the writes of 7.1 and 7.2 (validation of each value, indexes up to date, duplicate key); each row is changed only once, even if the change moves it within the path by which it is scanned; a statement that fails with a public validation error (B7 of the trees: invalid value, duplicate key, invalid size) leaves none of its changes and the transaction can commit the others; a failure that T11 of the transactions makes fatal for the transaction (disk full, maximum size, memory exhausted, damage, I/O) remains so, and T12 still holds for the store. | Whether the statement's own failures of a write statement (a CS8 budget exceeded, a cancellation) leave the transaction committable without the statement's changes; what it reports on finishing (for example, how many rows it changed) and whether it can return the changed rows; whether there is an `INSERT` that updates when the key already exists; whether an `UPDATE` can change the primary key; its plan operators and its explanation (CS2). Its changes count towards the statistics threshold like the inserts, updates and deletes of 7.1 (ST4). |
| 17.8 | **Grouping** | `GROUP BY`, `HAVING`, `DISTINCT` and the aggregates per group, with spilling to disk (CS9). | The aggregates that are added (for example `count(distinct)`), the order of the groups without `ORDER BY`, which value a group of `ci` or `ci_ai` text returns (CS7) and the planning budget. |
| 17.9 | **Joins** | Inner, left, right and full outer, semi and anti join between tables, with nested loop by key, hash and merge join; the order of the joins chosen by cost. | The budget for enumerating join orders (§2.5 of document 11); how the columns of several tables are named in a document. |
| 17.10 | **Subqueries and CTEs** | Scalar subqueries, `EXISTS`, `IN` and `NOT IN` with a subquery, and correlated subqueries; `WITH`, including recursive. | Which subqueries are rewritten as joins; the recursion limit. |
| 17.11 | **Set operations** | `UNION`, `UNION ALL`, `INTERSECT` and `EXCEPT`. | The names and types of the result columns when the two inputs differ. |
| 17.12 | **Window functions** | `ROW_NUMBER`, `RANK`, `DENSE_RANK`, `LAG`, `LEAD` and the aggregates with `OVER`, with partitions, ordering and frames. | The frames admitted and the list of functions. |
| 17.13 | **Query profile** | Stage 1 of design document 15: the exact aggregates of the profile per query shape, stored apart from the store's data, and forcing the last good plan of a shape when its plan changes and performs worse. | The nine open product questions of document 15 that touch stage 1; where the profile is stored (for example, a reserved name in the store folder, P3 of the Pager) and what happens to it when the store is copied, compacted or deleted; how a regression is detected and how the forced plan is undone. |

## Validations and messages

Those of the ancestors hold. This module adds these situations, of the library only:

| Situation | Error class |
|---|---|
| A document that is not well-formed JSON, or does not meet the grammar, the members, the types or the invariants of its type and version, or uses a removed construct | Invalid document (new), with path, what was expected and what was found (CS10) |
| A document of a version later than the reader's | The one that 17.1 fixes (CS3) |
| A document or a statement whose identities, names or state marks do not match the catalog that the transaction sees | Different schema (N10 of the tables), with the object and the two identities |
| A bound value that does not meet the declared type of its parameter | Invalid value (N10 of the tables) |
| Executing with a parameter without a value | The one that 17.1 fixes (CS5) |
| A budget requested above its hard maximum | The one that 17.1 fixes, with the budget and its maximum (CS8) |
| An execution that exceeds its memory, its spilling, its rows examined or its time, or that is cancelled | The ones that 17.1 fixes (CS8) |
| Closing the store with live statements | Misuse (CS5) |

Each child PRD adds the situations of its operations and fixes the data of each error.

## Output

The module writes nothing to the screen. A cursor returns the rows in batches. The values read from rows and from index
entries follow the reading rules of the tables (D11 and its annotations of the parent: no copy for those of `string`,
`bytes`, vector and JSON of a row, with the exceptions of JSON strings with escapes and of the keys of index entries).
The values that the execution computes (aggregates, expressions, distances) and those that went through a temporary
structure spilled to disk (CS9) are returned in a copy owned by the application. The documents that Krummi writes
(explanations, plans and normalized queries) have a single deterministic form: the same document is always written with
the same bytes (CS4). The exact outputs are fixed by each child PRD with its reference documents.

## Reuse

- The reads of the tables as access paths: by primary key, by ranges of the key and of the indexes, the index entries
  and the projections (7.1, 7.2).
- The statistics estimation for the planner (N8 of the tables, 7.3), as 7.3 fixed («Planificador de las consultas
  (módulo 17): usará la misma estimación»).
- The neighbor search and its filter (7.6), the navigation and typed reading of JSON (7.5) and the folded form of text
  (7.7).
- The random identities and the state marks of the catalog, and the description of the tables (P12, 7.1, 7.2).

## Out of scope

- A text query language and SQL, a network server and a remote protocol (annotation of 2026-11-10 to the parent).
- The C interface and the C# binding (module 18), except that their documents are those of this module.
- The command line, the skill and the MCP server of document 10: module 12 (CS12).
- Execution sampling, the tuner and the estimation directives of design document 15; creating indexes automatically.
- The MCP server of document 10 (CS12).
- Views, materialized views, triggers, stored procedures and foreign keys.
- Keyset pagination, planner hints and forcing a path in any way other than preparing a plan, except the last good plan
  of 17.13; regular expressions.
- Application-defined functions and time-zone functions (the tables do not store time zones).
- Cursors that go backwards or reposition.
- Inserting rows given in the document itself (the application inserts them with 7.1), multi-statement scripts and
  savepoints.
- Cancelling or limiting the time of preparing and of explaining, except what 17.1 fixes about the wait for a sampling.
- A translator from or to Substrait (design document 8).
- Saving statements or the plan cache (U4).
- Executing a query in several threads at once (design README: «one thread per query in the first version»).
- Full-text search and text indexes (RavenDB features, out of scope of the parent).
- Adjusting the estimates with what the queries return, like the *CE feedback* of SQL Server, which 7.3 left to
  module 17: it stays with the estimation directives of document 15, outside this module.
- Queries that span several stores (out of scope of the parent).

## Left to the implementation

- The cost model, the planner algorithm, when some values ask for another plan (with the policy of document 4 as the
  starting point, CS5), the organization of the plan cache except its budget (CS8), and the internal representation of
  plans, as long as they meet this PRD and those of its children. Design documents 4, 8 and 11 fix their starting point.
- The names of the public Rust operations, of their types and of the new error classes, which follow Rust conventions
  and are in English (D8 of the parent). The names inside the JSON documents are a public contract and are fixed by each
  child PRD (CS3).

## Acceptance criteria

Those of the ancestors hold. Each criterion is checked by the child PRD named and repeated by the following ones with
the operators they add.

1. **AC-CS1** — Given randomly generated queries, with the seed and the number of cases that 17.1 fixes, over randomly
   generated schemas and rows, when they are executed with the plan that the planner chooses and with each alternative
   it considered, which a test hook exposes, then each result is admissible (CS7) against that of a reference plan that
   scans the whole table, filters, sorts and projects or aggregates, over the same snapshot; a generated query that
   needs an exact result adds the primary key to its ordering keys (17.1 and each child PRD with its operators; 17.4
   with the rule of its approximate path).
2. **AC-CS2** — Given a document of each type that Krummi wrote, when it is read and written again, then the bytes and
   the typed values are identical; when an explanation is prepared, then it fails with invalid document; when its plan
   is prepared in the same transaction and executed with the same values, then it returns an admissible result for the
   statement it came from; and when a query is built with the fluent API and the same one is written as JSON, then the
   two documents are byte-for-byte identical (CS1, CS4, CS6) (17.1 for queries and executions, 17.2 for plans and
   explanations).
3. **AC-CS3** — Given mutated documents and plans (without members, with unknown or repeated members, with swapped
   operators, removed ranges or changed types), and the validation subjected to fuzzing for the time that 17.1 fixes,
   when they are prepared and executed, then none ends in a panic; each one that fails does so with a class of CS8, of
   CS10 or of the ancestors (damage, I/O, disk full), and the others run to the end of the cursor (CS6, CS10) (17.1,
   17.2 and each child PRD).
4. **AC-CS4** — Given a prepared statement, when the transaction that executes it sees each case of the
   [invalidation table](#invalidation-of-statements), then the query and the plan do what the table says, and a new
   planning is seen in the explanation of the next execution; the lost-commit case is tested with a simulated crash,
   and that of the copies, when modules 10 and 11 exist, in them; and when a write prepares a statement and then
   changes the schema, commits or aborts, then each transaction accepts or rejects it as the paragraph after the table
   says, including a read opened before the commit, which keeps executing the plan (CS5) (17.1 for queries, 17.2 for
   plans).
5. **AC-CS5** — Given a statement, when 16 threads execute it at once with their own values and cursors, in different
   read transactions, then each cursor returns an admissible result for its snapshot; when the values are changed after
   executing, then the open cursor does not change; when one of the executions exhausts its memory, then it fails alone
   and the others finish; and when it is executed with a parameter without a value, then it fails with the class that
   17.1 fixes (CS5, CS8) (17.1).
6. **AC-CS6** — Given each CS8 budget that makes things fail (those of documents, `IN` lists, ranges, the memory of an
   operator that does not spill, spilling and rows examined), when a document or an execution reaches its limit and
   when it exceeds it by one, then the first works and the second fails with the class that 17.1 fixes, with no test
   memory allocator that counts reservations recording one proportional to what was requested before the check; when
   the plan cache is full and another statement is prepared, then the least used entry is discarded and the next two
   statements keep giving admissible results; when the time limit of an execution has already passed or it is
   cancelled, then its cursor ends with the class that 17.1 fixes; when a budget above its hard maximum is requested,
   then it fails naming the budget and its maximum; and when an execution that spilled to disk ends, fails, is
   cancelled or its cursor is dropped half-read, then its files do not remain in the store folder (CS8, CS9) (17.1).
7. **AC-CS7** — Given invalid documents, at least one for each cause of CS10 (not JSON, a repeated member, an unknown
   one, an unknown operation, a type that does not match, a value in a form not accepted, a removed construct), when
   they are prepared, then they fail with invalid document with the path, what was expected and what was found that
   17.1 fixes; an execution error carries the path of the operator that failed; and, with the hook that simulates reader
   versions, a document of an earlier version is accepted with its meaning, one with a removed construct fails naming it
   with the version that removed it, and one of a later version fails with the class that 17.1 fixes (CS3, CS10)
   (17.1).
8. **AC-CS8** — Given rows with nulls in the columns of a query, when they are filtered with each comparison operator,
   `IN`, `AND`, `OR` and `NOT`, then only the rows whose condition is true under three-valued logic pass (CS7) (17.1
   and each child PRD with its operators).
9. **AC-CS9** — Given the queries crate, when it is compiled in continuous integration, then the CS11 check fails if
   its tree of normal dependencies has a crate that is not in the list of the annotation to D12, if an engine crate has
   `serde` among its normal dependencies, or if any engine or queries crate depends on an MCP server, an async runtime
   or an AI SDK (17.1).
10. **AC-CS10** — Given a query cursor, when it returns `string`, `bytes`, vector or JSON values read from rows without
    going through a structure spilled to disk, then they point to engine memory and not to a copy, and using them after
    closing the transaction does not compile (AC20 of the parent); when it returns values read from the keys of an
    index entry, aggregates or distances, then they are copies owned by the application; a cursor of a write that is
    used after a change of that write does not compile; and a query inside a write sees what 17.1 fixes (CS1, CS5,
    *Output*) (17.1).
11. **AC-CS11** — Given a store with live statements, when it is closed, then it fails with misuse and the store stays
    open (CS5) (17.1).
12. **AC-CS12** — Given the example values of each form that 17.1 fixes (the written, accepted and rejected ones of
    each value type), the extremes of the dates and instants of the domain of 7.1, all the `f32` and 10⁸ random `f64`,
    when they are written and read, then each accepted value gives a single typed value, each rejected one fails with
    its class, and each value written by Krummi is read the same and written again with the same bytes (CS4) (17.1).
13. **AC-CS13** — Given a saved query with the identities that Krummi wrote, when its column is removed and another is
    added with the same name and it is prepared, then it fails with different schema; and the same query without
    identities is bound to the new column (CS5) (17.1).
14. **AC-CS14** — Given the reference documents of each type, when they are validated with their published JSON Schema,
    then all pass; the store description names each table, column and index with its identity and its schema version;
    validating without executing a query returns the same normalized query that its explanation carries; the execution
    bounded to N rows returns N rows and the cut-off mark when there are more, and all of them without the mark when
    there are not; and the comparison gives admissible for an index plan of a query, not admissible for one that removes
    its filter, and engine defect when a test hook alters the result of the query itself (CS12) (17.1 the description
    and the validation, 17.2 the rest).
15. **AC-CS15** — Given a planning with a budget of a single estimation operation, when it is planned and executed, then
    the result is admissible and the explanation says that planning ran out of budget (CS8) (17.2).
16. **AC-CS16** — Given an execution with explanation, when a `LIMIT` stops an operator, then that operator reports its
    actual rows with the completion mark false, and those that finish carry it true; when a test does a known number of
    seeks and scans through an index and of inserts, updates, deletes and bulk-loaded rows in a table, then the M4
    counters give those numbers; and without a receiver no record is built, according to the test hook that 17.2 fixes,
    while with one each execution delivers one with its transaction, its sequence number and its values (CS13) (17.2).
17. **AC-CS17** — Given the overhead loads of CS14, when the full benchmark is run on Windows and on Linux, then the
    report shows for each one the ratio between the statement and its direct operation and whether it reaches 0.80, and
    the command ends with `1` if any does not (K13); and the closing report of each child PRD includes the
    measurements before and after of N11 and the comparison with SQLite (CS14) (17.1 and each child PRD).

## Annotations to other PRDs

This PRD changes or refines rules of earlier documents, which carry the annotations of 2026-11-28 (in their
translations too):

- Root parent, objectives: objective 4 is added, **AI-assisted development**, limited to the agents and tools that
  developers use: the library offers them the schemas of its documents, the store description, the explanation and
  errors with a path, without depending on any MCP server or AI SDK (P13 of design document 11, approved on
  2026-11-10; CS12).
- Root parent, split: the row of module 17 (annotation of 2026-11-10) adds the prepared statements and the documents
  with their schemas (P2, P8), is split into the child PRDs 17.1 to 17.13 of this document and its goal is most of the
  queries of standard SQL (CS2); module 18 is built when 17 is closed entirely; module 12 also builds the command line
  for agents, with `--json`, and the skill of design document 10, over the functions of CS12.
- Root parent, D2: the public surface adds the query, plan, execution and explanation documents and their JSON Schemas
  (CS3).
- Root parent, D5: the benchmark also compares with SQLite, which is not a Rust engine, only as information in the
  closing reports of module 17, and the query overhead loads have a threshold against the direct operations of Krummi
  (CS14).
- Root parent, D7: documents have their own versioning, apart from the store format (CS3).
- Root parent, D8 and *Validations and messages*: new error class invalid document (CS10).
- Root parent, D11 and AC20: the values that a query computes and those that went through a structure spilled to disk
  are returned in a copy owned by the application (*Output*).
- Root parent, D12: under the rule of the annotation of 2026-11-10, `serde`, `serde_derive` and `serde_json` are
  admitted, and the crates they bring according to the list that 17.1 fixes, only in the queries crate, with the reason
  of CS11.
- Root parent, *Out of scope*: the dependencies on MCP servers, async runtimes and AI SDKs stay out of the library
  (CS11).
- Benchmark, K7 and *Output*: the query overhead loads also have a threshold (CS14), against the direct operation of the
  tables and not against Voron; the report counts them with the other thresholds and K13 holds for them. K2 and K14:
  SQLite enters the benchmark only for the comparison without a threshold of the closing reports of module 17.
- Pager, P3: the spilling of queries uses a reserved name in the store folder that opening deletes, `krummi.sort` or
  another that 17.1 fixes (CS9).
- Tables, N10: different schema also covers the documents and statements whose identities do not match the catalog, and
  invalid value also the bound values that do not meet the declaration of their parameter (CS10).
- Tables, N7: the reserved partial indexes are built by 17.6, with the expression indexes. *Out of scope* of 7.5,
  «expression indexes, after 7.2 and module 17»: they enter in 17.6.
- Statistics, *Out of scope* «Adjusting the estimates with what the queries return [...]: module 17»: it stays out of
  module 17, with the estimation directives of design document 15.
- Projections, «the queries of module 17 will fix whether they use them»: 17.1 fixes it.
- Collation, *Out of scope* «A text prefix search operation, `LIKE` or «starts with»: module 17»: 17.3 covers it.
- JSON columns, requirements for module 17 (paths, «any» and «all», multi-entry index, typed reading, benchmark load
  that filters by a path): 17.3 covers them; which other kinds of JSON index enter, 17.6.
- Vector indexes, *Out of scope* «Filtering by a distance threshold»: 17.4 covers it.

## References

- Parent: [krummi.prd.en.md](../krummi.prd.en.md) v1.1 (D2, D5, D7, D8, D11, D12, G3, G6, G7, G9, G11, G12, AC20, split
  and annotations of 2026-11-10)
- Tables: [tables.prd.en.md](../tables/tables.prd.en.md) (N3, N5, N7, N8, N10, N11) and its children, especially
  [tables-schema.prd.en.md](../tables/tables-schema.prd.en.md),
  [tables-indexes.prd.en.md](../tables/tables-indexes.prd.en.md),
  [tables-projection.prd.en.md](../tables/tables-projection.prd.en.md),
  [tables-statistics.prd.en.md](../tables/tables-statistics.prd.en.md),
  [tables-json.prd.en.md](../tables/tables-json.prd.en.md),
  [tables-collation.prd.en.md](../tables/tables-collation.prd.en.md) and
  [tables-vector-indexes.prd.en.md](../tables/tables-vector-indexes.prd.en.md)
- Pager: [pager.prd.en.md](../pager/pager.prd.en.md) (P3); transactions:
  [transactions.prd.en.md](../transactions/transactions.prd.en.md) (T6, T11, T12)
- Benchmark: [benchmark.prd.en.md](../benchmark/benchmark.prd.en.md) (K2, K4, K7, K11, K13, K14)
- Design: [docs/design/tables/README.md](../../../design/tables/README.md) and its documents 4, 8, 9, 10, 11, 13, 14 and
  15

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| 77da4411 | 2026-11-28 09:18 UTC+00:00 | 1 h 31 min | 24 min | 1 h 7 min | 47 min | 20 min | 10 |

<!-- prd-time: [{"session": "77da4411-edaa-49ca-96e6-3afe464b2b23", "start": "2026-11-28T09:18:54.972000+00:00", "wall": 5486, "discarded": 1465, "agent": 2807, "human": 1213, "interventions": 10, "idle_minutes": 10}] -->
