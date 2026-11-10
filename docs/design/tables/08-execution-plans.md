# 08 - Physical execution plans, their execution and the executor's temporary structures

Part of the [tables design set](README.md). Written 2026-11-09. Nothing here is implemented, built or measured: no
code, benchmark or test was run for it, and every cost is an estimate that says so. External facts were fetched from
the sources listed at the end; anything not fetched is marked "not verified".

## Summary

1. A plan is a tree of physical operators in a format of Krummi's own: the wire envelope of [document 4](04-query-tree-and-planner.md)
   (magic, version, tagged length-prefixed nodes, a separate parameter block, canonical bytes) with a second root
   kind, `Plan`. Operator names follow Substrait's vocabulary where the semantics are equal, with a mapping table; Substrait is not the wire format.
2. Why not Substrait: its relations have no index seek, key lookup or spool, it is 0.x with breaking changes, and its Rust
   and C# libraries bring protobuf. A Krummi-specific plan would be all extension relations, so interoperability
   would buy nothing. A later translator from Substrait logical relations to the query tree stays possible.
3. Validation is five phases (structure, binding with the schema version, types, operator invariants and derived
   properties, resource admission) plus run-time limits (memory, spill, rows examined, deadline). A bad plan is a typed error with a node index.
4. Execution stays pull-based batches of up to 256 rows. A batch owns a small arena, so operators that outlive a page
   reference (sort, spool, hash table) copy rows into it without `unsafe`.
5. Temporary structures: plain Rust structures in memory up to a per-query budget; above it, one file-backed scratch
   store per query (trees, no journal, no log), created lazily and released with the cursor. Today's anonymous-memory
   temporary mode is not a spill: it is bounded by RAM plus commit charge.

## Requirements

From the README decisions and documents 1 to 6:

- R1. The public interface accepts plans from the first version of module 17, built by an application or an AI. A bad
  plan is an error, never a panic or a silent wrong result; plans carry resource limits and are checked against the
  schema version.
- R2. First version: primary key range, index seek and range, full scan, filter, fetch by primary key, project, sort,
  top-k by column or vector distance, limit and offset, ungrouped aggregates. Growth, by adding operators: joins,
  group by, unions and set operations, window functions, spools.
- R4. D11: reads are zero-copy while the read transaction is open. D12: no `unsafe` in Krummi's code outside the pager
  and `krummi-capi`, so the plan decoder, validator and executor have none.
- R5. Keys are FoundationDB tuples (index key = `tuple(columns) ‖ tuple(primary key)`, doc 3); rows are clustered by
  primary key (doc 2); estimates come from histograms and boundary descents (doc 3), never exact range counts.
- R6. The executor's own structures (table and index spools, hash tables, temporary indexes, sort runs) are not subject
  to the journal (README).
- R3. The planner emits plans and the executor only runs them; explain shows estimated and, after execution, actual rows per operator.

## Options compared

### 1. Representation of a plan

