# 04 - The query tree, its serialization and a cost-based planner

Part of the [tables design set](README.md). Status: reconciled 2026-11-09 with the user's decisions and the
[fact-check](verification.md). Nothing here is implemented.

## Summary

- A query is a tree of typed nodes (source, filter, aggregate, sort, limit, projection) built by a fluent API in Rust and C#, serialized to a compact versioned binary, validated by Rust in one pass and executed. No text, no SQL.
- The tree carries no values: every literal is a numbered parameter in a separate typed block. The tree bytes are the plan cache key; the planner plans a shape once and re-checks it against the real parameters.
- Serialization: a custom tag-based binary format (about 400 lines per side) beats MessagePack/CBOR, protobuf and FlatBuffers on validation cost, `unsafe`-freedom and shape caching. It stays small enough that a hand-written C# writer and Rust reader are cheap to keep in sync.
- Planner: a small cost model in page reads and rows decoded, fed by **estimates**: per-column histograms that exist also for unindexed columns and are built in the background, plus boundary-descent range estimates on the B+ tree indexes (doc 3). It chooses among full scan, one index range, index intersection and union, with sort avoidance and top-k as plan properties. No exact range counts exist (decided).
- Tables are clustered by their mandatory primary key (doc 2): a primary-key range is a table range, and a secondary index entry is `tuple(columns) ‖ tuple(primary key)` (doc 3).
- Execution: a pull cursor that yields **batches of rows** (a caller buffer, up to 256 rows per call), zero-copy inside the read transaction; the C interface (doc 6) exposes the same batches. C# reads columns through accessors, copying only what it asks to materialize.
- First-version scope (decided): one table per query; filters; vector distance as threshold and as top-k; projection; `ORDER BY`, `LIMIT`, `OFFSET`; aggregates over the whole filtered table. Out: joins, `GROUP BY`, subqueries, functions over columns, writes inside the tree.

## Requirements

From the agreed scope (README) and the root PRD:

1. Queries are a tree built by a fluent API in Rust and C#; C# serializes it, sends it through the C interface, Krummi validates and executes it. No text language, no SQL.
2. Planning is cost-based.
3. Tables have typed columns (including vectors, ULID, UUID), a primary key and secondary indexes on B+ trees (docs 1 to 3). Vector indexes are out of scope; vector distance is a filter and a sort key evaluated on rows.
4. D11: reads are zero-copy while the read transaction is open. D12: `unsafe` only in the pager, so the query layer and the tree decoder in Rust contain none.
5. Reused from the parked documents PRD: parameters separate from the tree (DD6), Rust validates every tree whatever its origin (DD5), results by cursor (DD8), a readable dump that cannot be read back (DD9), plan chosen by cost (DD10, **changed**: the documents PRD used exact counts from the indexes; the user decided on estimates, so counts are replaced by statistics and range estimates), plan explanation (Q16), and selectivity without knowing the parameter (Q17).
6. The user wants no later redesign for performance. So the choices below must not close doors: the wire format is versioned and extensible, plans are cacheable, execution is batch-oriented from the start, and the cost model has one place to be improved.

Non-requirements: a text parser, a schema-less document model (the documents product keeps its own planner per DD11).

## Options compared

### 1. Query tree scope

Node families, with the first version (V1, decided 2026-11-09: "Me sirve") and later.

| Area | V1 | Later |
|---|---|---|
| Source | one table, by name | joins |
| Predicates | `=`, `<`, `<=`, `>`, `>=`, `BETWEEN` as two bounds, `IN (list)`, prefix on text/bytes, `IS NULL` / `IS NOT NULL`, `AND`, `OR`, `NOT` (so `<>` is `NOT (=)`; the builders may offer `ne` as sugar), vector `distance(col, q) < t` (or `<=`) | `LIKE` patterns, regex, expressions between columns, functions over columns |
| Order | `ORDER BY` columns asc/desc, multiple keys, `ORDER BY distance(col, q)` | order by expression |
| Top-k | `limit` with `order by` is top-k; nearest-k by distance is the same node with a distance key | |
| Paging | `limit`, `offset` | keyset paging (`after` a key), cheaper than offset; not in the decided scope |
| Projection | list of columns; the whole row by default; the `distance` of a distance filter or top-k may be projected (implied by top-k; confirm in the PRD) | expressions |
| Aggregates | `count(*)`, `count(col)`, `min`, `max`, `sum`, `avg` over the whole filtered table | `group by` |
| Joins | none | nested-loop join by key (an index lookup on the inner table) |

