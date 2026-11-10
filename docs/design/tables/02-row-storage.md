# Tables: where rows live

Technical design for module 7 (tables), document 2 of the [tables design set](README.md). It compares three places to
keep a row (Voron's raw data sections, a table clustered by primary key, a heap of rows keyed by a row id) and
records the one chosen, in terms of Krummi's existing trees. The row encoding is document 1; key encoding and index
definitions are document 3. Reconciled 2026-11-09 with the user's decisions and the [fact-check](verification.md).

## Summary

- Decided (2026-11-09): a table is **clustered by its primary key**, as in SQL Server. The row is the value of a
  normal tree whose key is the encoded primary key; the primary key is mandatory; each secondary index is a normal tree
  keyed by `columns ‖ primary key`. All existing tree types, no new page format, no row id, no heap.
  User: "Las claves primarias rara vez cambian y suelen ser artificiales como autonuméricos o ULID o GUID. Así que
  quizá mejor tener índices agrupados para la clave primaria como en SQL Server" and "Hacemos la clave primaria
  obligatoria."
- The user's argument removes the clustered table's two classic costs. A primary key that rarely changes seldom
  rewrites the row and every secondary entry. An artificial key (auto-increment, ULID, sequential GUID) is increasing,
  and Krummi's append split keeps the leaves of increasing keys full, so inserts append instead of splitting half-full
  leaves.
- The heap of the first draft (rows keyed by a hidden row id, a `pk` tree from key to row id) is kept below as the
  rejected option and as the reference for what clustering costs.
- Costs that remain: every secondary entry repeats the primary key; a secondary lookup is two descents; a random
  primary key (UUID v4) spreads inserts over the leaves that hold the rows. The last one is a risk to measure, not a
  design change.

## Requirements

From the root PRD, the trees PRDs and the [performance plan](../../performance/plan.md):

1. **No new page format.** Use normal, multi-value and fixed-size trees as built (B3). Rows can be 0 to 256 MiB (B6).
   A leaf cell has a 4-byte header, the key and the value, and at most 4,078 bytes (`MAX_LEAF_CELL`), so a value over
   4,074 bytes minus its key lives off its leaf in a contiguous run (`fits_inline`: `4 + key + value <= 4078`;
   large-values execution D7).
2. **Zero-copy reads** (D11): a row read returns a reference to engine memory valid for the read transaction. Inline
   rows point into the leaf; off-leaf rows are contiguous (V2).
3. **Cheap writes.** Pages changed per operation drive the journal (page diffs: only changed 8-byte words, O8) and the
   copy of each page on its first change (T14). Fewer distinct dirty pages and fewer splits are better; the plan shows
   commit cost (journal, copies, checksums) is more than half of write time (55.5 % in W2, 59.4 % in W21).
4. **Snapshot safety.** Readers see a consistent version; a changed page keeps its number while the old version
   lives in the versions area (T14), so no parent page is rewritten when a child changes ("no path is copied",
   `crates/krummi-tree/src/lib.rs:13`).
5. **Space** follows the free-space map (F2–F6): single pages are reused after the freeing commit, runs need contiguous
   pages (F5, V6). No density heuristics or preallocated areas.
6. **Keys of any shape**, including 16-byte ULID/UUID that are time-ordered or random, and composite keys up to 2,048
   bytes (B5), without making the time-ordered case pathological.
7. **No later redesign** for the planner (module 17): stable locators, cheap index maintenance, predictable descent
   counts for a cost model.

## Voron's raw data sections, precisely

**Layout.** A section is a header page plus N data pages allocated contiguously. N is 32 pages while the allocated
pages are at most 4,096 (`GetNumberOfPagesInSmallSection`; 32 MiB at 8 KiB pages, though the code comment says
16 MB), then 128, 512 and 1,024, and `(PageSize - 96) / 2 = 4,048` above 32,768 pages
(`ActiveRawDataSmallSection.cs:249-266`); each new section doubles the previous one, capped at 32 MB (1 MB on 32-bit)
(`Table.cs:1186-1190`). The header page
(`RawDataSmallSectionPageHeader`) holds the entry count, allocated bytes, last used page, the owner hash and, from byte
96, one `ushort` of available space per data page (`RawDataSection.cs:16`, `83-90`). Each data page has a header with
`NumberOfEntries`, `NextAllocation` (a bump pointer), its index in the section and the owner hash
(`RawDataSmallPageHeader.cs`). An entry is 4 bytes of sizes (`AllocatedSize`, `UsedSize` with bit 15 for "compressed",
`-1` for freed) followed by the row (`RawDataSection.cs:25-62`).

**Ids.** `id = pageNumber * pageSize + offsetInPage` (`ActiveRawDataSmallSection.cs:53`). The id is a physical address:
reading it is a page fetch and an offset, no tree descent (`RawDataSection.cs:231-265`). A row of
`(pageSize - header) / 2` bytes or more is not in a section: it goes to its own overflow pages and its id is
`firstPage * pageSize`, so an id with offset 0 means overflow (`RawDataSection.cs:21`, `Table.cs:728-748`, `769-783`).

**Allocation.** `TryAllocate` scans from the last used page for one with enough available space and a bump pointer
that fits, and bumps (`:42-65`). If none, it scans from the start for a page with enough total space, compacts that
page (`DefragPage`) and allocates at the end (`:70-89`). Otherwise the table retires the section (`InactiveSections`),
tries a section from the `ActiveCandidateSection` tree, or creates a new one (`Table.cs:786-796`, `1186-1230`). Every
allocation modifies the section header page (`EnsureHeaderModified`, `:59`), and every insert rewrites the table
statistics entry (`Table.cs:756-764`).

**Update.** If the new row fits in the entry's `AllocatedSize`, it is written in place and the id stays
(`RawDataSection.cs:187-224`, `Table.cs:381-399`); the allocated size is never reduced. An overflow row stays in place if
its page count is unchanged (`Table.cs:404-437`). Otherwise `Update` is a delete plus an insert with a new id
(`Table.cs:439-446`), and `UpdateValuesFromIndex` is not used: every index entry for the old id is deleted and
re-added (`Table.cs:1097-1148`). Indexes store the id, so index maintenance on a changed key walks the primary key
tree, each secondary index and each fixed-size index (`Table.cs:927-1000`).

**Delete.** `Free` marks the entry freed, zeroes its bytes and returns the bytes to `AvailableSpace`
(`RawDataSection.cs:303-348`). Space inside a page comes back only when a later allocation compacts the page.

**Moves and fragmentation.** `DefragPage` rewrites a page, giving surviving rows new offsets and raising
`DataMoved(previousId, newId, data)` for each (`ActiveRawDataSmallSection.cs:124-195`; delegate in
`DataMovedDelegate.cs`). `Table.OnDataMoved` deletes the old id from every index and inserts the new one
(`Table.cs:110-125`). After a delete, a section with density above 0.5 stays; between 0.15 and 0.5 it becomes a
candidate for reuse; below 0.15 `ReleaseNearlyEmptySection` moves every surviving row to the active section
(re-indexing each) and frees the section's pages (`Table.cs:575-637`). A delete or an insert can thus trigger
a burst of index updates proportional to a page or a whole section.

**Indexes.** The primary key tree maps key to id; a secondary index maps each indexed value to a fixed-size tree of
ids (`Table.cs:1106-1118`).

## Options compared

All numbers are for 8 KiB pages, a table with a primary key and *k* secondary indexes. "Pages" counts distinct pages
changed by one operation, excluding splits and merges and the shared branch pages. Estimates are marked and need
measuring in the PRD's execution.

| Criterion | A. Voron raw data (rejected) | B. Clustered by primary key (**decided**) | C. Heap by row id (rejected) |
|---|---|---|---|
| Insert, pages | data page, section header, statistics, PK leaf, *k* index pages (4 + *k*) | 1 + *k* | row leaf (shared by many inserts when appended) + PK leaf + *k* = 2 + *k* |
| Update, same size or smaller, no indexed column changed | 1 | 1 | 1 |
| Update, row grows | delete+insert, new id: data pages, header, statistics, and PK and all *k* indexes rewritten | 1 leaf, maybe a split; indexes untouched unless their key changed | same as B |
| Update of the primary key | PK leaf only (id unchanged) | delete+insert of the row and every secondary entry (they hold the PK): 2 + *k* | PK leaf only |
| Delete, pages | data page, header, statistics, PK, *k* | 1 + *k* | 2 + *k* |
| Descents, PK lookup | 1 tree + direct page jump | 1 | 2 |
| Descents, secondary lookup | 1 tree + direct page jump | 2 | 2 |
| Range scan in PK order | PK walk, then random page jumps | sequential leaves | PK walk, then row-tree descents (sequential if ids correlate) |
| Locator size in indexes | 8 bytes | PK, 16 to 2,048 bytes | 8 bytes |
| Rows move, indexes rewritten | on defrag and on section evacuation | never (the key is the row) | never |
| Row id visible | physical, changes on grow | none | stable if kept |
| Large rows | overflow pages, whole pages | existing off-leaf runs | existing off-leaf runs |
| Space reuse | density thresholds, compaction, whole-section release | tree merge at a quarter, page-level reuse (F6) | same as B |
| Per-row overhead in leaves (4-byte cell header + 2-byte slot per cell, key included) | 4-byte entry sizes in the section plus the PK tree cell | 6 + PK | 28 + PK (rows tree 6 + 8, `pk` tree 6 + PK + 8) |
| New machinery in Krummi | sections, allocator, density logic, move callback | none | row id counter |

**A. Raw data sections** give the cheapest reads (no descent for the row) and in-place updates. They bring a
separate allocator with sections of contiguous pages, density thresholds, compaction and a move callback that
re-indexes rows. They duplicate what the free-space map and the trees already do, they change ids on growth, and
their per-insert header and statistics pages add dirty pages in every transaction. A physical id also cannot be
kept in Krummi's trees: a leaf cell has no fixed address, because cells shift within a page and cross pages on a
split (`crates/krummi-tree/src/node.rs:1-25`). Rejected.

**B. Clustered** (SQLite WITHOUT ROWID, InnoDB, SQL Server) has the fewest structures: the primary key tree is the
table. It wins primary key lookups and range scans and takes the least space. Its costs show with the keys users want
to include (ULID, UUID, composite keys): secondary indexes carry the whole primary key, so a 16-byte UUID or a
100-byte composite key repeats in every entry of every index, and a primary key change rewrites every index entry. A
random UUID spreads inserts over every leaf of a tree that holds the rows, not only the keys: with 500-byte rows a leaf
holds about 15, and a random insert splits a leaf about one in eight times, writing a new page of mostly moved cells
(estimate: about 1.5 KB of journal per row, against 0.6 KB for C below). SQLite's own guide says WITHOUT ROWID suits
rows under about a twentieth of a page and warns that large rows are slower there ("traditional rowid tables tend to
work faster for large row sizes" because content is also stored in intermediate nodes,
[withoutrowid](https://www.sqlite.org/withoutrowid.html)); Krummi's branch nodes hold only keys (a branch cell is key
length, key and child page, `node.rs:19-21`), so that warning applies less.

**C. Heap by row id** (SQLite rowid tables) separates identity from key. A row has a monotonic id; the row tree is
ordered by it. The primary key is an ordinary unique index. Inserts append to the rightmost leaf, which Krummi's
split rule fills completely (`lib.rs:14-16`), and all inserts of a transaction share that dirty page. The primary key
tree has 20 to 40 bytes per entry, so it is 15 to 25 times smaller than a clustered tree of 500-byte rows, which makes
random key inserts cheaper in cache and in splits (one in about 130 against one in about 8). Secondary indexes hold 8
bytes whatever the primary key is. The price is the extra descent for primary key reads, the indirection for PK order
scans, 22 more bytes of overhead per row, and a second tree to keep in step. It was the first draft's recommendation,
written before the user's argument below.

On random ULID or UUID v4 keys, C does not dirty fewer distinct pages per batch than B: one random leaf per row in the
key tree, plus the appended row leaves. Its advantage is smaller pages to cache, fewer splits, and smaller diffs, not
page count. On time-ordered keys, B and C both append, B is one descent shorter, and C's range scans fetch rows in
nearly sequential order because row ids follow insertion order.

### Why the user's argument decides it

1. **Primary keys rarely change.** The case where B loses to C by a wide margin (update of the primary key: 2 + *k*
   pages against 1) is the rare case.
2. **Primary keys are artificial and increasing.** Auto-increment values, ULIDs (48 bits of milliseconds first, then
   an 80-bit random part incremented within a millisecond by the monotonic generator the product provides), and GUIDs
   from `NEWSEQUENTIALID()` or a sequential GUID generator (ordered per column like SQL Server, document 3) all grow
   with time. For those, inserts go past the last cell of the rightmost leaf, and the **append split** applies: a node
   that overflows past its last cell keeps its cells and a new sibling starts with the new cell, so keys added in
   increasing order fill their nodes instead of leaving them half full (`crates/krummi-tree/src/lib.rs:14-18`). The
   execution `trees-append-split` ([plan](../../../odd/tasks/trees-append-split.md),
   [report](../../../odd/tasks/trees-append-split-report.md); it had no PRD because it changed no product behavior)
   measured 100,000 `set` in increasing order: pages halve (543 to 272, height 3 to 2) for small keys and values;
   random order is unchanged (+0.6 % with 16-byte keys and 100-byte values). With 500-byte rows a clustered leaf holds
   about 16 and fills completely; a new leaf is opened every 16 inserts without moving a cell. That makes B as cheap
   as C for inserts, which was C's main advantage, and keeps B's shorter reads and its sequential scans.
3. **Composite keys scanned by prefix** (tenant, timestamp), C's weak case in the first draft, become sequential leaf
   walks in B.

The report's figures are for the tree layer with small values; they show the mechanism, not the cost of a 500-byte row.
The PRD's execution must measure the table insert with the three key kinds (see Costs and risks).

## Decided design

Per table, these trees (names are examples; the catalog naming is document 3's):

| Tree | Type | Key | Value |
|---|---|---|---|
| `rows` | normal | encoded primary key (document 3), 1 to 2,048 bytes | encoded row (document 1) without the primary key columns, up to 256 MiB; off-leaf above the leaf threshold |
| `ix:<name>` | normal | `encoded index columns ‖ encoded primary key` (document 3) | the included (covering) columns in document 1's row format, or empty |
| `stats` | normal | column id (and index id) | histogram and counts (document 3, statistics) |
| `meta` | normal | fixed names | schema versions, flags, modification counter |
| `dict` | normal, reserved | big-endian dictionary id | dictionary (document 7); not created until compression is built |

There is no `pk` tree and no row id. The primary key is mandatory and its columns are never null.

**Operations.**

Every write of a row runs in two phases (corrected 2026-11-10 after an external review; N10 of the tables PRD: the
public validation errors are detected before the operation changes anything). The **check phase** changes nothing:
it validates every value (invalid value), encodes the row, the primary key and every index entry the write needs and
checks each against its maximum (invalid size, naming the index), and probes the primary key and every unique index
whose key the write adds, skipping the row's own entry (duplicate key, naming the index). Only then does the **write
phase** change the table tree, the indexes, the autonumeric counter (whose next value the check phase reads without
taking it) and the modification counter. A public validation error leaves every structure as it was and the
transaction usable (B7); a failure in the write phase (full disk, maximum size, memory, damage, I/O) follows T11.

Operations over many rows cannot check row by row and then write, because a later row may fail after earlier ones
were written, and a write transaction has no savepoint to undo them (found 2026-11-10 when looking for
counterexamples to the two phases):

- *Bulk load* (N6). Corrected in the third review round (H9, 2026-11-10): the second round checked every row of the
  load before the first write, which needs the rows twice (a source the application can read only once would have to
  be buffered whole in memory) and its own detection of duplicates inside the load. The simpler rule: **a bulk load
  that fails whole is one write transaction of its own**, which the load opens, fills and commits. It reads each row
  once and writes it at once with the two phases of a single insert; its probes see the rows it already wrote, since
  they are in its transaction, so a duplicate inside the load, between a generated and an explicit value, or with an
  existing row is found by the same probe as any duplicate. Any failure (invalid value, duplicate, size, the
  autonumeric counter exhausted, the transaction's maximum size P10, a full disk) aborts that transaction, and the
  load fails with that class: no row of it remains, whichever row failed, the last one included, and the counters
  (autonumeric and modifications) are as before, since they are table data of the aborted transaction. Only the size
  of `krummi.data` may stay larger, as after any aborted write that grew it (T10, P11). Memory is that of any large
  write (the scratch area), and the load's size limit is the transaction's (P10). A thread that already holds the
  write transaction and starts a load gets T5's misuse error at once, as for any second write of the same thread.
  The load holds the write turn while it reads its source (fourth review round, H19): readers are not affected
  (MVCC), and other writers wait as for any long write, within their own time limits (T4). The source is a fallible
  sequence of rows from the application: an error from it, which is also how the application cancels or puts a
  deadline on a load, aborts the load and returns that error inside the load's failure; the end of the sequence is
  the only way to commit. A panic in the source or in the load, or a load dropped unfinished, aborts too, and the turn
  goes to the next writer with no action of the application (T10). The size limit is P10's; Krummi adds no time limit
  of its own. Tools that load from files (document 10, module 19) put their own limits and interruption on top of the
  same API: an interrupted process leaves no partial load, since an uncommitted write never survives (D9). If 7.1
  instead makes a bad row fail
  only that row, each row runs its own two phases and the load reports which rows failed; then the load could run
  inside the caller's transaction. Which one is 7.1's decision; the PRD lists it.
- *Creating a unique index over existing rows* (7.2 fixes what happens when they repeat values). Corrected in the
  fourth review round (H15, 2026-11-10): the third round built the index into a tree and deleted it on a duplicate,
  which allocates and frees pages before the duplicate is known; that breaks N10's "detected before the operation
  changes anything" if 7.2 makes the duplicate key class fail without effect, and needed a caveat on space accounting.
  The fix needs no savepoint: the build is already a sorted bulk load (doc 3, *Index creation on an existing table*).
  It reads the table, encodes and sorts the entries (in memory, or for a large table in temporary sort runs outside
  the store's data pages, as doc 8's sort runs; where they go is 7.2's decision), and a duplicate shows up as two
  neighbours in the sorted entries whose encoded column parts are equal byte strings, before the first page of the
  index tree is allocated. The test compares the whole column part, never a byte prefix ("a" against "a\0", doc 3),
  and skips entries with a null column, which never collide (N7); whole entries never compare equal, since each ends
  with its row's primary key. Only then is the tree
  written, by the append path. So a duplicate fails before the store changes at all: catalog, rows, indexes, counters,
  statistics, free pages and file size are as before, and the transaction can commit its other changes. Failures
  while writing the tree (full disk, P10) are T11's, not public validation errors. A non-unique index uses the same
  build without the check.
  Acceptance (AC-N7): after the failed creation and a commit of other changes, the index does not exist, the table and
  its other indexes are unchanged, and the store's information (P12) is as it would be without the attempt.
- *Range delete*: no public validation error can occur after the first row is removed, so it needs no check phase.

- *Insert:* encode the primary key; check that it is absent and check each unique index first (a duplicate must fail
  before anything changes, B7-style); `rows.Add(pk, row)`; each index adds `(columns ‖ pk, included)`. No statistics
  entry per insert: the trees keep their own entry counts (A5).
- *Update by primary key:* `rows.Read(pk)` (one descent) gives the old row, which is read anyway to build the new one;
  `rows.Store(pk, newRow)` after the check phase, which probes each unique index whose key changes. For each index
  whose encoded key or included value differs between old and new row, remove
  `(oldKey ‖ pk)` and add `(newKey ‖ pk)`, or replace the value when only the included columns changed. A row that
  grows relocates cells inside its leaf or splits it; every index stays as it is.
- *Update of primary key columns:* read the row, remove `rows[oldPk]`, add `rows[newPk]`, and for every index remove
  `(columns ‖ oldPk)` and add `(columns ‖ newPk)`. This is the 2 + *k* case; the user's premise is that it is rare.
- *Delete:* read the row for its index keys, remove from `ix:*` and from `rows`. An off-leaf row's run is freed
  without reading it (V7).
- *Point lookup by key:* `rows.Read(pk)`; by secondary key: scan the index range (the primary keys are in the entry
  keys), then `rows.Read(pk)` per entry unless the index covers the query. Both reads are zero-copy (D11).
- *Scans:* a full scan, and a scan by primary key range, walk `rows` in key order, the sequential leaves of one tree. A
  range over a secondary index returns entries ordered by the indexed columns and then by primary key.
- *Truncate and drop:* emptying or deleting the trees (A2) frees every page at once.

**Primary key columns live only in the key.** The row value does not repeat them (document 1); a reader that needs one
decodes the key by the schema. This saves the key's length per row and cannot disagree with the key. If the PRD's
measurement shows the decode costs more than the bytes, repeating the columns is a layout change per schema version.

**Order of a full scan.** It is primary key order, because that is how the rows are stored. Document it as a
guarantee: there is no other table kind to change it.

## Costs and risks

- **Random primary keys (UUID v4).** Every insert lands in a random leaf of a tree that holds the rows. Estimate (not
  measured): about 1 split in 8 inserts for 500-byte rows against 1 in 130 in a key-only tree, and about 1.5 KB of
  journal per row against 0.6 KB. The decision accepts the risk because the user's keys are sequential; the PRD must
  measure autonumber, ULID and random UUID inserts (the method of A15 of `trees-keys.prd.md`: sequential against random
  load) and state the figure. A UUID v4 primary key is allowed and documented as slower; the generators default to
  ULID or sequential order (document 3, question 9).
- **Primary key bytes in every secondary entry.** A 16-byte ULID over four indexes costs 64 bytes per row beyond the
  columns, and the index key limit of 2,048 bytes covers `columns ‖ pk`. A long composite primary key is the weak case;
  document 3 gives the numbers.
- **Primary key change** costs a row delete and insert plus a rewrite of every secondary entry. Rare by the user's
  premise; no mitigation designed.
- **Secondary lookups cost two descents** (the index, then `rows`). Branch pages are shared and hot; the cost is extra
  cache misses, not extra reads when the working set fits. A covering index (included columns) removes the second
  descent.
- **Row overhead.** 6 bytes per row in the leaf (4-byte cell header, 2-byte offset) plus the key, against 28 plus the
  key for the heap. Prefix compression of keys is not available (key bytes are stored whole); not worth adding.
- **Off-leaf rows** (value over 4,074 bytes minus the key, for example a vector of 1,024 floats or more) take whole
  pages plus one (V5), need contiguous pages (F5, V6) and are rewritten whole on update (runs are never changed,
  `lib.rs:7-9`). Same as in Voron overflow. A scan that reads only the primary key columns stays in the leaves: freeing
  and key reads never touch the run (V7). A scan of small non-key columns of a table with large vectors reads every
  run. A companion tree keyed by the primary key for large columns is a later remedy that the layout permits
  (document 1); it is not part of the first version.
- **Hole compaction.** A leaf that updates rows to larger sizes eventually compacts its cells, which changes much of the
  page (the diff approaches a full page). Bounded by the page and amortised; to watch in the update benchmark.
- **Estimates not measured.** The pages-changed table and the journal bytes per row are derived from the node format;
  the PRD execution must measure them on Windows and Linux with random and ordered keys before fixing the thresholds, as
  earlier tree PRDs did.

## Product questions

1. **Table kinds.** *Decided 2026-11-09:* clustered by primary key only; no heap kind. README: "Rows are stored
   clustered by primary key, as in SQL Server". The first draft recommended the heap; the user's argument replaced it.
2. **Row ids in the public API.** *Moot:* there are no row ids; the primary key is the identity.
3. **Tables without a primary key.** *Decided 2026-11-09:* required. README: "Hacemos la clave primaria obligatoria."
   The first draft recommended optional; logs and append-only data take an auto-increment or ULID key.
4. **Order of a full scan.** *Settled by decision 1:* primary key order, documented as a guarantee.
5. **Maximum row size.** *Still open.* Options: (a) the tree maximum, 256 MiB (B6); (b) a table limit such as 16 MiB.
   Recommendation: (a), with vector limits set in document 5, because the trees already handle it and a lower limit
   could be raised later but not the other way.

## Sources

Voron (`C:\projects\ravendb\src\Voron\Data`):

- `RawData/ActiveRawDataSmallSection.cs:14-21` (design comment), `29-94` (TryAllocate), `124-195` (DefragPage, DataMoved),
  `198-266` (section creation and size), `269-284` (IsOwned).
- `RawData/RawDataSection.cs:16-62` (header space, entry sizes), `83-90` (AvailableSpace), `187-224` (TryWriteDirect),
  `231-265` (DirectRead), `303-348` (Free), `398-404` (OnDataMoved).
- `RawData/RawDataSmallPageHeader.cs`, `RawDataSmallSectionPageHeader.cs`, `RawDataOverflowPageHeader.cs`,
  `DataMovedDelegate.cs:3`.
- `Tables/Table.cs:91-125` (active section, OnDataMoved), `357-446` (Update), `547-637` (Delete, ReleaseNearlyEmptySection),
  `716-796` (Insert, overflow, AllocateFromAnotherSection), `927-1000` (UpdateValuesFromIndex), `1097-1148`
  (InsertIndexValuesFor), `1186-1230` (new section, candidate sections).

Krummi (`C:\projects\Krummi`):

- `docs/prd/krummi/krummi.prd.md` (D1–D12; D11 zero-copy reads at line 52).
- `docs/prd/krummi/trees.prd.en.md:42-45` (B3, B5, B6); `trees-multivalue.prd.en.md:40-47` (M1, M2, M5–M7);
  `trees-fixed-size.prd.en.md:40-46` (X1, X5, X6); `trees-large-values.prd.en.md:42-48` (V1–V7).
- `docs/prd/krummi/free-space.prd.en.md:39-45` (F2–F8); `transactions.prd.en.md:45` (T14).
- `docs/performance/plan.md:109-128` (cost of a write), `418-436` (O8, page diffs).
- `crates/krummi-tree/src/lib.rs:6-20` (no path copy, append split rule, runs never changed);
  `crates/krummi-tree/src/node.rs:1-47` (slotted node, holes, leaf threshold, `MAX_LEAF_CELL`);
  `crates/krummi-tree/src/fixed.rs:1-30` (dense node, fan-out).
- `odd/tasks/trees-append-split.md` and `odd/tasks/trees-append-split-report.md` (the append split and its
  measurement; no PRD exists for it).
- `odd/tasks/trees-large-values.md:42-59` (D2, D6, D7); `odd/tasks/trees-fixed-size-measurements.md:38-50`.

Other systems:

- SQLite, WITHOUT ROWID tables: https://www.sqlite.org/withoutrowid.html (fetched in the fact-check, 2026-11-09).
- SQLite database file format, rowid tables and index b-trees: https://www.sqlite.org/fileformat2.html (cited from
  knowledge, not re-read).
- MySQL InnoDB clustered and secondary indexes: https://dev.mysql.com/doc/refman/8.0/en/innodb-index-types.html (cited
  from knowledge, not re-read).