| System | What a plan is | Index access | Spools and temporary structures | Stability | Lesson |
|---|---|---|---|---|---|
| Substrait | Protobuf plan of relations; "there is no true distinction between logical and physical operations", physical status is a convention. Physical relations listed: Hash Join, Nested Loop Join, Merge Join, Exchange, Merging and Simple Capture, Top-N, Hash and Streaming Aggregate, Consistent Partition Window, Expand, Hashing and Streaming Window. Logical: Read, Filter, Sort, Project, Cross Product, Join, Lateral Join, Set, Fetch (offset, count), Aggregate, Reference, Write, Update, DDL | None in the lists fetched. Read carries an optional filter and projection | None | 0.x: "x increments for breaking changes and new features"; deprecated fields are removed at 1.0.0. The "User Defined Relations" page was "Pending" | Good vocabulary and semantics (Top-N, Fetch, Streaming Aggregate); no physical access paths |
| DataFusion `ExecutionPlan` | Rust trait: `children()`, `properties()` (partitioning, ordering, boundedness), `execute(partition)` returns a stream of `RecordBatch` | By table providers | Memory pool plus disk manager | Rust API, no wire format | Ordering travels with each node; metrics per node |
| DuckDB | Logical tree converted to a `PhysicalOperator` tree; "push-based vectorized model" | Not verified in detail | Spills group by, join, sort and window to a temp directory | Internal | Every blocking operator can spill |
| SQLite VDBE | Register bytecode, 191 opcodes; `EXPLAIN` lists instructions | Cursors | `OpenEphemeral` (transient table or index, deleted when its cursor closes), `OpenAutoindex` | "not an API of SQLite"; changes per release | Wrong shape for an AI or application to author; the ephemeral index is the right idea |
| PostgreSQL | Tree of nodes: Seq, Index, Index Only and Bitmap scans, BitmapAnd/Or, Nested Loop, Hash and Merge Join, Hash, Materialize, Sort, Limit, Append | Explicit | `Materialize`, `Hash`; `Sort` reports memory or disk | Text only | `rows`, `width`, cost per node; `ANALYZE` adds actual rows and loops |
| SQL Server showplan | XML plan of operators. Table Spool, Eager/Lazy Spool, Index Spool (with `SeekPredicates`, "existing only for the lifetime of the query"), Window Spool, Hash Match | Yes (seek, scan, key lookup) | Spools live in tempdb; Hash Match builds a hash table | Versioned XML schema | The closest vocabulary to what the README asks for; estimated and actual rows per operator |

Three candidates:

| | A. Substrait as the wire format | B. Own format, Substrait vocabulary | C. Doc 4 format extended with physical nodes |
|---|---|---|---|
| First version needs | Index scan, key lookup, range lists need extension relations, so the whole plan is extensions | Native | Native |
| Growth (joins, group by, windows, set ops) | Covered by the spec, but each needs Krummi's access paths anyway | Add a tag per operator | Add a tag per operator |
| Size and stability | Large spec, 0.x, breaking changes weekly-release cadence | Ours: about 25 query-tree tags plus about 15 operators | Same as B |
| Rust and C# | `substrait` crate via prost, needs `protoc` at build time; C# libraries found are an experimental `Substrait.Net` 0.1.0 (last push 2024-02 per a third-party listing) and a third-party package that depends on `Google.Protobuf`; setup not verified | A second writer/reader on each side, same pattern as doc 4 | Same as B |
| `unsafe` | Whether prost output contains `unsafe` is not verified; D12 governs Krummi's own code | None | None |
| Validation cost | Full protobuf decode to owned structs, then validation; no canonical bytes for hashing (doc 4) | One forward pass, canonical | Same |
| Names tables, indexes, columns, schema version | Named tables; index and version are extensions | In the format (below) | In the format |
| Reach | Acero, Velox, ADBC, Flight SQL consume Substrait; DataFusion and DuckDB produce and consume it | None outside Krummi | None |

B and C are the same decision. The query tree and the plan share the envelope, the expression nodes (`Cmp`, `And`,
`Or`, `Not`, `In`, `Prefix`, `IsNull`, `Distance`, `Param`) and the validator's type checker. They differ in the root kind, in
relation nodes (physical access paths) and in how columns are named: the query tree names column ids of one table, a plan names
column ordinals of each operator's input, as Substrait does ("field references are always ordinal based on the order of the incoming streams").

**Recommendation: B/C.** A maintained table maps Krummi operators to Substrait relations (`TableScan` and `IndexScan` to
Read, `Limit` to Fetch, `TopK` to Top-N, ungrouped `Aggregate` to Streaming Aggregate, and so on). The name `Fetch` is not used
for "fetch by primary key" because Substrait uses it for offset and count; ours is `KeyLookup`. If interoperability
is wanted later, a translator maps Substrait logical relations to the query tree (not to a plan), so the planner still chooses the access paths.

### 2. Execution model

| Model | Fit | Note |
|---|---|---|
| Volcano, a row per call | Simplest | One virtual call per operator per row |
| Pull batches (doc 4) | Amortized calls, selection vectors, bounded memory | A row-at-a-time loop is a helper on top |
| Push pipelines (DuckDB) | Best for parallel analytic scans | Wrong for one-thread-per-query reads; hard with a borrowed read transaction |
| Async streams (DataFusion `execute` returns a stream) | Parallel partitions, async I/O | Krummi has no async in the first version (doc 6) |