Reasoning:

- Single-table filters cover the stated need; `OR`, `NOT` and `IN` are needed for them to be useful and map to union and difference of index ranges.
- `count(*)` without a filter is the table tree's entry count (A5, O(1)); `min` and `max` of a primary-key or indexed column are the first or last key of the range. A `count(*)` with a filter is exact, so it walks the matching index range (keys only, no row read when the index covers the filter) or scans; the planner's estimates choose the path but never answer the count. `sum`/`avg` are one pass over rows and cheap to add.
- `group by` forces hash aggregation or sort aggregation, memory limits and spill; it is a large step, so it is proposed as out of V1 but the tree must reserve the node (below).
- Joins multiply the planner (join order, join algorithms). The one that matters for an embedded engine is the nested-loop join by key. Out of V1, with a reserved node.
- The tree is closed over node kinds in a version: a V1 reader rejects an unknown kind with a clear error rather than ignoring it.

### 2. Serialization between C# and Rust

Criteria: stable versioned schema, validation cost, size, ease in C# and Rust, no `unsafe` in the Rust code (D12), parameters separate so the shape can be cached.

| Option | Schema and versioning | Validation and decode | Size | C# and Rust | `unsafe` | Shape caching |
|---|---|---|---|---|---|---|
| Custom compact binary | Ours: a version byte, tagged nodes, explicit length per node so unknown future fields are skippable | One forward pass that is validation and decode at once, no intermediate DOM | Smallest: a node is a tag byte plus small varints | Hand-written writer and reader on each side; the fluent builder maps 1:1 to node writers | None in our code | Trivial: the bytes before the parameter block are the key |
| MessagePack / CBOR | Self-describing, no schema; versioning by convention | Decode to a generic value tree (allocations), then validate shapes by hand | Small | Mature libraries on both sides (MessagePack-CSharp, `rmp-serde`, `ciborium`) | None in our code | Possible only if parameters are kept separate by convention |
| Protocol Buffers | Real schema, good evolution rules, codegen on both sides (`Google.Protobuf`, `prost`) | Full decode to owned structs; unknown fields silently kept; a recursive tree needs a recursion limit anyway | Small | Excellent tooling | None in our code | Needs a canonical serialization for hashing, which protobuf does not guarantee |
| FlatBuffers | Schema, good evolution; offset-based | Zero-copy read, but needs a verifier pass; the Rust `flatbuffers` crate uses `unsafe` internally and its `unsafe` reads are skipped when the verifier is on | Larger (alignment, vtables) | Official generators for both | Dependency uses `unsafe`; our code does not | Needs canonical builders; parameters separate by design is possible |
| Cap'n Proto | Schema, pointer-based; builds well from C++ | Zero-copy with traversal limits | Larger | The C# support is weak and less maintained | Dependency uses `unsafe` | Same issue |

What zero-copy buys here: a query tree is tiny (tens to a few hundred bytes) and decoded once per call, or once per shape when cached. Zero-copy decode is worth nothing at that size. What does matter is validation cost, which the data-driven single pass makes minimal, and shape stability, which needs a canonical encoding.

Verdict: custom compact binary.

- Canonical by construction (the same tree always gives the same bytes: fixed child order, minimal varints, no padding), so a hash of the bytes before the parameter block is the shape key.
- One pass validates version, tags, arity, depth limit (say 64), node count limit, column ids and types against the table schema; the pass produces the typed in-memory plan.
- Few moving parts: about 25 node kinds, six scalar parameter kinds plus vectors.
- Protobuf is the credible alternative if the team prefers generated code over hand-written readers; its cost is the missing canonical form and the extra decode allocation. If the user prefers it, the rest of the design is unchanged.

### 3. Planner models

What each system teaches for a single-table engine with secondary indexes.

