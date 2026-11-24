# Approximate nearest neighbour (ANN) vector indexes

Technical design, document 12 of the [tables design set](README.md). Written 2026-11-10, after the user asked which
ANN indexing algorithm suits Krummi: "Seleccionar un algoritmo ANN **equilibrado, práctico y razonablemente sencillo
de implementar y mantener**. No buscamos el algoritmo con el mayor rendimiento absoluto". Document 5 left vector
indexes out "de momento", without closing the door on them; this document is the follow-up that opens that door.
The user then brought them into scope: "Además los índices ANN hay que inluirlos en el alcance". The tables PRD
([tables.prd.md](../../prd/krummi/tables/tables.prd.md)) was annotated the same day with child PRD 7.6, *Approximate vector
indexes*, built after 7.5 and before module 17, with this document as its technical starting state.

Status: a proposal. Nothing here is implemented, built or measured. The six decisions in *Product questions* need the
user's approval; PRD 7.6 settles them.

Annotated 2026-11-24: the in-memory experiment of *Benchmarks*, steps 1 to 3, has run. Its results are in *Measured
result* below and in the [report](../../benchmark/2026-11-24-ann-experiment.md). The rest of the document is unchanged,
and its **[M]** estimates are kept as they were written.

History: a first evaluation (v1) assumed that the index is maintained inside the transaction that changes the table.
An external review relayed by the user (findings R1-1 to R1-9, *Review history* below) and a change of requirement
the same day replaced it (v2, this document). The requirement change, verbatim: "Los índices vectoriales ANN de
Krummi pueden ser asíncronos y eventualmente consistentes. No tienen que actualizarse dentro de la transacción que
modifica la tabla." This document calls it A1.

**Evidence tags.** Every claim that is not a design decision carries one:

- **[L]** literature or public documentation, cited from memory and not re-fetched; check before quoting it in a PRD.
- **[K]** a hypothesis about Krummi, derived from its code and its design documents.
- **[M]** needs measurement before anyone relies on it.
- **[V]** verified in this repository (file named).

## Summary

1. **Recommendation: IVF with SQ8 codes and exact re-ranking (IVF-SQ8 + rerank), maintained asynchronously.** A
   write transaction only records the primary key of each changed row in a small per-index `pending` tree; a
   background worker brings the index up to date in short batches.
2. A1 helps HNSW, which loses its main penalty (the work inside the write transaction), but HNSW keeps its background
   cost, its deletes, its weakness under filters and its RAM needs. Under the weighted matrix IVF-SQ8 scores **76**
   and HNSW **56**.
3. Approximate search is **opt-in**, through a new query node `ApproxNearest`. Exact queries never use the index and
   keep every guarantee of document 5.
4. A query reads the index **and** evaluates the pending rows exactly at its snapshot, so the index's lag never hides
   a row: results are approximate only through the partitioning and the quantization, never through staleness. The
   price is time in proportion to the backlog.
5. One step, `reconcile`, is the only writer of the index. It serves maintenance, the initial build and rebuilds, so
   no write holds the write turn for long and no second code path for an exclusive build is needed.
6. **Fallback: HNSW stored in pages**, maintained by the same `pending` tree and worker, only if IVF-SQ8 misses the
   recall and latency thresholds of *Benchmarks*.
7. The exact scan of document 5 stays the reference for correctness, and the planner keeps using it when the filters
   are selective or no generation of the index is ready.

## Requirements

From document 5, the user's request and A1:

- **N1. Exact queries are untouched.** A distance threshold and an exact top-k never use an ANN index and return the
  same rows under any plan (R4 of document 5).
- **N2. Asynchronous maintenance (A1).** The table keeps all its ACID and MVCC guarantees. The extra cost of a write
  transaction should be minimal, ideally recording durably what changed.
- **N3. Durability and recovery.** The index recovers correctly after a crash or a restart, with no stale entries and
  no orphan pages.
- **N4. Visibility.** An ANN query may temporarily miss recent vectors, but never returns a row that is not visible,
  or that does not satisfy its filters, in its snapshot.
- **N5. Reuse.** No second transactional engine: the asynchronous propagation reuses trees, transactions and the
  journal.
- **N6. Simplicity and robustness** over peak performance. From the request: "Prioriza la simplicidad siempre que no
  implique una pérdida significativa de rendimiento", "No presupongas que todo el índice puede residir en RAM" and
  "No optimices exclusivamente para búsquedas: las modificaciones también importan."

## What Krummi has

- 8 KiB pages. A leaf cell holds at most 4,078 bytes; anything larger goes to the large-value path, contiguous, up to
  256 MiB, read without copying **[V]** (`crates/krummi-tree/src/node.rs`, `MAX_LEAF_CELL`;
  `crates/krummi-pager/src/format.rs`).
- **A single writer per store (G4)**, writers served in order of arrival, concurrent readers with snapshots,
  copy-on-write pages in the scratch area.
- A read transaction and its tree handles can be shared between threads **[V]** (AC-A20,
  `crates/krummi/tests/ui/trees_pass/read_tree_handle_crosses_threads.rs`).
- A deferred commit (`commit_deferred`) returns without waiting for the disk; a crash may lose it, "always together
  with every later commit and never in part" **[V]** (`crates/krummi/src/lib.rs`; journal group commit PRD, E5 and
  AC-J7). What survives is always a prefix of the commits.
- A long-running read keeps old page versions alive: the committed version stays in the scratch area until no open
  read needs the version before it, and free pages cannot be reused meanwhile (module 4).
- Normal trees, multi-value trees and fixed-size trees; a tree's entry count is known at once (A5). Containers,
  posting lists, compact trees and lookups (modules 13 to 16) **do not exist yet**.
- Tables, secondary indexes, statistics and the planner are designed, not implemented:
  - document 5: element types `f32`, `f16`, `i8` and `bit`, dimension up to 4,096; distances bit-identical on every
    platform, with fixed-order kernels that accumulate in `f64` since its R3, at up to about twice the arithmetic cost
    of `f32` lanes (its estimate); exact search with a pre-filter and a bounded top-k heap; a vector is never a key of
    an index;
  - document 3: FoundationDB tuple keys; a persistent modification counter per table; statistics built by a thread of
    the store from a sample in a read transaction and stored with a short write, with one lock order (the write turn
    last) and a check that the table and the column are still the same before storing;
  - the tables PRD: a table's internal trees never appear in the catalog; deleting a table deletes its indexes and
    statistics and frees their pages (E2).