**Keep doc 4's choice**: synchronous pull batches of up to 256 rows. One revision: a batch is not only references to page rows.

## Recommended design

### Operators of the first version

Each operator has an output schema (types, nullability) and derived properties (order, uniqueness of the primary key).

| Operator | Inputs | Meaning | Substrait analog |
|---|---|---|---|
| `TableScan { table, ranges, dir, cols }` | 0 | Clustered tree: a list of disjoint primary key ranges, ascending or descending; a full scan is one open range | Read |
| `IndexScan { index, ranges, dir, cols }` | 0 | Index tree: ranges over `tuple(columns) ‖ tuple(pk)`; emits index columns, included columns and the primary key; a point lookup is a one-key range | Read (extension) |
| `KeyLookup { table, cols }` | 1 | For each row of the input, which carries a primary key, descend the clustered tree; fails the query if the row is missing (a bug, never silent) | none |
| `Filter { predicate }` | 1 | Selection vector; three-valued logic as doc 4's open question 11 decides | Filter |
| `Project { exprs }` | 1 | Columns, `Distance(col, param)`; computed values go to the batch's extra columns | Project |
| `Sort { keys }` | 1 | Full sort, may spill | Sort |
| `TopK { keys, k, offset }` | 1 | Bounded heap of `k + offset` entries; ties broken by primary key (README); keeps keys and locator only, so a `KeyLookup` above it reads the rows | Top-N |
| `Limit { offset, count }` | 1 | Stops pulling | Fetch |
| `Aggregate { aggs }` | 1 | `count`, `min`, `max`, `sum`, `avg` over the whole input, constant state | Streaming Aggregate (no groups) |
| `KeyUnion`, `KeyIntersect` | 2 or more | Inputs are index scans projected to primary keys; output is the keys sorted and distinct (doc 4's index union and intersection); uses the key sorter below | Set |

Output columns, by operator (made explicit 2026-11-10 after an external review, H16, which found a plan whose result
lost the requested columns): `TableScan`, `IndexScan` and `KeyLookup` emit the columns their `cols` name (an
`IndexScan` only among the index, included and primary key columns); `Filter`, `Sort` and `Limit` emit their input's
columns unchanged; `Project` emits what its expressions name; `Aggregate` emits its aggregates. Two kinds emit **keys
and locators only**: `TopK` emits its sort keys and the primary key, and `KeyUnion` and `KeyIntersect` emit primary
keys; any other column must come from a `KeyLookup` above them. The validator's phase 4 checks every column an
operator consumes or the plan returns against what its input emits.

Reserved tags, rejected by a first-version reader with a typed "unknown operator" error: `HashJoin`, `NestedLoopJoin` (inner side an `IndexScan` or
`TableScan` whose ranges refer to outer columns), `MergeJoin`, `HashAggregate`, grouped `StreamAggregate`, `Window*`, `Union`,
`Intersect`, `Except`, `Distinct`, `TableSpool`, `IndexSpool`, `Extension(tag, bytes)`. The spools are reserved because they
only matter once a subtree can be rescanned (the inner side of a nested loop); nothing in a single-table plan rescans.

### Plan format sketch

```text
Plan   { magic, version, flags(kind = Plan), refs, nodes (pre-order, tag + len + payload), param_block }
refs   [ { table_id, table_name, schema_version, indexes: [ { index_id, index_name } ] } ]
Node   tag(1) len(varint) payload       // children follow by the arity of the tag
Expr   Col(ordinal) | Param(i) | Cmp(op) | And | Or | Not | IsNull | In | Prefix | Distance(metric, col, param)
est    optional trailing field of each node: est_rows, est_cost   // ignored by the executor, used by explain
```

- Tables, indexes and columns are named by id and name together. A mismatch between id and name is an error, so a plan built from a stale
  catalog cannot silently bind to a different object.
- `schema_version` is a per-table counter bumped by every schema change (doc 2 allows changes without rewriting rows). Phase 2 requires
  equality; otherwise the error names the table and both versions, and the client rebuilds the plan.
- Values travel in the parameter block as in doc 4, so the bytes before it are the plan's shape key and a plan is a cacheable object.
  Range bounds are parameters; Rust encodes them into tuple bytes (doc 3), so the C# builder needs no key encoder.
- The planner's output is the same bytes. The plan cache stores them; the executor never sees the query tree.

### Validation of an external plan

Phases, in order; the first failure returns `{ code, node index, expected, found }` so that an AI author can correct it:

1. **Structure.** Magic, version, total size at most 1 MiB, depth at most 64, at most 4,096 nodes, known tags, lengths inside the buffer,
   arity matches the tag. Iterative or bounded recursion; fuzzed (doc 4).
2. **Binding.** Every table, index and column exists; id and name agree; the index belongs to the table; `schema_version` equals the current
   one; vector dimensions equal the column's.
3. **Types.** Each operator derives its output schema from its inputs bottom-up. Every expression is typed: comparison operands have
   comparable types, parameter types match the declared ones, a distance takes a vector column and a parameter of the same dimension and element type, `NaN`, infinities
   and zero vectors in cosine are rejected as in doc 5. No implicit conversions in the first version.
4. **Operator invariants and properties.** Examples: `KeyLookup`'s input must carry the table's full primary key; range lists must be sorted and
   disjoint and each bound must be a prefix-compatible tuple of the index's columns; `KeyIntersect` inputs must come from indexes of the same
   table; `Limit` and `TopK` counts are parameters of integer type; `TopK` keys are orderable types (not vectors). The validator derives order and
   uniqueness as a property of each node: an `IndexScan` is ordered by its key, a `Sort` by its keys, a `Filter` keeps its input's order. A future `MergeJoin` or grouped `StreamAggregate` is
   valid only if the derived order of its inputs covers its keys. Properties come from the trees and from `Sort`, so
   the proof is by construction, not by trusting the author. Operators that rely on an order still verify it cheaply while they run
   (adjacent keys compare in order) and fail with `PlanInvariantViolated` rather than return a wrong result.
5. **Resource admission.** Fills missing `est` fields from the statistics (doc 3). The plan is refused if it names more ranges than a limit (default 10,000, like an `IN` list), or
   if a `TopK` has `k` above 10,000 (README). It may also be refused above an optional `max_estimated_cost`, off by default, because the estimates are wrong exactly where nobody optimized the plan.

**Run-time limits are the real guard**, because the estimates may be wrong. A `QueryLimits` struct travels with the call (defaults in brackets, all configurable, doc 4 question 9):

| Limit | Enforced where | Error |
|---|---|---|
| Memory per query [64 MiB] | every structure reserves bytes before growing (below) | spills if the structure can; else `ResourceExhausted(memory)` |
| Spill per query [1 GiB, a proposal] | the scratch store's maximum size | `ResourceExhausted(spill)` |
| Rows examined [none] | `TableScan`, `IndexScan`, `KeyLookup` count decoded rows | `ResourceExhausted(rows)` |
| Deadline [none] | checked once per batch, and every 1,024 rows inside a sort or hash build | `Timeout` |
| Cancellation | same checks, a flag the cursor owner sets | `Cancelled` |

No arithmetic on lengths uses unchecked operations; the decoder and executor have no `unsafe` (R4), so the worst a hostile plan does is
return one of these errors. Whether any plan can panic is a fuzzing and property-test target of the PRD, not a claim made here.

### Execution

- `trait Operator { fn next_batch(&mut self, ctx: &QueryContext, out: &mut Batch) -> Result<bool>; }` with `Batch<'tx>`: up to 256
  entries, a selection vector, and an owned arena. An entry is a `RowRef` that is either `Page(&'tx [u8])` (key and row borrowed from the read
  transaction, valid until the transaction ends, D11) or `Owned(range in the batch's arena)`, plus the *extra columns* (a projected distance, aggregate results) in small typed arrays.
  Column ordinals of a plan address row columns first, then extras.
- A page reference is valid for as long as the transaction. An owned row is valid until the producing operator is called again. Operators
  that must keep rows past the page (Sort, TopK, `KeyUnion`, future spools and hash tables) copy the bytes into memory they own, charged to the
  budget. They emit by copying into the output batch's arena, which costs one more copy per row and needs no lifetime tricks and no `unsafe`.
- `Filter` evaluates predicates on the columns it needs through doc 1's offset access and sets the selection vector; distance predicates run last.
- Late materialization is a plan shape, not a special case: a `TopK` over a vector distance keeps `(distance, primary key)` for at most 10,000 entries
  (a few hundred KiB), and `KeyLookup` reads the 16 KiB rows of the winners at the end. Without it, 10,000 rows of a 4,096-dimension `f32` column would
  be 160 MB in the heap.
- C interface (doc 6): the cursor's `next_batch` returns `krummi_row` entries. Owned rows are copied into a cursor buffer valid for the same period
  as the computed values of doc 4.

### The executor's temporary structures

**Tier 1, memory.** `Vec` arenas, `HashMap`, a binary heap. Each structure holds a `Reservation` from the query's `MemoryBudget`
(single-threaded, a `Cell<usize>`): `try_reserve(bytes)` before it grows, the reservation drops with the structure. DataFusion has the same
idea: a `MemoryPool` that "tracks and potentially limits memory use", `MemoryReservation`s that return bytes to the pool when dropped, and a `FairSpillPool` that splits the room among spillable consumers.
In Krummi a query runs on one thread, so the operator that fails to reserve is the one that spills; a store-wide pool (the sum over concurrent queries) is a later addition and an open question.

**Tier 2, a scratch store per query.** When a reservation fails, the operator moves its data into trees of a *scratch store* owned by the
query's context, created at that moment:

| Structure | Representation in the scratch store |
|---|---|
| Sort runs | Each run is sorted in memory before it spills, then appended in that order to its own fixed-size tree keyed by its position in the run (a `u64`, append path of doc 2), with the encoded sort key and the row as the value; a k-way merge over the run cursors compares the decoded sort keys in memory. Merge fan-in limited, as DataFusion's disk manager limits `max_spill_merge_fan_in`. Corrected 2026-11-10 after the Codex review (R6): the first version keyed the run tree by `sort key ‖ sequence`, which a sort key over a long `string` or `bytes` column (3,000 bytes is valid in a row) cannot fit in the tree's 2,048-byte key; the sort key now lives in the value, which has no such limit (large values go off-leaf), and the run's order needs no key comparison because the run is already sorted |
| Key set (`KeyUnion`, `KeyIntersect`) | A sorted run of primary keys, merged with duplicate removal |
| Table spool (reserved) | One tree keyed by a big-endian sequence number; eager builds in a write transaction, commits, then reads through a read transaction with zero-copy references; lazy appends while reading, copying rows out (a write transaction's reference is valid only until the next mutation, doc 6) |
| Index spool and temporary index (reserved) | A tree keyed `seek columns ‖ sequence`; the seek is the tree's own range scan. When they are designed, seek columns that can exceed the key limit need the same care as the sort runs (R6). This is SQL Server's Index Spool and SQLite's automatic index, built from the trees Krummi already has |
| Hash table (reserved) | Grace partitioning: rows written to N partition trees keyed `hash ‖ sequence`, each partition rebuilt in memory afterward |

The scratch store has no journal, no recovery, and no log of any kind. The trees give ordered runs and seeks for free; the cost is that rows
are encoded and decoded through the tree layer (not measured).

**Why not SQL Server's "worktables in tempdb" as is.** The fetched page lists work tables for spools, cursors and sorts, work files for
hash operations, and intermediate sort results among tempdb's internal objects; the Spool operators describe their hidden tables as living in tempdb "for the lifetime of the query". But
tempdb has a log file (`templog.ldf`); the page says operations are "minimally logged", that tempdb is re-created at every start so it needs no recovery,
that delayed durability is always on and files are not opened with `FILE_FLAG_WRITE_THROUGH`. A log in tempdb is there, as the user explained, mainly for undo, because user temporary tables take part in
transactions and can roll back partially (that purpose is not stated on the page fetched; not verified). **Krummi's executor structures need no log at all
because they are not transactional:** each is created and discarded whole with its query's cursor, never rolled back in part, and nothing survives a crash.
If user-visible temporary tables inside transactions are ever added, they would need undo, and would not be built on the scratch store.

#### Where the scratch store lives

| | Plain heap only | Anonymous temporary store (today's mode) | File-backed scratch store (recommended tier 2) | One shared scratch store |
|---|---|---|---|---|
| Backing | `malloc` | `VirtualAlloc` reserve then commit on Windows (`sys.rs:1152-1170`); `mmap_anonymous` private then protect on Linux | A temp file, mapped; Windows `FILE_ATTRIBUTE_TEMPORARY` plus `FILE_FLAG_DELETE_ON_CLOSE`; Linux created then unlinked | Same file, shared |
| Limit | RAM | `MEM_COMMIT` charges "from the overall size of memory and the paging files"; the commit limit is physical memory plus paging files and an allocation beyond it fails. Krummi reports out-of-memory (P13). Linux overcommit and swap behavior: not verified | Free disk space; the mapping extends the file and `CreateFileMapping` fails with `ERROR_DISK_FULL` if it cannot; a file view "doesn't count toward the limit unless the application asks for copy-on-write" | Same |
| Under memory pressure | Process pages out, or killed | Committed anonymous pages go to the paging file | Dirty file pages are written back and evicted; this is a real spill | Same |
| Short queries | free | no disk I/O expected | Windows: with `FILE_ATTRIBUTE_TEMPORARY` file systems "avoid writing data back to mass storage if sufficient cache memory is available"; Linux: flusher threads write dirty data once older than `dirty_expire_centisecs` or past the dirty ratios, so a mapped temp file can reach the disk even if it is deleted soon after | Same |
| Creation cost | none | one reservation (up to 64 GiB of address space, `TEMPORARY_RESERVATION`) plus the pager's page-remembering tables; not measured | A file through the file-system filters: about 100–200 µs on Windows here (reported by the coordinator from `docs/performance/plan.md`; the plan attributes 34 % of W17 to creating temporary-area files); not measured on Linux | Paid once |
| Cleanup after a crash | process exit | process exit | Windows: deleted after all handles close, which process exit should do (to test); Linux: the name is gone after unlink, "the file will remain in existence until the last file descriptor referring to it is closed" | Same |
| Concurrency | n/a | one writer per store: queries would queue on a shared one | one writer per query store, no contention | Serializes every spilling query's writes; page reuse and fragmentation like tempdb |
| G8 (encryption) | no disk | no disk | writes user data to disk; no longer a risk, see risks | Same |

Growth uses the pager's own method (D11): the file grows by successive mappings and no page moves, and space is reserved on disk so that a
shortage appears at growth, not at a later write (`reserve_space`, P11), which also avoids a `SIGBUS` for a page beyond the end of the file (mmap(2)).
Creation of the file is the only new `unsafe`, inside the pager, which already maps temporary-area files (`create_temp_file`, commit `fafb5bf`).
On Linux the file is created and unlinked immediately; `O_TMPFILE` avoids even that window but "only a subset of Linux filesystems provide that support".
`FILE_FLAG_DELETE_ON_CLOSE` fails if other handles exist unless they were opened with `FILE_SHARE_DELETE`, so the scratch store opens its
file once and keeps that handle.

**Recommendation:** tier 1 in memory; tier 2 a file-backed scratch store, one file per spilling query holding all its trees, created at the first spill and sized by the spill limit.
The public temporary mode is unchanged (anonymous memory, for tests and small stores); the scratch store is an internal constructor of the pager plus a safe `Store::open_scratch(&ScratchOptions)`.
Creating it only on the first spill keeps the estimated (not measured) 100–200 µs off every query that fits its budget; a spilling query is already doing milliseconds of work. If measurements show the creation cost matters, keep a small pool of
pre-created files; that is an optimization to measure, not to build now.

**Lifetime.** The scratch store belongs to the query's `QueryContext`, which the cursor owns. It is released when the cursor is freed, when its read
transaction ends (the results are invalid then anyway), on any error, and on panic (`catch_unwind` at the C boundary drops the context). Operators release their
reservations and trees as they finish; the file goes with the context. A leaked C# cursor is released by its `SafeHandle` finalizer (doc 6).

### Explain

`explain` returns text and a structured form (an array of per-node stats indexed by pre-order node number, through the cursor).

```text
Limit 20                              est 20      act 20      0.1 ms
└─ KeyLookup customers                est 20      act 20      0.2 ms
   └─ TopK k=20 by created desc       est 20      act 20      mem 1.9 KiB
      └─ IndexScan ix_country [ES]    est 840     act 912     fwd, covering
```

Per node: estimated rows (from the planner, or filled by the validator for an external plan), actual rows out, batches, time, peak memory, bytes spilled. Counters are
incremented once per batch, not per row. PostgreSQL's `EXPLAIN` shows `rows`, `width` and cost per node and `ANALYZE` adds actual rows and loops; SQL Server's actual plan carries the same pair. The text is output only (DD9), never parsed back. Estimates that miss by a wide factor are
visible here, which is the only feedback the user asked of a planner on estimates.

## Costs and risks

| Item | Cost or risk | Mitigation |
|---|---|---|
| A second wire format | Writer and reader on two sides for the plan nodes; drift | Shared golden files with doc 4's; the expression nodes are shared, so only relation nodes are new |
| Hostile or careless plans | Unbounded work or memory | Run-time limits are the guard, not the estimates; fuzzing of the decoder and validator |
| Order or uniqueness assumed wrongly | Wrong merge results | Properties derived by construction; cheap order checks in operators that rely on them |
| Scratch store creation cost | Reservation of up to 64 GiB of address space and its tracking tables, or a file (an estimated 100–200 µs on Windows, not measured) | Created lazily on the first spill; pass `max_size` equal to the spill limit so the reservation is small (minimum 1 MiB, P10); a pool if measured |
| G8: user data on disk in scratch files | Retired on 2026-11-09: encryption at rest is out of scope and delegated to the file system's encryption; G8 now holds only for encrypted backups (root PRD annotation, README) | None needed; where scratch files go, and so which file system's encryption covers them, stays open (question 5) |
| Linux writeback of doomed pages | Dirty pages of a scratch file can be written to disk after the expiry although the file is about to be deleted | Documented; `tmpfs` as the scratch directory avoids it (it keeps files "in virtual memory", swappable, default size half of RAM) at the price of using RAM |
| I/O errors on mapped pages | Windows asks code touching a file view to use structured exception handling; Rust cannot catch it | Same exposure as the store's data file today; not new |

## Product questions

Decided by the user on 2026-11-09 (README, "Plans": "Me sirven"): 1 (a), 2 (a) with the error naming the table and both versions, 3 yes, 4 per query only, 6 three-valued logic, 7 (a), 8 yes. The encryption half of question 5 is moot since encryption at rest left the scope (README, encryption entry); where scratch files go stays open for the PRD of module 17. The text below is the analysis as it was proposed.

1. **Format of plans.** Options: (a) Krummi's own, same envelope as the query tree, Substrait vocabulary; (b) Substrait protobuf as the wire format; (c) both from the start.
   Recommendation: (a); a Substrait to query-tree translator is a separate module if someone needs it.
2. **Schema version mismatch.** Options: (a) reject any difference; (b) accept when none of the objects the plan references changed. Recommendation: (a) in the first version, with an error that names the changed table, because (b) needs a per-object version and
   a wrong "unchanged" is a silent wrong result.
3. **Who stores plans.** May applications keep plan bytes across runs? Recommendation: yes, Krummi never persists them, and the version check makes stale ones fail fast.
4. **Default limits.** Memory per query 64 MiB (doc 4's proposal), spill per query 1 GiB, no default rows or deadline; configurable per call. Is a store-wide memory cap needed in the first version?
   Recommendation: per query only, and document that concurrent queries add up.
5. **Spill and encryption (G8), and where scratch files go.** Options: (a) when the store is encrypted, queries never spill; (b) scratch pages are encrypted with a per-query key; (c) `tmpfs` or a user-chosen directory. Default location: a subfolder of the store's folder, or the operating system temp directory?
   Recommendation: (a) until (b) is built; default to a subfolder of the store's folder so that the user controls the disk.
6. **Null semantics in plans.** Doc 4's question 11 decides them for the query tree; plans must behave the same. Recommendation: three-valued logic, as proposed there.
7. **Row order without `Sort`.** Options: (a) unspecified; (b) defined as the scan order of the leaf operator. Recommendation: (a) in the documentation, since two plans of one query may differ, with ties in top-k by primary key (README).
8. **Explain in the C interface.** Per-node stats as a structured array next to the text. Recommendation: yes, so a C# tool can compare estimated and actual rows without parsing text.

## Sources

Repository (paths under `C:\projects\Krummi`):

- `docs/design/tables/README.md`, `03-keys-and-indexes.md`, `04-query-tree-and-planner.md`, `06-c-interface-and-csharp.md` (read whole); `02`, `01`, `05` are referred to through them.
- `crates/krummi-pager/src/pager.rs` (`open_temporary`, `TEMPORARY_RESERVATION = 64 << 30`, `Storage::File`/`Storage::Memory`); `crates/krummi-pager/src/sys.rs` (`reserve_range`/`commit_range` at 1152-1210, `create_temp_file` at 384, `reserve`); `docs/prd/krummi/pager.prd.md` (P10, P15, AC14); `docs/prd/krummi/krummi.prd.md` (G8 at line 143, G9 at 144, D11); `docs/performance/plan.md` (W17 and temporary-area file creation, lines 241-244).
- Commit `fafb5bf perf(pager): mark temporary-area files as temporary on Windows` (named by the coordinator and present in `git log`).

External, fetched on 2026-11-09:

- Substrait: https://substrait.io/ ; https://substrait.io/relations/physical_relations/ ; https://substrait.io/relations/logical_relations/ ; https://substrait.io/relations/basics/ ; https://substrait.io/relations/user_defined_relations/ ("Pending") ; https://substrait.io/spec/versioning/ ; https://substrait.io/community/powered_by/
- Rust and C# libraries, from search results only (setup not verified): https://docs.rs/substrait-prost ; https://www.lib.rs/crates/substrait ; https://www.nuget.org/packages/Substrait.Net ; https://www.nuget.org/packages/flowtidedotnet.substrait
- DataFusion: https://docs.rs/datafusion/latest/datafusion/physical_plan/trait.ExecutionPlan.html ; https://docs.rs/datafusion/latest/datafusion/execution/memory_pool/index.html ; https://docs.rs/datafusion/latest/datafusion/execution/disk_manager/struct.DiskManager.html
- DuckDB: https://duckdb.org/docs/2.0/internals/overview.html ; https://duckdb.org/docs/2.0/guides/performance/how_to_tune_workloads.html
- SQLite: https://www.sqlite.org/opcode.html (first 100,000 characters; `SorterOpen` itself not read)
- PostgreSQL: https://www.postgresql.org/docs/current/using-explain.html (the `external merge` sort line is not on the page, so it is not asserted)
- SQL Server: https://learn.microsoft.com/en-us/sql/relational-databases/showplan-logical-and-physical-operators-reference ; https://learn.microsoft.com/en-us/sql/relational-databases/databases/tempdb-database
- Windows: https://learn.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants ; https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew (`FILE_ATTRIBUTE_TEMPORARY`, `FILE_FLAG_DELETE_ON_CLOSE`, `FILE_SHARE_DELETE`) ; https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-createfilemappingw ; https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-virtualalloc ; https://learn.microsoft.com/en-us/archive/blogs/markrussinovich/pushing-the-limits-of-windows-virtual-memory
- Linux: https://man7.org/linux/man-pages/man2/open.2.html (`O_TMPFILE`) ; https://man7.org/linux/man-pages/man2/unlink.2.html ; https://man7.org/linux/man-pages/man2/mmap.2.html ; https://docs.kernel.org/admin-guide/sysctl/vm.html ; https://docs.kernel.org/filesystems/tmpfs.html

Not verified: Rust `std`'s default share mode on Windows; Linux anonymous-memory overcommit and swap behavior; DuckDB's physical operator names and spill internals beyond the two pages; SQL Server's tempdb log purpose (undo) beyond the Microsoft page's "minimally logged"; whether prost-generated code contains `unsafe`; whether Krummi's encryption (G8) is implemented; the 100–200 µs file-creation figure beyond the coordinator's message.