**SQLite (NGQP).** The Next-Generation Query Planner chooses a loop nest per table by dynamic programming over the "N best" partial paths; costs combine rows estimated and rows scanned, in a logarithmic scale; statistics come from `sqlite_stat1` (average rows per index prefix) and optionally `sqlite_stat4` (samples), produced by `ANALYZE`; without them it falls back on defaults (equality on an index prefix assumed to return about 10 rows). It supports covering indexes, the multi-index `OR` optimization (union of index lookups), skip-scan, sort avoidance via index order, and an automatic transient index. Lesson: one small cost function, decisions on index usability, covering and order are the big wins. Its weakness is that statistics are stale or missing; ours are built automatically in the background, and are estimates too (doc 3).

**PostgreSQL.** Costs in abstract units: `seq_page_cost` 1.0, `random_page_cost` 4.0 (default, lowered for SSD), `cpu_tuple_cost` 0.01, `cpu_index_tuple_cost` 0.005, `cpu_operator_cost` 0.0025. Selectivity from `pg_statistic`: most common values, an equi-depth histogram, `n_distinct`, null fraction, and physical correlation, combined for `AND` by assuming independence. Plans: sequential scan, index scan, index-only scan, bitmap index scan with `BitmapAnd`/`BitmapOr` (index intersection and union by page bitmap). Prepared statements use custom plans for the first five executions, then a generic plan if its estimated cost is not worse than the average custom plan (`plan_cache_mode` auto). Lessons: the bitmap approach to intersection, the random-vs-sequential distinction, and the generic-vs-custom plan rule, which maps directly to our shape caching with parameters.

**Corax (RavenDB, read in `C:\projects\ravendb\src\Corax`).** Not a cost-based planner. Query matches expose `Count` and a `QueryCountConfidence` (`Low`, `Normal`, `High`); a term match gets its count from the terms tree, and the binary matches (`AND`, `OR`, `AND NOT`) are built over operands whose counts combine into an estimate. Operand order and sort strategy use these counts, and sorting with a take limit uses a top-N sorting match. Lesson: a count taken from the tree is the cheapest statistic, and carrying a confidence with each count tells the planner how much to trust it. We keep the confidence and drop the exactness (the user decided on estimates: the tree has no range counts, doc 3). Corax lacks the cost comparison between a scan and an index; we add it.

**DuckDB.** A columnar vectorized engine with a rule-based rewrite stage followed by a cost-based optimizer, mainly for join order using cardinality estimates (from statistics including min/max zone maps and HyperLogLog distinct counts). It executes in vectors (standard size 2048 rows) with a push-based pipeline model (switched from pull to push in a 2021 pre-release; the version number was not confirmed). Lessons: filter pushdown to the scan, projection pushdown (read only needed columns), and vectorized execution. Join ordering is irrelevant to V1.

What we take: PostgreSQL's cost shape, histograms and bitmap-style intersection, SQL Server's statistics that exist without an index and refresh in the background, SQLite's covering/order/OR handling, Corax's counts with confidence, DuckDB's batch execution and filter/projection pushdown.

### 4. Execution model

| Model | Fit | Cost |
|---|---|---|
| Volcano, row at a time (`next()` per row) | Simple, lowest memory; the call overhead per row is the main CPU cost | One virtual call per operator per row |
| Vectorized batches (pull, a batch of N row references per `next_batch`) | Amortizes calls and dispatch, lets filters run tight loops; compatible with the zero-copy row reference; still lazy, so memory bounded by N | Slightly more complex operators |
| Push-based pipelines (DuckDB, morsel-driven) | Best for parallel analytic scans | Large; wrong for an embedded engine whose reads are one transaction per thread |
| Materialize everything | Simplest; breaks D11 and memory | Rejected by DD8 |

A vectorized pull cursor costs little more than row-at-a-time and is the choice that avoids a later redesign: a row-at-a-time API can always be built on top of batches, not the other way round.

## Decided design

### The query tree