- RavenDB's HNSW lives in Voron and relies on containers, posting lists, a compact tree and lookups. Deletes leave
  tombstoned nodes, and no graph repair code was found **[V]** (`ravendb/src/Voron/Data/Graphs/Hnsw.cs`).

## Options compared

### Alternatives

| Alternative | Advantages | Drawbacks | Difficulty specific to Krummi |
|---|---|---|---|
| **Exact scan** (document 5) | Recall 1. No index to maintain. Predictable cost. Perfect with a pre-filter. | O(N): about 0.3 s per query for 1 M × 768-dim `f32` on one core (document 5's estimate), up to about twice that if the `f64` arithmetic and not the memory bounds the scan **[K][M]**. | None: already designed. Stays the reference for correctness. |
| **Approximate SQ8 scan, then rerank** | Reads 4× fewer bytes than the exact scan. | Still O(N), and **approximate**: the rerank makes the distances exact, not the choice of candidates. | Kept as a benchmark experiment only, computed in memory; never a stored structure. |
| **IVF-Flat** | Very simple **[L]** (Jégou et al., TPAMI 2011; Faiss). One entry per row. Lists are read in order. | Needs k-means training. Lower recall and QPS than HNSW **[L]**. Duplicates the vector in `f32`. | An `f32` vector over ~1,000 dimensions spills off its leaf: one extra page read per entry. |
| **IVF-SQ8 + rerank** | As IVF-Flat, reading 4× fewer bytes. Returned distances are exact. | Trained; loses some recall before the rerank **[L]**. | The code fits a leaf up to ~4,050 dimensions. Fits document 3's tuple-keyed trees. |
| **IVF-PQ / RaBitQ** | Tiny codes (16 to 64 bytes), scales to tens of millions **[L]** (Jégou et al. 2011; Gao and Long, SIGMOD 2024). | PQ needs trained codebooks and lookup tables per query; its recall depends on the rerank. | Codebooks stored and versioned per generation. More code and tests. |
| **HNSW** | The best QPS/recall in memory **[L]** (Malkov and Yashunin, TPAMI 2018; ann-benchmarks). No training. | An insert searches with `ef_construction` (thousands of distances) and rewrites on the order of M0 (≈32) neighbour lists. Deletes need tombstones and repair: pgvector documents that vacuuming an HNSW index is slow, and nodes become unreachable (Xiao et al. 2024) **[L]**. Restrictive filters break connectivity (ACORN, Patel et al., SIGMOD 2024) **[L]**. Needs its working set in RAM: a query makes 1,000 to 3,000 random node reads **[L][K]**. | With A1 the insert leaves the write transaction, but the worker still copies about 30 to 40 random pages per insert in the write turn **[K][M]**. Needs dense node ids (a fixed-size tree can hold them). |
| **DiskANN / Vamana** (Fresh, Filtered) | Designed for SSD with little RAM **[L]** (Subramanya et al., NeurIPS 2019). Scales to hundreds of millions. | Needs PQ codes in RAM. FreshDiskANN's temporary index and merge are an LSM of its own **[L]** (Singh et al. 2021). Filtered-DiskANN handles labels only (Gollapudi et al., WWW 2023). | The `pending` tree would replace the temporary index, but each insert still adds about 64 back-edges. The most complex option. |
| **SPANN / SPFresh** | IVF at billion scale; SPFresh rebalances in place with LIRE **[L]** (Chen et al., NeurIPS 2021; Xu et al., SOSP 2023). | A graph of centroids in RAM, several lists per vector, a rebalancer. | Many subsystems of its own. Splitting an oversized list locally can be taken alone later. |
| **LSH** | Trivial. | Poor recall/space trade-off on dense embeddings **[L]**. | Discarded. |

### What A1 changes in the comparison

Under v1, the cost of maintaining the index was inside every write transaction. Under A1 it splits in two:

- **Foreground:** the cost every write transaction pays. With the `pending` tree it is the same for every algorithm:
  one small upsert per changed row and per vector index. It no longer separates the alternatives, so it leaves the
  matrix.
- **Background:** the work of the worker. It still matters in Krummi, because the worker's writes take the single
  write turn (G4), copy pages and go through the journal, and because a worker slower than the writes lets the backlog
  grow, which every ANN query then pays.

### Weighted decision matrix

Weights, from the criteria of the request and N6:

| Criterion | Weight | Why that weight |
|---|---|---|
| 1. Latency and recall | 20 | With the write path out of the way, search quality weighs relatively more. The exact scan still covers correctness. |
| 2. RAM and disk accesses | 15 | An embedded engine; the index cannot be assumed to fit in RAM. |
| 3. Background maintenance | 15 | CPU, pages copied, journal bytes and time in the write turn per change; the backlog's growth. |
| 4. Simplicity and robustness | 20 | N6: the requirement change asks for it first. |
| 5. Scalability | 10 | From thousands to millions. |
| 6. Integration, persistence and recovery | 10 | N3, N5: everything in versioned pages, no second engine. |
| 7. Filters and the planner | 10 | A cost-based planner with histograms needs a predictable cost; N4. |

Scale (1 to 5): latency 5 = HNSW in memory, 1 = O(N); background 5 = one entry and a few pages per change, 1 = tens of
random pages or a merge process; simplicity 5 = reuses existing pieces, 1 = several new subsystems; filters 5 = exact
and linear, 2 = recall degrades under filters with an unpredictable cost.

Scores **[K]**, ordinal, justified by the table of alternatives:

| | Lat. | RAM/disk | Background | Simplicity | Scale | Integr. | Filters | **Total /100** | v1 total |
|---|---|---|---|---|---|---|---|---|---|
| **IVF-SQ8 + rerank** | 3 | 4 | 4 | 4 | 3 | 5 | 4 | **76** | 80 |
| IVF-PQ / RaBitQ | 3 | 5 | 4 | 3 | 4 | 4 | 4 | 75 | 76 |
| IVF-Flat | 2 | 3 | 4 | 4 | 2 | 3 | 4 | 63 | 69 |
| SPANN / SPFresh | 4 | 4 | 3 | 1 | 5 | 2 | 3 | 61 | 61 |
| HNSW in pages | 5 | 2 | 2 | 2 | 3 | 3 | 2 | **56** | 53 |
| DiskANN | 4 | 4 | 1 | 1 | 5 | 2 | 2 | 53 | 47 |

**The exact scan is a gate, not a candidate.** Its scores (latency 1, RAM/disk 3, background 5, simplicity 5, scale 1,
integration 5, filters 5) give 70, and the approximate SQ8 scan about 72; under v1's weights the exact scan gave 76.
A weighted sum rewards doing nothing, so both are used as a **knock-out gate**: serve 1 M vectors with a p95 of about
20 ms. They fail it above roughly 50,000 to 100,000 vectors **[K][M]**.

IVF-PQ is one point behind: it scales better and needs less disk, but codebooks and per-query lookup tables are more
code to write, test and version. It is the natural next step if tables grow past what SQ8 codes serve (*When the
recommendation would change*).

**Sensitivity.**

- HNSW gains 3 points and IVF-SQ8 loses 4 against v1: the gap narrows from 27 to 20, but the order does not change.
- HNSW is ahead only on latency (+2). Its deficits weigh 2·15 + 2·15 + 2·20 + 2·10 + 2·10 = 140. To win, latency must
  weigh at least 70, that is **about 47 % of the total weight**.
- Even scoring HNSW's background 3 and its simplicity 3 (batched inserts, with the search done outside the write turn)
  gives 63 against 76.

**Why A1 does not change the winner.** It removes HNSW's largest v1 penalty, the work inside the write transaction.
It does not remove the 30 to 40 pages copied per insert, the deletes and their repair, the random reads that need the
graph in RAM, or the poor behaviour under filters. It also helps IVF: the same machinery gives an online build and
rebuild almost for free, and removes the need for a reproducible assignment (see the locator below).

## Recommended design

### Structures

Per vector index, as internal trees of the table, which the catalog never lists (tables PRD):

| Tree | Key | Value | Scope |
|---|---|---|---|
| `meta` | fixed slots | definition; state (`training`, `building`, `ready`); active generation; building generation and its **cursor** (a primary key); quantizer and centroids of each live generation (the centroid matrix as one large value); counters of *SQ8 quantizer and drift* | Per index |
| `pending` | primary key | empty | Per index; small, because the worker drains it |
| `postings.g` | `tuple(partition columns…, list_id) ‖ primary key` | SQ8 code | Per generation `g` |
| `locator.g` | primary key | `list_id` (and the partition columns) | Per generation `g` |

`f(row)` denotes the entry a row should have in generation `g`: none when the row does not exist, otherwise
`(assign_g(vector), sq8_g(vector))` with its partition columns. `assign_g` is the nearest centroid of `g` under
document 5's kernels, ties to the lowest list id.

The **locator** lets the worker delete the old entry of a row without recomputing it. The row's old vector is gone by
the time the worker runs, so A1 requires it, and it also means correctness never depends on reproducing an
assignment bit for bit. The assignment stays deterministic (document 5's kernels on the platforms CI tests), which the
integrity check uses to compare `postings` with `f(row)`.

Keying `postings` by `list_id ‖ primary key` does not contradict document 5's "a vector is never a key of an index":
no vector is a key. What needs a decision is the new kind of index itself (*Product questions*, 1).

Included columns (document 3) are left out of the first version: each one would turn updates of that column into
maintenance work. Filters are evaluated on the rows at rerank. Partition columns are the exception, for tenants.

### Write path (foreground)

In the write transaction that changes a row, for each vector index of the table:

- insert or delete of a row: upsert `pending[pk]`;
- update: upsert `pending[pk]` only if the vector's bytes or a partition column differ between the old and the new
  row (a byte comparison; the update replaces the whole row and has both).

Nothing else: no assignment, no quantization, no access to `postings` or `locator`. The record is durable with the row
by construction, since it is in the same transaction. A bulk load (E13 of the schema PRD) follows the same rule: one
`pending` key per loaded row, in key order.

### Reconcile: the only step that writes the index

`reconcile(pk, G)`, for a set of generations `G`, runs in a worker's write transaction `W`:

1. Read the row at `W` (the latest committed state).
2. For each `g` in `G`: if `locator.g[pk]` exists, delete the `postings.g` entry it names and the locator entry; if
   the row exists, insert `f(row)` into `postings.g` and `locator.g`.
3. Delete `pending[pk]`, but only when `G` holds every generation that must reflect `pk`: the active one, and the
   building one when `pk ≤ cursor`. The backfill passes only the building generation, so it never deletes a pending
   key.

All in one transaction, so it either happens whole or not at all. The CPU work of `f(row)` is computed beforehand in a
short read transaction at a snapshot `S`, possibly by several threads; in `W` the worker compares the row's vector and
partition columns with those it used at `S` and applies the precomputed `f` when they match (when the row did not
exist at `S`, it must not exist at `W` either), and recomputes `f(row@W)` inside `W` otherwise, which is rare. Either
way, what `W` writes equals `f(row@W)`.

### The worker

A thread of the store, like the statistics thread of document 3. It loops:

1. Take up to `B` keys from `pending`, in key order, or, while a generation is building, the next `B` keys of the
   table after its cursor.
2. Compute `f` for each, in a short read transaction.
3. Apply them in one write transaction, sorted by `(list, primary key)` for locality: for pending keys,
   `reconcile(pk, G)` with `G` the active generation plus the building one when `pk ≤ cursor` (keys beyond the cursor
   with no active generation leave `pending` with nothing to apply); for backfill keys,
   `reconcile(pk, {building generation})`, then advance the cursor to the last key of the batch.
4. Commit in deferred mode. A crash may lose the worker's last commits, but only together with every later commit
   **[V]**, so their `pending` entries and the cursor are still there and the work is simply done again.

`B` is chosen so that a batch holds the write turn for a few milliseconds (*Costs*). Application writers wait at most
one batch, in order of arrival (G4).

Rules taken from document 3's statistics thread:

- **One lock order, the write turn last.** Steps 1 and 2 read a snapshot and hold nothing; step 3 takes the write turn
  like any writer and holds nothing else. The worker never waits for another job while it holds the turn.
- **Identity.** Step 3 drops its batch, writing nothing, if the table is no longer the same table (dropped and
  recreated) or the index no longer exists; it identifies them by id, not by name, so a rename changes nothing.
- **Closing and opening.** Closing the store stops the worker between batches. Unlike the statistics queue, nothing is
  lost: `pending`, the state and the cursor are in pages, and the worker resumes from them when the store opens.

### States and transitions

| From | Event | To | Written in one transaction |
|---|---|---|---|
| (none) | create index | `training` | definition |
| `training` | training done | `building(g=1, cursor=−∞)` | centroids and quantizer of `g` |
| `building(g)`, no active generation | cursor passes the last key | `ready(g)` | state |
| `ready(g)` | rebuild requested | `ready(g)` + `training` of `g+1` | — |
| `ready(g)` + `training` | training done | `ready(g)` + `building(g+1, cursor=−∞)` | centroids and quantizer of `g+1` |
| `ready(g)` + `building(g+1)` | cursor passes the last key | `ready(g+1)` | **publication**: active generation `g+1`; trees of `g` deleted |
| any building | cancel, or failure of the build | previous state | trees of the building generation deleted |

While a generation is building, the worker's `reconcile` for a pending key applies to the active generation, and to
the building one only when `pk ≤ cursor`; keys beyond the cursor are covered when the cursor reaches them. The initial
build, a rebuild and the publication are thus **the same protocol**: the only differences are whether an older
generation serves queries meanwhile (rebuild) or the planner uses the exact scan (initial build), and what the
publishing transaction removes. Publication is atomic: a query sees either `g` or `g+1`. Snapshots opened before it
keep reading `g`'s pages, which are freed when they close (MVCC, module 4).

Deleting the index, or the table, deletes every tree of the index, `pending` included, in that transaction, as for
any secondary index; the worker's next batch then finds the index gone and drops itself.

### Invariants

For every snapshot, for every primary key `pk`:

- **I-ready.** If generation `g` is active: `pk ∈ pending`, or the entry of `pk` in `postings.g` and `locator.g`
  equals `f(row)` (both absent when the row does not exist).
- **I-building.** If `g` is building with cursor `c`: the same, for every `pk ≤ c`. Nothing is claimed beyond `c`.
- **I-gen.** The codes and lists of `postings.g` were all computed with the quantizer and centroids of `g`; no
  generation mixes parameters.
- **I-pages.** Every page of a generation's trees is reachable from the table's catalog entry, or free.

Why I-ready holds: a row changes only in a transaction that also writes `pending[pk]`; `pending[pk]` disappears only
in a `reconcile` that, in the same transaction, sets the entry to `f(row)` at that moment. So when `pk ∉ pending`, the
row has not changed since its entry was last set to `f(row)`. I-building holds the same way, adding that the backfill
sets `f(row)` for every key it passes and the worker keeps every `pk ≤ c` current; publication happens when `c` passes
the last key, so at that moment I-building is I-ready.

The scenarios the review raised (R1-1):

| Scenario | What happens |
|---|---|
| A row is updated after training, before the backfill reaches it | Its key is in `pending` and beyond the cursor; the backfill later reads it at `W` and writes `f(row@W)`. |
| A row changes list during the backfill | The batch's precomputed `f` no longer matches at `W`, so `f(row@W)` is recomputed and written; or the change commits after `W` and puts the key back in `pending`. |
| A row is deleted while its entry is being built | At `W` the row does not exist: the locator's entry, if any, and its posting are deleted; nothing is inserted. |
| A transaction inserts an entry that the backfill then inserts again | Transactions never write `postings`. Both paths go through `reconcile`, which deletes the old entry through the locator before inserting: there is never a duplicate. |
| The process ends during the build | Everything is in pages: the state, the cursor, `pending` and the trees recover as a prefix of the commits; the worker resumes from the cursor. No page is orphaned (I-pages). |
| A transaction modifies a row whose old entry the index does not yet hold | It only writes `pending[pk]`; `reconcile` handles a missing locator entry as "nothing to delete". |

### Queries and their guarantees

Document 5 expected that an index would need no change to the query tree, since `Distance < t` and
`OrderBy(Distance)` are what an index accelerates. That holds for an exact index, not for an approximate one: serving
`OrderBy(Distance)` with `Limit(k)` from IVF would change its result, against N1. So approximate search is a node of
its own, which the caller asks for:

`ApproxNearest(column, q, k, nprobe | target)` at a snapshot `S`:

- If no generation is `ready` at `S`, the planner runs the exact top-k (document 5).
- Otherwise:
  1. Probe the `nprobe` nearest lists of the active generation `g` at `S`; keep the best `k·r` candidates by SQ8
     distance, **skipping keys present in `pending` at `S`**, whose entries may be stale.
  2. Add every key in `pending` at `S` whose row exists at `S`.
  3. Read each candidate's row at `S` (`KeyLookup`), evaluate the filters on the row, compute the exact distance with
     document 5's kernels, and keep the top `k`, ties by primary key.

The SQ8 distance of step 1 uses a fixed-order kernel like document 5's, so the candidate set, and with it the result,
is the same on every platform for the same data and parameters.

Guarantees, for any open snapshot, older ones included (the index, `pending` and the rows are versioned together):

- **G1 (N4).** Every row returned is visible at `S`, satisfies the filters at `S`, and carries its exact distance at
  `S`.
- **G2 (freshness).** A row visible at `S` can be missed only through the approximation (partitioning and
  quantization), never through the index's lag, because step 2 covers exactly what the index has not reconciled.
- **G3 (N1).** Exact queries never use any of this.

This is more than A1 requires ("may miss recent vectors"); its price is step 2, which reads `|pending|` rows. The
backlog's size is known at once (A5), so the planner can cost it. When the worker keeps up, the backlog is small. The
behaviour beyond a cap is a decision (*Product questions*, 2): scan it anyway, or skip step 2 and mark the result as
possibly stale, which still satisfies A1 and G1.

**Using the index while it is being updated.** The latest committed generation is usable in all but one case:

| Situation | What a query uses | What it sees |
|---|---|---|
| Normal maintenance (the worker applying batches) | The active generation as of its snapshot `S` | The worker's uncommitted batches are invisible, and committed deferred ones too until they are durable. Entries of keys in `pending` are skipped and those rows evaluated exactly (step 2), so the lag does not show. |
| Rebuild (`g` active, `g+1` building) | Generation `g`, which the worker keeps maintaining | `g` until the transaction that publishes `g+1`; never a mix of both. Queries already open keep reading `g` until they end. |
| Initial build (no generation ready) | The exact scan | Querying the partial generation (its entries up to the cursor, plus an exact scan of the rows beyond it) would scan nearly the whole table for most of the build, so it is not offered. |

What "lagging" means depends on question 2:

- **Pending rows evaluated** (recommended): the lag does not show; the result is approximate only through the
  partitioning and the quantization. Cost: reading as many rows as are pending.
- **Pending rows skipped beyond a cap:** rows changed since the worker's last pass are missing, neither with their old
  version nor with the new one. No extra cost.

Either way G1 holds: every row returned is read again and checked at `S`.

### Training without long snapshots

- **Sample** in short read transactions (for example per key range), so that no snapshot stays open for minutes and
  keeps page versions alive. The sample lives in memory: about 80 MB for 25,600 × 768-dim `f32`, 150 MB for 50,000,
  480 MB for 158,000 **[K]**. A training memory budget is a decision (*Product questions*, 5).
- **k-means** with a fixed seed, in memory, with no transaction open. It need not be reproducible across platforms:
  its result is stored. It may use a blocked matrix multiplication, faster and without a fixed order.
- **Quantizer** from the same sample.
- Enough rows to train: at least about 39·nlist points (Faiss's heuristic **[L]**); until then the index stays in
  `training` and queries use the exact scan.

### SQ8 quantizer and drift

- **Parameters:** per dimension, a range `[lo, hi]` taken from the sample and widened by a margin; the margin's value
  is to be measured **[M]**.
- **Out of range:** a component beyond the range is clamped to code 0 or 255 (saturation). It changes only which rows
  become candidates; the rerank reads the stored vector, so returned distances are never affected.
- **Detection:** the worker keeps, per generation, the number of codes written, of codes with at least one saturated
  component, and of saturated components; and the list sizes (largest list against the mean). An explicit operation
  estimates recall by comparing `ApproxNearest` with the exact top-k on a few sample vectors (a full scan each, so on
  demand, not automatic).
- **Rebuild criteria:** saturated codes above a fraction, list imbalance above a ratio, or estimated recall below a
  target. The thresholds come from the benchmarks; the first version reports them and recommends a rebuild, and the
  rebuild is explicit.
- **Versioning:** centroids, quantizer, `postings` and `locator` belong to one generation (I-gen); a rebuild computes
  everything again for `g+1` and publishes it atomically.

### Filters and the planner

- The planner estimates the rows `F = s·N` that pass the filters, from the histograms of document 3.
- **`F` below a crossover `C`** (measured; roughly 50,000 to 100,000 at 768-dim `f32` **[K][M]**): exact distance over
  the pre-filtered rows, as in document 5. Exact, and usually cheaper.
- **Otherwise:** IVF with `nprobe' = min(nlist, nprobe / s)` and `k·r / s` candidates read at rerank, so that the
  expected number of candidates that pass stays the same. Getting k rows that pass does not prove good recall: with a
  filter correlated with the clusters, `s` is wrong inside the probed lists. The benchmarks measure correlated
  filters, not only random ones.
- **Tenants:** an index may declare **partition columns** (for example `TenantId`) as the leading part of its key,
  `tuple(TenantId, list_id) ‖ pk`. An equality on them then probes only that tenant's part of each list, and the
  crossover uses the tenant's row count. Centroids stay shared by all tenants in the first version.
- Cost of `ApproxNearest` for the planner: `nlist·dim + nprobe·(N/nlist)·code_bytes + (k·r + |pending|)·lookups`.
- The explanation prints the generation used, `nprobe`, the candidates read and the backlog evaluated, as document 5
  asks for rows scanned and bytes streamed.

### Online build against an exclusive build

Building the first version of the index while blocking writes is allowed by the review, but under A1 it would be the
more complex choice:

- the worker and `reconcile` are needed anyway, to maintain the index;
- the online backfill adds only a cursor and one rule (apply to the building generation when `pk ≤ cursor`);
- an exclusive build would be a second way of writing the index, and would hold the write turn for about 5 to 20 s per
  million rows (*Costs*), blocking every writer of the store.

The exclusive build remains a valid fallback if the online one shows a defect in testing.

## Costs

### Foreground, per changed row and per vector index **[K][M]**

- One byte comparison of the vector and partition columns, on updates only.
- One upsert into `pending`, a small tree:
  - sequential keys (`ulid`, identity): the rightmost leaf, copied once per transaction and shared by every row of the
    transaction;
  - random keys (`uuid` v4): one leaf, plus a branch level or two, since the tree stays small.
- Journal: the key, a few tens of bytes with the page-diff journal; a whole page per touched page until it lands.
- Several vector indexes on one table: one upsert per index whose columns changed.

Against v1's synchronous design, this saves the assignment and the writes to the two large trees inside the
application's transaction.

### Background, per change **[K][M]**

Document 5's kernels accumulate in `f64`; the figures below take 2.5 to 5 G multiply-adds per second per core for
them, half the `f32` rate (document 5 estimates up to about twice the arithmetic cost).

| Part | Cost | Where |
|---|---|---|
| Assignment | nlist × dim multiply-adds: ~0.15–0.3 ms at nlist 1,000, ~0.5–1 ms at nlist 3,162 (768 dims) | Read transaction, parallel |
| Quantization | dim operations, negligible | Read transaction |
| Tree operations | read the row, locator lookup, delete and insert in `postings`, locator upsert, delete from `pending`: about 5 | Write turn |
| Pages copied | `postings`: a random leaf, two when the row changes list; `locator`: a random leaf unless keys are sequential; branch pages shared within the batch. About 2 to 4 per change when changes are scattered, fewer when they cluster | Write turn |
| Journal | about the code's size plus keys (~0.8–1 KB at 768 dims) with page diffs | Write turn |
| Write turn | a batch of 1,000 changes, about 5–15 ms | Write turn |

For HNSW, by comparison: the search (2,000 to 5,000 distances) can run outside the write turn, because the worker is
the only writer of the graph, but about 30 to 40 random pages per insert are still copied inside it.

**Backlog.** The worker keeps up while it applies changes faster than they arrive: one core of assignment covers
about 1,000 to 6,000 changes per second, depending on nlist **[M]**. Beyond that the backlog grows and queries pay it.
The first version applies no back-pressure on writers; it reports the backlog's size.

### Training and building **[K][M]**

Assumptions: 768-dim `f32`; k-means at the `f32` rate of a blocked multiplication, 5 to 10 G multiply-adds per second
per core, 10 iterations; the assignment with document 5's kernels at 2.5 to 5 G; nlist by pgvector's rule (rows/1000
up to 1 M rows, √rows above) **[L]**; a sample of 50 to 256 points per list.

| Rows | nlist | Sample | k-means, 1 thread | Assign every row, 1 thread | Both on 8 threads (×5–7) | Apply in the write turn (serial) |
|---|---|---|---|---|---|---|
| 100,000 | 100 | 25,600 | 2–4 s | 1.5–3 s | ~1 s | ~0.5–2 s |
| 1 M | 1,000 | 50,000 | 40–80 s | 2.5–5 min | ~30–75 s | ~5–20 s |
| 10 M | 3,162 | 158,000 | 6–13 min | 80–160 min | ~13–35 min | ~1–3 min |

- Eight threads give ×5 to ×7, not ×8: the assignment streams every vector once (about 3 GB per million rows), which
  is bounded by the disk when the rows are not cached; threads share caches and memory bandwidth.
- The serial part is the worker's write transactions. It is spread over many short batches, interleaved with the
  application's writes, so it delays each of them by at most one batch.
- Levers: a two-level assignment (√nlist coarse centroids, then the fine ones), which cuts the assignment by 10 to 30
  times at 10 M rows for some recall **[M]**; a faster `f32` kernel, if document 5's measurement adds one; a smaller
  sample.

**HNSW, for comparison:** no training, but building 1 M vectors costs the same order or more, and all of its page
writes go through the write turn **[K][M]**.

### MVCC retention

Only short read transactions are opened (sampling, assignment per batch), so neither training nor the build keeps old
page versions alive for long. What remains: a publication or a deleted generation frees its pages only after the
snapshots that still read them close, so during a rebuild the store holds both generations' pages.

### Search latency

For 1 M × 768-dim `f32`, from document 5's estimates **[K][M]**:

| | Approximate latency | What it represents |
|---|---|---|
| Exact scan | ≈ 0.3 s, up to twice that | Baseline |
| IVF-SQ8 | ≈ 5–12 ms | Probes 2–4 % of the lists, about 15–30 MB |
| HNSW, warm cache | ≈ 1–2 ms | — |

Measured as **latency reduction against the exact scan**, IVF keeps about 97 % of the benefit; measured as **QPS**,
10–30 % of HNSW's. IVF-SQ8 is the balanced choice for interactive latency, not for a service with many queries per
second.

Measured on 2026-11-24, in memory, one thread, at base clock (*Measured result*):

- Exact scan: 0.92 s.
- IVF-SQ8: 17.2 ms with 4 % of the lists for L2, and 20.9 ms with 7.2 % for the dot product.
- External HNSW: 11.2 ms for L2 and 5.9 ms for the dot product.

## Risks

| Risk | Consequence | Mitigation |
|---|---|---|
| Backlog after a bulk load or a burst of writes | `ApproxNearest` reads every pending row: after loading 1 M rows it costs an exact scan until the worker drains them | The planner costs `|pending|` and chooses the exact scan when it is cheaper; the cap of question 2; the backlog is reported |
| The worker cannot keep up with the write rate | The backlog grows without bound | Several assignment threads, a smaller nlist, a two-level assignment; back-pressure is left for later |
| The worker delays application writers | Each batch holds the write turn | `B` bounded by time (question 5); the turn is served in order of arrival, so a writer waits at most one batch |
| Recall under filters correlated with the clusters | `ApproxNearest` returns k rows with poor recall | The exact scan below the crossover; partition columns for tenants; correlated filters in the benchmarks |
| Drift of the data after training | Saturated codes, unbalanced lists, lower recall | Counters per generation, an explicit recall estimate, an explicit rebuild |
| Dot product over unnormalized vectors | IVF partitions by L2; recall may be poor | Measured with its own data set; a graph if it fails (*When the recommendation would change*) |
| A defect in the online build | A generation published with missing or stale entries | I-ready and I-building checked by tests under concurrent writes and crashes; an integrity check compares `postings` with `f(row)`; the exclusive build as a fallback |
| Space during a rebuild | Two generations live at once | Reported before a rebuild starts; the old generation is freed when its last snapshot closes |
| `f64` kernels in the assignment | About twice the CPU of `f32` lanes | Measured; the levers of *Training and building* |

## Benchmarks

**Data:** SIFT1M (128 dimensions, L2); GloVe-100 (angular, hard for IVF); a set of modern embeddings of 768 or 1,536
dimensions; a set of unnormalized vectors for the dot product. Scales of 10,000, 100,000 and 1 M, plus 10 M with data
that does not fit in RAM. **Ground truth:** Krummi's own exact scan.

**Order of the experiments:**

1. The approximate SQ8 scan with rerank, computed in memory in the benchmark tool: recall lost by quantization alone.
2. IVF-Flat and IVF-SQ8: recall lost by partitioning, and by both.
3. An external in-memory HNSW, **in the benchmark tool only**, to measure the gap.

**Search:**

| Measures | Detail |
|---|---|
| recall@10 and @100 | Against the exact ground truth |
| QPS–recall curve | Sweeping nprobe |
| Latency | p50, p95 and p99, cold and warm cache |
| Memory | RSS plus the working set in the page cache |
| Index size | Pages per tree |
| Filters | Random filters at selectivities of 50 %, 10 %, 1 % and 0.1 %; **filters correlated with the clusters**; a **multi-tenant** set with skewed tenants, with and without partition columns; the crossover with the exact scan |
| Drift | Recall after 20 % and 50 % churn without rebuilding; recall and saturation counters after inserting vectors outside the trained ranges |

**Correctness and durability:**

- Inserts, updates and deletes concurrent with the backfill and with a rebuild, checked against I-ready and
  I-building.
- The process killed at each phase (training, building, publication, during a worker batch, during an application
  commit), then reopened: the index matches the committed rows (I-ready, after the worker drains `pending`), no stale
  entries, every page used or free.
- Publication: no query sees two generations; old snapshots keep reading theirs.
- Queries at old snapshots: G1 and G2 hold.
- The table or the index deleted, and the table recreated under the same name, while a worker batch is in flight: the
  batch writes nothing.

**Operational cost:**

- p95 and p99 latency of write transactions with and without the index, and with 1, 2 and 4 vector indexes per table.
- Longest time the worker holds the write turn, during maintenance and during a build.
- Write amplification: bytes journaled and pages copied per change, foreground and background separately.
- Total build time, memory used, and the growth of retained versions (scratch area).
- Backlog: worker throughput against write rate; backlog size and query latency when it grows.

**Thresholds** (hypotheses until reproducible measurements exist **[K]**), at 1 M × 768:

- recall@10 ≥ 0.95 with a p95 ≤ 20 ms, one thread, warm cache, backlog drained;
- a write transaction's p95 with the index within 10 % of without it;
- the worker holds the write turn ≤ 15 ms per batch.

### Measured result (2026-11-24)

Steps 1 to 3 ran in memory with `krummi-ann-lab`, on Windows and Linux; see the
[report](../../benchmark/2026-11-24-ann-experiment.md). Setup: one thread, warm cache, Windows on the P cores with
boost off (base clock).

| Data set | First nprobe with recall@10 ≥ 0.95 | % of the lists | p95 | External HNSW (M 16) at recall@10 ≥ 0.95 |
|---|---|---|---|---|
| Cohere 1 M × 768, L2 | 40 | 4.0 | 17.20 ms | ef 320, p95 11.19 ms |
| Cohere 1 M × 768, dot product, unnormalized | 72 | 7.2 | 20.90 ms | ef 80, p95 5.89 ms |
| GloVe 1.19 M × 100, cosine | 160 | 14.7 | 6.18 ms | not reached: 0.9175 at ef 640, p95 19.24 ms |
| SIFT 1 M × 128, L2 | 32 | 3.2 | 2.49 ms | not measured |

- **Step 1 (quantization):** quantization alone loses almost nothing. The SQ8 scan with a rerank of 10·r candidates
  gives recall@10 ≥ 0.978 at r = 1 and 1.0000 at r = 2 on every data set at 1 M.
- **Step 2 (partitioning):** IVF-SQ8 with r = 2 has IVF-Flat's recall in every row, at about 1/3.5 of its p95, so
  all the loss comes from partitioning. The 1 M IVF-SQ8 index takes 739 MiB, against 2936 MiB for IVF-Flat.
- **Thresholds:** L2 at 1 M × 768, the reference case, meets them. Two switch criteria fire narrowly:
  - the dot product misses the 20 ms p95 by 0.9 ms at base clock (boost makes full passes 1.7–1.8 times faster at
    10,000 rows; it was not measured at 1 M);
  - GloVe needs 14.7 % of the lists, at a p95 far under 20 ms.
- **The graph:** the external HNSW is 3.5 times faster than IVF-SQ8 for the dot product and 1.5 times faster for L2,
  a narrower gap than the estimates above suggested (IVF-SQ8 5–12 ms, HNSW 1–2 ms). It does not reach 0.95 on GloVe with M 16. It took 1169 to 1791 s to build
  in parallel, against 579 to 773 s for IVF.
- **Determinism:** the 12 result digests match bit for bit between Windows and Linux.
- **Memory** (Linux, 100,000 rows, RSS growth while building). The `hnsw_rs` graph takes about 2.7–2.8 KiB per vector
  beyond the vectors, against 128 bytes for compact 4-byte links. IVF grows to about twice its computed size while it
  builds, and keeps its computed size afterwards.

## When the recommendation would change

| If this happens | Switch to |
|---|---|
| IVF-SQ8 needs more than ~10 % of the lists to reach 0.95 recall, or cannot get under a 20 ms p95 at 1 M | HNSW (the fallback) |
| The load is many queries (thousands of QPS, under 5 ms), few writes, and the index fits in RAM | HNSW |
| More than 10 to 100 M vectors per table with little RAM | IVF-PQ or RaBitQ with a large nlist; then a graph over the centroids (a light SPANN) or DiskANN |
| Tables usually hold fewer than about 100,000 vectors, or filters are always selective | **No index**: the exact scan |
| The dot product on unnormalized vectors dominates and IVF gives poor recall | A graph |
| The worker cannot keep up with the write rate even with several assignment threads | A smaller nlist, a two-level assignment, or back-pressure on writers |
| Modules 13 to 16 get built (Voron parity) | Voron-style HNSW becomes cheaper; re-evaluate |

## Plan

**First:** the benchmark experiment (*Benchmarks*, steps 1 and 2), with no change to the product.

**Then, if the thresholds hold: IVF-SQ8 as a new kind of index, maintained asynchronously,** in PRD 7.6: the
`pending` tree in the write path, the worker with `reconcile`, generations with an online build and atomic
publication, the opt-in `ApproxNearest` node, and the exact scan when the filter is selective or no generation is
ready.

**Left for later:** included columns; per-tenant centroids; automatic rebuilds; back-pressure on writers; PQ and
RaBitQ; splitting lists locally, LIRE-style; parallelism within a query; an approximate distance threshold; HNSW
unless measurements require it; DiskANN and SPANN as complete systems.

## Product questions

All open; each needs the user's approval before the PRD.

1. **A new kind of index.** An IVF vector index maintained asynchronously (A1), and the opt-in `ApproxNearest` query
   node, in PRD 7.6 (in scope since 2026-11-10). Document 5's "a vector is never a key of an index" stays true; the new kind of index is
   what needs the decision.
2. **The backlog in queries.** Recommendation: queries read the backlog exactly (fresh results, cost in proportion to
   the backlog). Open: what happens beyond a cap, scanning it anyway, or skipping it and marking the result as
   possibly stale.
3. **Partition columns** for tenants in the first version; included columns left out.
4. **The online build** through the worker, instead of an exclusive build.
5. **The worker's resource policy:** batch size or the longest time it may hold the write turn, threads for training
   and assignment, and the training memory budget.
6. **Rebuilds explicit** in the first version, with metrics that recommend them.

## Review history

The external review of v1 (findings relayed by the user on 2026-11-10) and how v2 handled each one:

| # | Finding | Verdict | Resolution in this document |
|---|---|---|---|
| R1-1 | The incremental build had no precise protocol | **Defect confirmed**: "each batch checks that the row still exists with the same vector" defined no persistent states, no recovery and no invariant per state | *States and transitions*, *Reconcile*, *Invariants* with the review's scenarios one by one, recovery; the exclusive build weighed in *Online build against an exclusive build* |
| R1-2 | An SQ8 scan with rerank is not exact | **Defect confirmed** (naming): v1 called it "exact scan over SQ8 codes" | Renamed **approximate SQ8 scan**; measured first, to separate quantization error from partitioning error |
| R1-3 | Maintenance cost; "1 to 3 pages per row" unjustified | **Defect confirmed**: stated without derivation; it ignored the pages on the path and splits | *Costs* breaks the cost down. Updates that touch neither the vector nor the partition columns record nothing |
| R1-4 | SQ8 values outside the trained range | **Gap confirmed** | *SQ8 quantizer and drift*: saturation, counters, rebuild criteria, versioning |
| R1-5 | Determinism, build against rebuild, MVCC retention | **Partly confirmed**: v1 described an online build and called the online rebuild "for later"; it ignored the versions a long snapshot keeps alive | One protocol with generations; only short read transactions; a locator tree, so correctness no longer depends on recomputing an assignment |
| R1-6 | Filtered queries and recall | **Gap confirmed**: widening `nprobe` until k rows pass does not ensure recall | *Filters and the planner*: planner rule, partition columns for tenants, correlated filters in the benchmarks |
| R1-7 | Duplicated SQ8 codes per row | **Defect confirmed**: a per-row SQ8 code before IVF would be stored twice | Dropped: the approximate SQ8 scan is a benchmark experiment computed in memory; SQ8 codes exist only inside the IVF index |
| R1-8a | Eight threads assumed almost ideal scaling | **Confirmed** | ×5 to ×7, plus a serial part in the write turn **[M]** |
| R1-8b | Can a read transaction be used by several threads? | **Already supported [V]** (AC-A20) | No change |
| R1-8c | The exact scan scores 76, not 79, under v1's weights | **Confirmed**: 15·1 + 10·3 + 15·5 + 15·5 + 15·5 + 10·1 + 10·5 + 10·5 = 380 → 76 | Corrected |
| R1-8d | `list_id ‖ primary key` does not contradict "a vector is never an index key" | **Confirmed** | Reworded: only the new kind of index needs a decision |
| R1-9 | Correctness, durability and operational benchmarks | **Accepted** | *Benchmarks* |

Changes made when this document moved into the design set: costs of the assignment recomputed for document 5's `f64`
kernels (its R3, later than the evaluation); the worker given the rules of document 3's statistics thread (lock order,
identity, closing); deletion of the index and of the table, bulk loads, the SQ8 kernel's determinism and the risks
table added.

## Sources

Repository files read:

- `README.md`; `docs/prd/krummi/krummi.prd.en.md` (modules, G4); `docs/prd/krummi/journal/journal-page-diffs.prd.en.md`;
  `docs/prd/krummi/journal/journal-group-commit.prd.en.md` (E5, AC-J7)
- `docs/prd/krummi/tables/tables.prd.en.md` (out of scope: vector indexes); `docs/prd/krummi/tables/tables-schema.prd.en.md` (E2,
  E13; internal trees out of the catalog)
- `docs/design/tables/README.md`, [05-vectors.md](05-vectors.md) (whole, including R3); [03-keys-and-indexes.md](03-keys-and-indexes.md)
  (statistics thread, index creation); [04-query-tree-and-planner.md](04-query-tree-and-planner.md) and
  [08-execution-plans.md](08-execution-plans.md) (searched for vector and index references)
- `crates/krummi-tree/src/node.rs`, `fixed.rs`, `multi.rs`; `crates/krummi-pager/src/format.rs` (constants)
- `crates/krummi/src/lib.rs` (`commit_deferred`); `crates/krummi/tests/ui/trees_pass/read_tree_handle_crosses_threads.rs`
  (AC-A20)
- `C:\projects\ravendb\src\Voron\Data\Graphs\Hnsw.cs`, `Hnsw.Options.cs`, `Hnsw.Constants.cs`;
  `C:\projects\ravendb\src\Corax\Mappings\VectorOptions.cs`

Literature (cited from memory, not re-fetched on 2026-11-10):

- Malkov and Yashunin, "Efficient and robust approximate nearest neighbor search using Hierarchical Navigable Small
  World graphs", TPAMI 2018 (arXiv 2016).
- Xiao et al., "Enhancing HNSW Index for Real-Time Updates: Addressing Unreachable Points and Performance
  Degradation", arXiv 2024.
- Patel et al., "ACORN: Performant and Predicate-Agnostic Search Over Vector Embeddings and Structured Data",
  SIGMOD 2024.
- Subramanya et al., "DiskANN: Fast Accurate Billion-point Nearest Neighbor Search on a Single Node", NeurIPS 2019.
- Singh et al., "FreshDiskANN: A Fast and Accurate Graph-Based ANN Index for Streaming Similarity Search", arXiv 2021.
- Gollapudi et al., "Filtered-DiskANN: Graph Algorithms for Approximate Nearest Neighbor Search with Filters",
  WWW 2023.
- Jégou, Douze and Schmid, "Product Quantization for Nearest Neighbor Search", TPAMI 2011.
- Johnson, Douze and Jégou, "Billion-scale similarity search with GPUs" (Faiss), 2017; Douze et al., "The Faiss
  library", 2024.
- Chen et al., "SPANN: Highly-efficient Billion-scale Approximate Nearest Neighbor Search", NeurIPS 2021.
- Xu et al., "SPFresh: Incremental In-Place Update for Billion-Scale Vector Search", SOSP 2023.
- Gao and Long, "RaBitQ: Quantizing High-Dimensional Vectors with a Theoretical Error Bound for Approximate Nearest
  Neighbor Search", SIGMOD 2024.
- Aumüller, Bernhardsson and Faithfull, "ANN-Benchmarks", Information Systems 2020.
- pgvector README (HNSW and IVFFlat, iterative index scans in 0.8.0, vacuuming of HNSW indexes):
  <https://github.com/pgvector/pgvector>.