All nodes are immutable, owned by the builder, built through a fluent API that mirrors the node kinds. Sketch of the node types (Rust names; C# names are the same):

```text
Query        { version, source: Source, root: Node, params_layout }
Source       Table(name) | (reserved) Join(left, right, on)   // V1 accepts Table only

Node  (relational, forms a short chain: Scan -> Filter -> Aggregate? -> Sort? -> Limit? -> Project)
  Filter     { predicate: Expr }
  Aggregate  { aggs: [Agg], group_by: [] }                    // group_by must be empty in V1
  Sort       { keys: [SortKey { expr: Column | Distance(col, param), desc }] }
  Limit      { limit: Param|u64, offset: Param|u64 }
  Project    { columns: [ColumnId] | All }                    // computed: Distance(col, param) as column

Expr  (boolean tree)
  And(Vec<Expr>) | Or(Vec<Expr>) | Not(Expr)
  Cmp { col: ColumnId, op: Eq|Lt|Le|Gt|Ge, value: Param }   // between = Ge and Le; <> = Not(Eq)
  In  { col, values: Param }           // a list parameter, sorted and deduplicated by Rust
  Prefix { col, value: Param }
  IsNull { col, negated }
  VectorWithin { col, query: Param, metric, max_distance: Param }
```

Notes:

- Columns are referenced by id after the C# builder resolves names against a schema it fetched; a name-based variant is allowed on the wire and resolved by Rust, because Rust validates every tree anyway (DD5). The shape key uses the resolved ids, so the cache works in both cases.
- Each value is a `Param(index)` with a declared type; the parameter block carries the typed values, and the tree has no literals. The type of each parameter is part of the shape (a `bigint` and a `text` parameter give different shapes) so type errors are shape errors, caught once.
- `IN` list length is a runtime property of the parameter; the planner treats it as a number of equality probes known only with the values, so the cost is computed at execution of the first plan step (see "Plan caching").
- The builder normalizes: nested `And` flattened, `Not` pushed down (De Morgan; `Not(Cmp)` of an ordering operator flips it), constants folded when a parameter is declared constant, double negation removed. Rust normalizes again on receipt, so a different client cannot bypass it, and the normalized tree is what is hashed for the shape.
- Predicates carry typed parameters; Rust, not the client, encodes them into the tuple elements of doc 3 (a `Cmp` bound, an `IN` probe, a `Prefix` as the type code plus the escaped bytes without the terminator), so index range bounds are `tuple(columns) ‖ tuple(primary key)` prefixes and the C# builder needs no key encoder.
- Three-valued logic is a question for the PRD (see question 11): a comparison with NULL is not true, so `NOT (x = 1)` does not return rows where `x` is NULL unless the tree says `IS NULL` too.
- Reserved tags (decided): `Join`, `GroupBy`, and a general `Extension(tag, len, bytes)` that V1 rejects. The version byte plus length-prefixed nodes allow new node kinds without a new format.

### Wire format

```text
bytes: magic(2) version(1) flags(1)
       table_id_or_name   node_count(varint)
       nodes...           // each: tag(1) len(varint) payload
       param_block        // count(varint), per param: type(1) len/value
```

- Integers as varints, strings as length-prefixed UTF-8, parameters in a separate block so that vector parameters (f32 arrays) are read as `&[u8]` with `from_le_bytes` without copying (no alignment is needed, doc 5). This section is the one description of the wire format; doc 6 refers to it. Nodes are written in pre-order, each as tag, length and payload; children follow their parent by the arity its tag defines.
- The encoder emits children in a deterministic order (sorted `And`/`Or` operands by column id, then operator, then param index); this canonical order is what makes the shape key stable.
- The decoder checks: version supported, depth <= 64, node count <= 4096, total size <= 1 MiB, tags known, column ids exist, parameter types match, parameter count matches the largest index, vector dimensions match the column. It builds the plan input directly, with no intermediate generic value tree.
- Shape key = hash of `table + nodes` bytes (a 128-bit hash); the parameters are not hashed.
- A human-readable dump (DD9) is produced from the validated tree, output only.

### The planner, step by step

Input: normalized tree, parameter values, table schema, index definitions, the table's and indexes' entry counts (A5, exact, whole tree) and the column statistics (doc 3).

1. **Rewrite.** Push the filter below sort/project (already the order), merge adjacent filters, drop `Project` columns not used downstream (projection pushdown to the row decoder), mark which columns are needed so the row decoder materializes only those.
2. **Enumerate access paths for the filter**, per leaf predicate:
   - Primary key range/equality: the table is clustered by primary key (doc 2), so key ranges are table ranges.
   - Secondary index range/equality/prefix/`IN`: an index on a column (or leading columns of a composite).
   - Full scan of the table.
   - `Or` becomes a union of per-branch paths if every branch has an index path (SQLite multi-index OR), else the whole `Or` is a residual filter on a scan.
   - `And` becomes: pick the best single path and apply the rest as residual filters, or intersect two paths (bitmap-style: collect the primary keys, which are the tail of each secondary entry's key (doc 3), from the more selective and probe the other; see below).
   - Vector predicates are residual filters on whichever path produces the fewest rows; distance computations are the expensive part, so they are applied last.
3. **Estimate.** For every candidate path the planner estimates the rows it returns, from three sources in this order: (a) an index range, by the boundary-descent estimate of doc 3 (`estimate_range`, exact when both bounds fall in one leaf, for instance equality on a unique index); (b) any predicate on a column, indexed or not, by that column's histogram, most common values and null fraction; (c) a default fraction, if the column has no statistics yet or they are stale, in which case the planner also asks the store to build them in the background and does not wait (doc 3). `IN` lists sum their equality estimates. Each estimate carries a confidence: `Exact` (same-leaf descent, whole-tree entry count, unique equality), `Histogram`, or `Default`; the table row count is the table tree's entry count (A5).
4. **Cost** each path (see below), combine residual selectivity assuming independence between predicates, and keep the cheapest. `And` selectivity of two paths is `sel1 * sel2` as in PostgreSQL, and the intersection is always estimated, never executed to be counted.
5. **Order.** If the query has `ORDER BY` and the chosen path already produces that order (index order of the same columns, ascending or descending by reverse traversal, or primary key order), drop the sort node. Otherwise compare two plans: (a) the cheapest filter path plus an explicit sort or top-k, (b) a plan walking an index that provides the order and filtering rows, stopping at `limit` (cheap when the filter is not very selective). Both are costed; the cheaper wins.
6. **Top-k.** `Sort + Limit` with limit `k` becomes a bounded heap of `k + offset` rows (memory O(k)), never a full sort. Distance ordering is the same node with a distance key.
7. **Aggregates.** `count(*)` with no filter is the table tree's entry count. `count(*)` over a filter that is a single index range with no residual walks that range's keys without reading rows; the answer is exact and is never taken from an estimate. `min`/`max` of a primary-key or indexed column over a range reads the first or last key of the range (nulls sort first or last by the column's declaration, doc 3). Otherwise a streaming aggregate over the rows.
8. **Covering.** If every needed column is a fixed-width column of the index key or of the primary key (every secondary entry carries it: the key is `tuple(columns) ‖ tuple(primary key)`), or one of the entry's included columns, skip the row fetch. A `string` or `bytes` column of the index key or of the primary key does not count: its key element is escaped, so returning it without a copy needs the row (doc 3; corrected 2026-11-10 after the Codex follow-up review, R4). A query that needs only fixed-width primary-key columns can even read just the table tree's keys.
9. **Emit** a physical plan: a small tree of operators (`IndexRangeScan`, `TableScan`, `Union`, `Intersect`, `Fetch`, `Filter`, `TopK`, `Sort`, `Limit`, `Aggregate`, `Project`), plus a per-step record of estimated rows and cost for the explain output (Q16).

### Cost model

Units: abstract cost, in the spirit of PostgreSQL, fed by the estimates above and by the page and entry counts the trees keep.

```text
cost(path) = pages_read * page_cost
           + rows_decoded * row_cost
           + rows_filtered * filter_cost
           + vector_dims_evaluated * dim_cost
```

- Full scan: `table_pages * seq_page_cost + table_rows * row_cost`. The page count comes from the table's leaf pages (stored or derived from the B+ tree).
- Index range returning `n` keys: `height * random_page_cost + ceil(n / keys_per_leaf) * seq_page_cost` for the index leaves; then, unless covering, `n * (random_page_cost + row_cost)` for the row fetches, each a descent of the clustered table tree by the primary key read from the entry (a worst case: a cold random access per row, cheaper when the rows cluster). A range of the primary key itself costs only its leaves.
- Intersection of two index ranges: scan the smaller range, collect its primary keys, sort them (so row fetches are in key order and page-friendly), and probe the second index for each key if the second range is large, or merge-intersect two sorted key lists if both are small. The cheaper of the two is chosen by cost.
- Constants: start from `page_cost = 1`, `random_page_cost = 2` (the engine is mostly in the OS cache or on SSD; the user can calibrate), `row_cost = 0.02`, `dim_cost = 0.0005`. These live in one `CostParams` struct so measurement can adjust them without changing the planner (the lever: "optimization by measurement", per the project principles). A micro-benchmark on this machine, run later and not now, can fit them.
- Selectivity estimation is the largest source of error in every planner, and here it rests on estimates that need be neither exact nor current (decided). Errors come from stale or default statistics, from combined predicates (independence assumption) and from the filtered fraction of a vector predicate (no statistics exist for vectors; a default of 0.1 % to 1 % is used, doc 5). The plan will therefore be a good one, not always optimal; the explanation prints estimated against actual rows so that a wrong estimate is visible.

### Plan caching per shape

1. Key: shape hash (plus table schema version and index set version, so DDL invalidates plans).
2. The cache stores the physical plan built for a parameter sample, together with the access path alternatives that were within a margin (say 2x) of the winner.
3. On execution with new parameters, a cheap "revalidation" re-estimates only the leading access path with the actual parameters (a histogram lookup in memory, or two boundary descents on an index). If the estimated rows stay within a factor (say 4x) of the estimate used at planning, the cached plan runs; if not, the planner runs again for this call (PostgreSQL's custom vs generic plan rule, simplified).
4. Shapes whose parameters change the best path often (low-selectivity predicates) are marked "plan per call" after two replans, and skip the cache until they stabilize.
5. Cache is per database handle, bounded by entries (for example 256) with LRU, never persisted, never shared across processes.

This is the answer to Q17 of the documents PRD: average selectivity is only needed for the first planning when no parameter exists; with parameters always present on the wire, the histogram and the boundary descent estimate the real value cheaply, so average selectivity is not needed in V1 beyond the default used while statistics are missing.

### Execution model

- Physical operators implement a pull API: `fn next_batch(&mut self, out: &mut Batch) -> Result<bool>`.
- A `Batch` holds up to `N = 256` entries, each a row reference: a `&[u8]` borrowed from the page (D11) plus the primary key slice (both are in the clustered leaf; fixed-width primary key columns are read from the key, `string` and `bytes` ones from the row, doc 1, Codex R4). N is tunable per operator; the aim is a batch that fits in L1/L2 cache, and 256 is a conventional start. Operators that cannot keep a reference (sort, aggregate, intersection) copy the key bytes they must keep into transaction-owned arena memory.
- Filters run per batch: decode only the columns the predicate needs (offset-based access, doc 1) and set a selection vector, so rows that fail are never fully decoded. Vector distance runs last in the chain.
- Lifetimes: batches and row references borrow the read transaction, so the compiler enforces D11; no `unsafe` in the query layer (D12). The cursor owns the transaction guard or borrows it.
- Zero-copy is available on the Rust side. Across the C interface, a reference to engine memory is valid only while the read transaction is open; the C interface (doc 6) hands out these pointers with explicit transaction scope.

### How C# reads rows

1. `Execute(tree_bytes, params, read_tx)` validates the tree, finds or builds the plan (the shape cache is inside the
   engine and transparent: there is no `Prepare` function in the first C surface; one can be added later because
   functions are only added, doc 6) and returns a cursor handle that belongs to a read transaction handle.
2. `cursor.NextBatch(buffer)` fills a caller-provided buffer of result rows (a `Span<ResultRow>` of up to 256 entries)
   and returns how many it wrote; 0 means the end. Decided: the cursor returns batches of rows. Each entry is a
   reference `{key ptr,len; value ptr,len}` to a table row in the page, valid until the next call on the cursor or until the
   transaction closes. C# reads columns through accessor calls (`GetInt64(row, col)`, `GetSpanUtf8(row, col)`,
   `GetVector(row, col)`): a non-key column is a cheap decode of the row bytes (doc 1), a primary-key column a decode
   of the key (doc 3) for a fixed-width one, and a read of the row for a `string` or `bytes` one, which doc 1 stores raw
   in the row (Codex R4), and either returns a `ReadOnlySpan<byte>` for text, bytes and vectors (zero-copy, tied to the
   transaction scope) or a primitive by value.
3. Computed values (a projected `distance`, aggregates) have no row in a page: they travel in a small buffer owned by
   the cursor and valid for the same period, read by the same accessors with a result-column index. The exact layout
   belongs to the PRD of module 17.
4. For simple users, a `ToList<T>()` helper in C# materializes the rows with a source-generated mapper after the batch call; this is C#-side convenience, outside the engine.
5. Aggregates return a single row with typed values, through the same cursor.

The exact C calling convention (opaque handles, error codes, ownership, how a safe interface reaches `unsafe` at the boundary only) belongs to doc 6.

## Costs and risks

| Item | Cost or risk | Mitigation |
|---|---|---|
| Custom wire format | We maintain a writer, a reader and tests on two sides; drift is a risk | One shared set of golden files (bytes and expected dump) used by both test suites; fuzz the Rust decoder; version byte from day one |
| Estimates are wrong | Stale or default statistics, correlated predicates and uneven fill after bulk deletes give a poor plan, never a wrong result | Background refresh and `analyze` (doc 3); plan revalidation against actual parameters; the explanation prints estimated against actual rows; `CostParams` margins |
| Independence assumption | Correlated `AND` predicates mis-estimate | Accept in V1; histograms and the boundary-descent estimate on the leading predicate bound the error; a later multi-column statistic can be added in doc 3 without changing the tree |
| Cost constants | A wrong `random_page_cost` picks poor paths | One `CostParams`; calibrate by benchmark later (not now) |
| Plan cache invalidation | A stale plan after DDL or a data skew | Key includes schema/index version; re-estimate check; replan on divergence |
| Vector filters | Distance is O(rows * dims); a bad residual order burns CPU | Apply last; cost term `dim_cost`; top-k heap; vector indexes are out of scope |
| Batches and sort/aggregate need owned memory | A big sort uses memory | Memory budget per query with a typed error; spill to disk deferred |
| Offset paging | O(offset) work | Keyset paging is a candidate for a later version (not in the decided scope) |
| Reserved node kinds | Over-engineering if never used | Only reserved tags in the format, no code until a PRD asks for them |
| C# zero-copy spans | Misuse after the transaction closes is memory unsafety on the C# side | The binding ties spans to a transaction scope and invalidates them (doc 6) |
| Recursion in decode | Hostile trees can exhaust the stack | Depth limit 64, node limit, iterative or bounded recursion, fuzzing |

Principles check: the design is DRY (the same nodes serve filters, estimates, explain), avoids premature abstraction (no join, no group by code), and treats performance by design (batch cursor, shape cache, statistics-based planning) without any unmeasured micro-optimization.

## Product questions

1. **First version scope of aggregates.** *Decided 2026-11-09:* `count`, `min`, `max`, `sum`, `avg` over the whole
   filtered table, no `group by` (README: "Me sirve"). `group by` needs hash aggregation and memory control: a later PRD.
2. **Joins.** *Decided:* none in V1; tags for joins and `GROUP BY` are reserved in the format.
3. **Paging.** *Decided for V1:* `limit` and `offset` only (README scope). Keyset paging is not in the decided scope;
   it remains an option for a later version, because offset is O(offset) and keyset is nearly free with ordered
   indexes.
4. **Wire format.** *Still open.* Options: (a) custom compact binary; (b) protobuf; (c) FlatBuffers. Recommendation:
   (a), for the canonical shape key, the single-pass validation and no dependency using `unsafe`. Choose (b) only if
   you value generated code over those. (The fact-check did not fetch the protobuf, FlatBuffers or Cap'n Proto
   claims; they are standard knowledge and none changes the choice.)
5. **Dependencies with `unsafe` inside.** *Still open; policy proposed.* D12 governs Krummi's own code, and its
   annotations already accept `unsafe` inside `lz4_flex` for the pager (2026-11-06) and the benchmark tool (2026-11-03).
   Proposal for every document: a dependency that contains `unsafe` is acceptable when it is widely used and audited
   and its use is recorded by an annotation; the choices in these documents do not need one (the query layer and the
   vector kernels use none; the C interface is Krummi's own `unsafe`, doc 6). The vector crates `wide`, `pulp` and
   `simsimd` are left out for determinism and cost reasons, not because of `unsafe` (doc 5).
6. **Name-based columns on the wire.** *Still open.* Options: (a) only column ids, resolved by the C# builder from a
   schema it fetched; (b) names, resolved by Rust; (c) both. Recommendation: (c), names for simplicity, ids as an
   optimization, with the same shape key.
7. **Plan visibility.** *Still open.* Should the developer be able to ask for the chosen plan, its estimated and actual
   rows (Q16), and force a path (an index hint)? Options: (a) explain only; (b) explain and hints; (c) neither.
   Recommendation: (a) in V1; hints later, if measurements show the planner mistaken.
8. **Collation of text.** *Decided:* binary (byte order of UTF-8) only, no collation (README), which is what makes
   prefix and range scans exact.
9. **Resource limits.** *Still open.* Maximum query size, depth, result memory for sorts. Recommendation: defaults
   1 MiB, depth 64, 64 MiB per sort/aggregate, all configurable, errors typed.
10. **Statistics beyond exact counts.** *Decided 2026-11-09:* persisted per-column statistics with histograms in the
    first version, also for unindexed columns, built in the background and forced by `analyze` (doc 3).
11. **Null in comparisons.** *New, still open.* Options: (a) SQL's three-valued logic (a comparison with NULL is not
    true, so `NOT (x = 1)` skips rows where `x` is NULL; `IS NULL` selects them); (b) two-valued logic in which NULL
    never matches a comparison but `NOT` inverts the result. Recommendation: (a), as every SQL engine the users know.

*Annotation of 2026-11-19 from the statistics PRD ([tables-statistics.prd.md](../../prd/krummi/tables/tables-statistics.prd.md), 7.3), by the user's decision:* a conjunction of equalities is not combined with `sel1 * sel2` (independence) but with an exponential backoff biased to overestimate: with the fractions of the groups sorted ascending, `f1 * f2^(1/4) * f3^(1/16) * f4^(1/64)`, where an index prefix that covers several columns is one group (ST18). There are no default estimates: without statistics the estimate samples (ST3).

## Sources

Local:

- `C:\projects\Krummi\docs\design\tables\README.md` (scope).
- `C:\projects\Krummi\docs\prd\krummi\krummi.prd.md` (D11, D12).
- `C:\projects\Krummi\docs\prd\documents\documents.prd.md` (DD4 to DD11, Q12, Q16, Q17).
- `C:\projects\ravendb\src\Corax\Querying\Matches\*.cs` and `IndexSearcher.*.cs` (`Count`, `QueryCountConfidence`, `AndNotMatch.cs`, `BinaryMatch.cs`, `IndexSearcher.Sorting.cs`).

External. The SQLite, PostgreSQL and DuckDB claims were checked against their documentation in the fact-check of 2026-11-09 ([verification.md](verification.md), claims 4.1-4.10); the serialization claims (protobuf, FlatBuffers, Cap'n Proto, MessagePack) were not fetched and are standard knowledge.

- SQLite, "The Next-Generation Query Planner": https://www.sqlite.org/queryplanner-ng.html; "Query Optimizer Overview": https://www.sqlite.org/optoverview.html; "The SQLite Query Planner": https://www.sqlite.org/queryplanner.html; `ANALYZE` and `sqlite_stat1/stat4`: https://www.sqlite.org/lang_analyze.html.
- PostgreSQL documentation: "Planner/Optimizer" (https://www.postgresql.org/docs/current/planner-optimizer.html), "Planner Cost Constants" (https://www.postgresql.org/docs/current/runtime-config-query.html), "Statistics used by the Planner" (https://www.postgresql.org/docs/current/planner-stats.html), `PREPARE` and `plan_cache_mode`.
- DuckDB documentation: "Execution Format / Vectors" (https://duckdb.org/docs/internals/vector) and the DuckDB optimizer blog posts; Boncz, Zukowski, Nes, "MonetDB/X100: Hyper-Pipelining Query Execution" (CIDR 2005) for vectorized execution; Graefe, "Volcano: An Extensible and Parallel Query Evaluation System" (IEEE TKDE 1994) for the iterator model.
- Serialization: FlatBuffers (https://flatbuffers.dev), Cap'n Proto (https://capnproto.org), Protocol Buffers (https://protobuf.dev), MessagePack (https://msgpack.org) and CBOR (RFC 8949).
