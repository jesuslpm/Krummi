# 01. Column types and the encoding of a row

Status: reconciled 2026-11-09 with the user's decisions (see [README](README.md)) and the [fact-check](verification.md).
Part of the tables design set. The PRD of module 7 takes the decided design below as its starting state; questions
still open are marked in the last section.

## Summary

1. A table value (a row) is one contiguous byte string stored as the value of the table tree, whose key is the primary
   key (document 2). It has a **fixed-offset section** for small fixed-width columns, a **null bitmap**, an
   **end-offset table** for the variable-length and large columns and a **variable section**. It is a schema-driven
   layout, not self-describing. Fixed-width primary key columns live only in the key; `string` and `bytes` primary
   key columns are also in the row, so that they read without a copy (corrected 2026-11-10, Codex review R4).
2. Access to column *i* is O(1) with no decoding: a fixed column is `base + offset[i]`; a variable column is two
   offset reads. Reads return `&[u8]` slices into the page (D11); numbers and vector elements are read with
   `from_le_bytes` (safe code, D12), so **no alignment** of the row, of a column or of a vector is needed or
   attempted. (Index and primary keys are a different encoding, the FoundationDB tuple layer of document 3, read
   sequentially.)
3. Each row starts with a 2-byte **schema version**. Adding a nullable column, dropping and renaming never rewrite
   rows; a row is upgraded only when it is rewritten for another reason.
4. Vectors are raw little-endian elements with the dimension in the schema (no per-value header, no padding), plus a
   4-byte `f32` scale for `i8` and a 4-byte `f32` norm for a cosine column (the one layout of document 5). They live in
   the variable section, so a null vector costs 0 bytes. The maximum dimension is 4,096.
5. Decided type set: bool, i8–i64, u8–u64, f32, f64, decimal(p, s) (128-bit fixed point, up to 38 digits), string,
   bytes, date, time, timestamp (UTC, microseconds), duration, uuid, ulid, vector (f32, f16, i8, bit). Every column
   is nullable unless declared `NOT NULL`. Arrays, enums, JSON, stored time zones, scalar f16 and case-insensitive
   collation are out of the first version. Typical row overhead: 6 bytes over the payload, against 11 (Voron), 7
   (SQLite) and about 30 (PostgreSQL).
6. Dictionary compression (document 7) is deferred, with its hooks reserved here: a "compressed" bit in the header
   flags byte and, when it is set, a dictionary id right after the 3-byte header.

## Requirements

From the agreed scope ([README](README.md)) and the root PRD:

- R1. Zero-copy column access (D11): a read returns references into engine memory valid while the read transaction is
  open. Reading column *i* must not decode or copy the other columns.
- R2. O(1) access to any column, including after variable-length ones, because the query layer will filter and
  project single columns over millions of rows.
- R3. Low fixed overhead per row and near-zero overhead per null; rows are values of the B+ trees of
  [trees.prd.md](../../prd/krummi/trees.prd.md), so every byte is paid in every leaf page and in the journal.
- R4. Updating one column must be cheap: the row is a copy-on-write tree value, so the cost is one memcpy of the row
  plus the write of the new leaf.
- R5. Schema evolution without rewriting rows: add a nullable column (or one with a default), drop, rename.
- R6. Vector distance kernels must run at full speed although a value starts at any byte of a page (the tree does not
  align values). D12 forbids `unsafe` outside the pager, so the row code must be safe Rust. Decided: the kernels read
  `&[u8]` with `from_le_bytes`, which compiles to unaligned loads, so the row format needs no padding (document 5
  shows the only reason one could have been, alignment, does not apply; fact-check contradiction 5).
- R7. Compression by dictionary later (doc 7) must fit without changing the layout.
- R8. The same layout must be readable from C and C# without a Rust-specific representation (doc 6).
- R9. Column types must have an order-preserving key encoding for those that can be indexed (doc 3: FoundationDB tuple
  elements; the row format below is a separate encoding and does not use it).

## Column types

Fixed widths are in bytes. The type set is decided (README, 2026-11-09). The order-preserving encoding of each type
in an index or primary key is the FoundationDB tuple element of document 3; this table covers the storage in a row.
Every column is nullable unless declared `NOT NULL`; primary key columns never are.

| Type | Storage | Cost and notes |
|---|---|---|
| `bool` | 1 | Trivial. |
| `i8 i16 i32 i64` | 1/2/4/8 LE | C and C# map directly. |
| `u8 u16 u32 u64` | 1/2/4/8 LE | C# has `byte/ushort/uint/ulong`; Java-style bindings would not, which does not matter here. |
| `f32`, `f64` | 4/8 LE IEEE 754 | Decided: `-0` is stored as `+0` and every NaN as one canonical NaN that sorts after `+∞`, applied at write so that rows and keys agree. Equality on floats is a planner trap. |
| `f16` as a scalar column | - | Out of the first version: Rust `f16` is not stable (tracking issue 116909) and C has no portable type; it exists only as a vector element (below). |
| `decimal(p,s)` | 16, i128 scaled by 10^s, p <= 38 | Exact, order-preserving once biased, comparison is an integer comparison. C has no `__int128` on MSVC (use two `u64`); C# has `Int128` (.NET 7+). A C# `decimal` has a 96-bit mantissa (about 7.9e28), so an i128 with p up to 38 converts only when the magnitude fits and `s <= 28`, not by scale alone. The alternative, IEEE decimal128, costs a software library for little gain. |
| `string` | UTF-8 bytes, variable | Validated at write (UTF-8 is a read-side guarantee: `&str` without a check). Decided: binary order, no collation. C# needs a UTF-8 to UTF-16 copy on read unless the binding exposes spans. |
| `bytes` | variable | Same as string without validation. |
| `date` | i32 days since 1970-01-01 | Proleptic Gregorian. C# `DateOnly` maps with an offset. |
| `time` | i64 microseconds since midnight | C# `TimeOnly` has 100 ns ticks: lossless one way only. |
| `timestamp` | i64 microseconds since the Unix epoch, UTC | Range +-292,000 years. Nanoseconds in an i64 would limit it to 1677-2262; C# `DateTime` ticks (100 ns) convert exactly to microseconds only if truncated. Decided: a UTC instant, no time zone stored. |
| `duration` | i64 microseconds | No calendar intervals (months, days): they have no total order and are a SQL-semantics rabbit hole. |
| `uuid` | 16 bytes, RFC 9562 byte order | Always stored in RFC 9562 order; the per-column choice of order (RFC bytes or SQL Server's `uniqueidentifier` order, for data from `NEWSEQUENTIALID()`) is made in the key encoding, which permutes the bytes (document 3), so the row never changes. C# `Guid` is mixed-endian in memory: the binding must swap the first three fields. |
| `ulid` | 16 bytes, 48-bit ms timestamp then 80-bit random, big-endian | Its own type, physically identical to `uuid`. Bytewise order equals creation order, so it works as a clustered primary key with append-friendly inserts (document 2). Krummi provides the monotonic generator (80 random bits incremented within a millisecond). Keeping both as logical types costs one enum entry and gives the planner and the bindings the right text form (Crockford base32 vs hex). |
| `vector(elem, dim)` | see "Vector payload" below | `elem` in `f32` (4 bytes), `f16` (2, IEEE binary16), `i8` (1, plus a 4-byte scale) or `bit` (1 bit). `dim` fixed per column, **1..=4,096** (one maximum for every element type; document 5). No per-value header: the schema carries the dimension. Not orderable and never a primary or index key; supports equality (memcmp) and distance functions (doc 5). `f16` needs software conversion (or `F16C`/NEON) at compute time. |
| nullability | per-column flag | Default nullable; `NOT NULL` is declared. A null costs one bitmap bit. Declare `NOT NULL` where possible: a nullable fixed column keeps its slot when null (see below). |
| arrays/lists | not in v1 | A list of fixed-width items is a vector without a distance; a list of variable items needs a nested offset table, nested nulls and a key encoding for list ordering. It can be added later as a new type code in the same layout. |
| enums | not in v1 | A small integer plus a catalog mapping; adds a catalog migration story (renumbering, dropping). Use `u8/u16` and a lookup meanwhile. |
| JSON, stored time zones, case-insensitive collation | not in v1 | Decided out of the first version. |

**Vector payload** (the only layout; document 5 gives the arithmetic):

```text
f32 : dim x 4 bytes, little-endian IEEE 754 binary32
f16 : dim x 2 bytes, little-endian IEEE 754 binary16
i8  : dim x 1 byte, then the scale: 4 bytes f32 (value = byte x scale)
bit : ceil(dim / 8) bytes, bit i = (byte[i / 8] >> (i % 8)) & 1; unused high bits of the last byte are zero
+ for a cosine column (f32, f16, i8): 8 bytes f64, the L2 norm, after the elements and the scale (doc 5, R3)
```

The length is a function of the column (element, dimension, cosine flag), so no length is stored. A `bit` column's
`dim` is a multiple of 8 (N4; corrected 2026-11-10, Codex review R7). There is no alignment padding.

Everything above encodes into one of three physical classes, which is what the row layout cares about: **small fixed**
(<= 16 bytes), **large fixed** (vectors; any fixed-width type above 16 bytes), **variable** (string, bytes).

## Options compared

### Voron's table value

`TableValueBuilder.CopyTo` ([TableValueBuilder.cs:133-202](../../../../ravendb/src/Voron/Data/Tables/TableValueBuilder.cs))
writes: a varint column count, then one **start offset per column**, each 1, 2 or 4 bytes wide, then the column bytes
concatenated. Offsets are relative to the start of the offset table, just after the count varint (the first offset equals
`count * elementSize`, line 145, so the table itself is included in the offsets; the reader returns
`dataPtr + position`). The width is chosen from the total size (`TableValueReader` constructor, lines 36-41: >255
bytes selects 2, >65,535 selects 4; the builder's `ElementSize`, lines 28-48, counts the offsets in the size and so
uses a slightly different threshold). `TableValueReader.Read(index, out size)`
([TableValueReader.cs:67-99](../../../../ravendb/src/Voron/Data/Tables/TableValueReader.cs)) reads `offset[i]` and
`offset[i+1]` (or the row end) and returns a pointer and a size: O(1), zero-copy, no types. The format is
schema-less: every column, fixed or not, costs one offset, and the meaning (and fixed size, asserted only in debug,
line 51) is known only to the caller. Nulls are not representable except by convention (an empty column). Rows may
be compressed whole by `TableValueCompressor` (builder lines 20, 204-226).

### SQLite record

A header made of a varint header length followed by one **serial type varint per column**, then the bodies. Integers
are stored with the minimal width (0, 1, 2, 3, 4, 6 or 8 bytes) and the constants 0 and 1 take no body; a string of
n bytes has serial type `13 + 2n`, a blob `12 + 2n`, NULL is type 0. Very compact, null costs 1 header byte.
Access to column *i* requires decoding all serial types before it: O(i), and the data offset is a running sum.
Adding a column is free (a short record takes the column's default value from the schema, NULL only when there is none). Integers need decoding (no `&i64`
into the page).

### PostgreSQL heap tuple

A 23-byte header (xmin, xmax, cid, ctid, infomask, hoff), an optional **null bitmap** (1 bit per column, 1 = not null, present only
if any column is null; the Krummi bitmap below uses 1 = null), then attributes padded to their alignment (int4 to 4, int8 to 8, double to 8); variable
fields have a 1- or 4-byte length header. Fixed columns before the first variable one are O(1) (`attcacheoff`);
anything after a variable column needs a walk. Padding and the header make it heavy (about 28-30 bytes minimum per
row); the 23-byte header holds MVCC data that Krummi keeps elsewhere. Adding a nullable column is free (short tuple
reads as null, `attmissingval` for defaults). Alignment is solved by padding relative to the tuple start, which
PostgreSQL aligns to 8 in the page.

### Fixed offsets plus an offset table (FlatBuffers, Cap'n Proto, Spark UnsafeRow)

FlatBuffers uses a per-type vtable of field offsets and pointers for variable data (self-describing, with alignment
padding); Cap'n Proto uses a fixed data section plus a pointer section; Spark's `UnsafeRow` places a null bitmap, fixed-width
fields at schema-known offsets and an offset/length word for variable fields. (Arrow defines only a columnar format;
arrow-rs's `arrow-row` is a byte-comparable format made for sorting and grouping, not a fixed-offset row.) The common idea: **the
schema fixes the offset of every fixed-width column**, and only variable columns need a table. Access is O(1)
everywhere, nulls are a bitmap, no per-column cost for fixed columns.

### Comparison

| Criterion | Voron | SQLite | PostgreSQL | Fixed + offsets (proposed) |
|---|---|---|---|---|
| Zero-copy column access | yes | variable cols yes, ints need decode | yes if aligned | yes |
| Access to column *i* | O(1) | O(i) | O(1) before first variable, O(i) after | O(1) |
| Per-row overhead | 1 + w*n (w = 1, 2, 4) | 1-2 + ~1*n | 23-30 + bitmap + padding | 3 + bitmap + w*(var-1) |
| Per null | no real null | 1 byte | 1 bit | 1 bit (fixed slot kept for <=16 B) |
| Types in the row | none (caller knows) | per value | none | none (schema) |
| Update one column | rebuild whole row | rebuild whole row | rebuild (new tuple version) | rebuild; fixed same-size column patchable in the write copy |
| Add nullable column | rewrite or caller convention | free | free | free (version) |
| Alignment for vectors | none | none | padding rel. to an 8-aligned tuple | none needed: elements are read as bytes with `from_le_bytes` |
| Dictionary compression later | whole row | per value | TOAST | flag bit, whole body |

Voron's format is the right shape for the variable part, but it pays an offset for every fixed column and has no
nulls, types or versions. SQLite is the most compact for narrow rows but its O(i) access and decode-on-read violate
R1/R2 for wide analytical filtering. PostgreSQL's overhead comes from MVCC and alignment; neither is needed here.
The fixed-offset layout is the only one that satisfies R1-R3 together, and it already has the shape of Voron's
reader for the variable part.

## Decided design

### Row layout ("row v1")

All integers little-endian. `V` = number of variable columns in the row's schema version, `N` = nullable columns.

```text
offset  size            field
0       2               schema_version        u16, index into the table's schema history
2       1               flags                 bits 0-1: offset width (0=u8, 1=u16, 2=u32)
                                              bit 2: body compressed (hook for doc 7, see below)
                                              bits 3-7: 0 (reserved)
3       ceil(N/8)       null bitmap           1 bit per nullable column, in column-id order, 1 = null;
                                              absent if the version has no nullable column
.       F               fixed section         small fixed columns (<= 16 B) at schema-computed offsets
.       (V-1) * w       end offsets           end of variable column j relative to the variable section
                                              start, for j = 0..V-2; the last ends at the row end
.       rest            variable section      string, bytes and vector columns, concatenated, in layout order
```

Rules:

- **Layout order is computed from the schema version**, not stored: fixed columns sorted by descending width (8, 4,
  2, 1) so the fixed section needs no padding, with ties broken by column id; variable columns in column-id order.
  The offset of every fixed column is a constant of the (table, version), cached in a per-version table.
- **Nullable fixed columns keep their slot** (a null writes zeros); that wastes at most 16 bytes per null and keeps
  every offset constant. **Variable and large columns take 0 bytes when null**: the end offset equals the previous
  one and the bitmap bit tells empty from null. This is why vectors and strings go to the variable section and why
  16 bytes is the cut: a nullable 768-dim vector in a fixed slot would waste 3 KB.
- **Variable column *j*:** `start = j == 0 ? 0 : end[j-1]`, `end = j == V-1 ? row_len - var_start : end[j]`. `var_start`
  is a constant of the version plus `(V-1)*w`. Two reads, no loop. The value size comes from the tree (the row is
  one tree value), so the last offset is implicit, as in Voron's last column.
- **Offset width** is written in `flags` by the builder (the smallest width that fits the variable section), not
  inferred from size as Voron does, so the writer has no circular dependency between width and size.
- **Compression hook (one place, agreed with document 7):** the `flags` byte carries the "compressed" bit (bit 2).
  When it is clear, the row is exactly as above. When it is set, the 3-byte header (schema version and flags) stays
  readable, and what follows it is `dictionary_id` (varint; 0 means plain LZ4 with no dictionary), `raw_len` (varint,
  the size of the uncompressed body, which `lz4_flex`'s dictionary decoder needs) and one LZ4 block holding the body
  (null bitmap, fixed section, end offsets and variable section). The body is decompressed once into transaction
  memory (D11) and read as above. Compression is whole-row; vectors and numbers are not compressed on their own.
  Nothing else in the layout changes, and until document 7 is built the bit is always clear.
- **Primary key columns in the row: only `string` and `bytes`.** In a clustered table the key of the tree holds every
  primary key column (document 2). A fixed-width one (integers, `uuid`, `ulid`, temporal types, `decimal`, `bool`) is
  left out of the row and decoded from the key: its value is a scalar the API returns by value anyway (the PRD's
  scalar exception). A `string` or `bytes` one is also stored in the row, because its key element escapes each `00`
  as `00 FF` (document 3): a `bytes` key `41 00 42` is `01 41 00 FF 42 00` in the key, so the original bytes are not
  contiguous there, and returning them would need a copy, which the PRD's output rule and T16 forbid. Corrected
  2026-11-10 after the Codex review (R4): the first version left every primary key column out of the row. Cost: the
  length of the variable primary key columns once more per row, zero for the usual `ulid`, `uuid` or integer keys.

### Reading

- Fixed column: `u32::from_le_bytes(row[off..off+4].try_into().unwrap())`. Safe, bounds-checked once per access, and
  LLVM lowers it to an unaligned load. Null check: one bitmap test.
- String/bytes/vector: returns `&[u8]` (`&str` after the validation done at write), borrowed from the transaction.
- Vector kernels: `as_chunks` over the bytes with `from_le_bytes` into fixed-lane accumulators auto-vectorizes with
  SSE/AVX/NEON using unaligned loads (document 5). The row is not aligned in the page and nothing in the format tries
  to align it: the assumption that an unaligned load not crossing a cache line costs the same as an aligned one on
  x86-64 is not measured here (the Krummi benchmark of `byte_sum` is the precedent), and a `bytemuck` cast fast path
  would apply only to an aligned value, which a leaf cell is not.

### Schema evolution

The catalog stores, per table, an append-only list of **schema versions**; each version lists its columns (stable
`column_id`, current name, type, nullable, default). A row records the version it was written with.

- **Add a nullable column:** new version; old rows read it as null (the reader sees that the row's version has no such
  column). **Add with default:** the default is stored in the catalog and returned for old rows, as PostgreSQL does
  with `attmissingval`. Writing a new row always uses the latest version.
- **Drop a column:** only a column outside the primary key and every index (N3); dropping one that a primary key or
  index uses fails without effect (AC-N6, AC-N7). The next version omits it; old rows keep its bytes until rewritten
  (space is reclaimed lazily). Corrected 2026-11-10 (Codex review R7): the first version dropped the index entries.
- **Rename:** catalog only; rows refer to `column_id`.
- **Allow nulls** in a `NOT NULL` column outside the primary key (N3): catalog only; the new version marks it
  nullable and old rows, which always hold a value, read unchanged (corrected 2026-11-10, R7: the first version
  rejected every nullability change).
- **Change type, forbid nulls:** rejected in this module (N3). Widening of integers could be done by the reader, a
  later version's decision (Q6).
- **Cost of mixed versions:** a scan resolves `(version, column_id) -> offset` through a small array indexed by
  version, built once per scan and column; the hot loop is the same as for a single version. If the number of live
  versions grows, a background or opportunistic rewrite on update upgrades rows; a full-table "upgrade" operation
  is a later feature.
- A new NOT NULL column without a default requires rewriting rows and is rejected in v1.

### Update of one column

Rows are immutable tree values; an update builds the new row: `memcpy` of the unchanged parts (the builder copies
slices of the old row, which D11 gives for free) and writes the new column. Same-size fixed columns can be patched in
the write copy of the leaf by the table layer. Cost is one row copy plus one leaf copy-on-write, identical to Voron.

### Estimated overhead for the typical row

Row: ULID `id` (primary key), `i64`, `i32`, `i32` (one nullable, the others declared `NOT NULL`), `string` of 40
bytes, `vector(f32, 768)` `NOT NULL`. Payload: ULID 16 (lives in the tree key; doc 2), 16 bytes of ints, 40 string,
3,072 vector. The row is a leaf value of 3,134 bytes, under the 4,074-byte inline limit minus the 17-byte key, so
two rows share a leaf.

| Format | Overhead without ULID in the row | Total value size |
|---|---|---|
| Proposed | 3 header + 1 bitmap + 2 (one `u16` end offset; V = 2) = **6** bytes; no vector padding (none exists) | 16 + 40 + 3,072 + 6 = **3,134** |
| Proposed, if the primary key were repeated in the row | same 6 | 3,150 |
| Voron | 1 (count) + 5 columns * 2 = **11** bytes (width 2 since >255) | 3,139 |
| SQLite | 1 (header length) + 5 serial types (ints 1 each, string 1, vector 2) = ~**7**; ints may shrink | ~3,120-3,135 |
| PostgreSQL | 24 header + bitmap + ~4 padding + 1-4 varlena headers = **~30-34** | ~3,160 |

The difference between formats is about 5-30 bytes against 3 KB of payload, because the vector dominates. For narrow
rows (say three `i64` columns, 24 bytes) the proposed overhead is 3 bytes with no variable column, against 4 (Voron:
1 + 3 * 1), 4 (SQLite) and 30 (PostgreSQL). The format wins on access time, not on bytes.

## Costs and risks

- **Not self-describing.** A row cannot be decoded without the catalog and its version. Mitigation: the version number
  is in the row, the catalog is part of the same store and covered by the same transaction; tooling (a dump command)
  needs the catalog. Corruption in a version number is caught by the page checksum.
- **Wasted space for nullable fixed columns** (<= 16 bytes per null). Accepted for constant offsets. If a table has
  many sparse nullable small columns, a later layout flag could switch to a sparse form; not designed now.
- **Large vectors dominate the row.** A 3 KB vector makes scans that touch only small columns read pages full of
  vectors: a 768-dim f32 row (3,134 bytes) fits two to an 8 KiB leaf (`PAGE_SIZE = 8192`), and a 1,536-dim row
  (6,144 bytes) exceeds the 4,078-byte leaf cell limit and always lives off its leaf in a contiguous run. Reading the
  vector column is free, but reading the others is not. Possible later remedy that this layout permits without changing it: store large columns in a companion tree
  keyed by primary key (the row would then hold the other columns only, with a tag in the schema). Document 2's
  clustered layout leaves the door open; it is not part of the first version.
- **Inline limit and max row size.** A row is one tree value: it is stored in its leaf while `4 + key + value <= 4,078`
  bytes (`MAX_LEAF_CELL`), and above that in a contiguous run of pages (document 2). The maximum is the tree's
  large-value limit, 256 MiB (`MAX_VALUE`, B6 and V1 of `trees-large-values.prd.md`; V2 is the contiguous
  zero-copy read rule); offset width 4 covers it. A single
  `u32` offset would cap rows at 4 GiB, fine.
- **Float semantics** (NaN, -0) are fixed at the type level (decided: `-0` stored as `+0`, one canonical NaN sorting
  after `+∞`), so that equal values compare the same in index and row (doc 3).
- **Fixed-width primary key columns decoded from the key.** Reading one costs a walk of the key's elements (document
  3), not the O(1) of a row column; repeating them in the row is a per-schema-version layout change if measurement
  asks for it. `string` and `bytes` primary key columns are in the row (R4 above).
- **Endianness** is fixed to little-endian; big-endian hosts are not a target.
- **UUID/ULID vs C# `Guid`/third-party ULID types** differ in byte order; the C# binding (doc 6) converts, and tests
  must pin it.
- **Schema version limit** of 65,535 per table, which is reached only after thousands of migrations; a table
  rewrite resets it.
- The 16-byte cut between "small fixed" and "large fixed" is a heuristic; a `decimal` or `uuid` with many nulls
  could prefer the variable section. Measure before changing; the layout is computed from the schema so it is
  changeable per table version.

## Product questions

1. **Type set.** *Decided 2026-11-09:* the list in "Column types" (README, "Column types of the first version": "Sí").
   Arrays, enums, JSON, stored time zones, scalar `f16` and case-insensitive collation are out. Enums or arrays of
   fixed-width scalars remain candidates for a later version.
2. **Timestamp resolution.** *Decided:* microseconds in an i64, a UTC instant; `time` and `duration` follow.
3. **Time zones.** *Decided:* UTC instants only; local time and zone stay in the application.
4. **Decimal.** *Decided:* `decimal(p, s)` as 128-bit fixed point up to 38 digits.
5. **Vector limits.** *Decided:* at most 4,096 dimensions for every element type (one maximum, document 5); elements
   `f32`, `f16`, `i8`, `bit`; `bit` requires a multiple of 8 (N4, README; corrected 2026-11-10, R7); a vector column is nullable like any other
   column (the layout supports a null vector at zero cost). The first draft proposed 16,384 and padding to 4 and 2
   bytes; both are dropped (document 5 reads `&[u8]`, so no alignment is needed).
6. **Schema changes in v1.** *Decided with the split of module 7:* child 7.1 includes "schema changes without
   rewriting rows": add a nullable or defaulted column, drop, rename. Not supported: change a column type or add
   `NOT NULL` without a default (they need row rewriting). Integer widening *remains open*, for a later version.
7. **Collation.** *Decided:* binary UTF-8 order, no collation.
8. **Both `uuid` and `ulid`?** *Decided:* two logical types over the same 16-byte storage; `uuid` has a per-column
   order option (document 3).
9. **Primary key columns in the row.** *Settled 2026-11-10 (Codex review R4):* `string` and `bytes` primary key
   columns are in the row, because zero-copy reads require it; fixed-width ones stay only in the key. Repeating those
   too remains open, if measurement shows decoding the key costs more than the bytes.

## Sources

Voron (local, read for this document):

- `C:\projects\ravendb\src\Voron\Data\Tables\TableValueBuilder.cs`: offset width selection lines 28-48; `RawSize` line
  58; `CopyTo` (count varint, offset table, data) lines 133-202; compression hooks lines 20, 204-226; `CreateReader`
  lines 232-237.
- `C:\projects\ravendb\src\Voron\Data\Tables\TableValueReader.cs`: element size from total size lines 36-41; count
  varint line 43; `Read(index, out size)` lines 67-99; `ReadLong` size assertion line 51.

Krummi:

- `docs/prd/krummi/krummi.prd.md`: D11 (line 51 and annotations), D12 (line 52).
- `docs/prd/krummi/trees.prd.md` and `trees-large-values.prd.md` (V1: values up to 256 MiB; V2: large values read contiguous).

External:

- SQLite record format: https://www.sqlite.org/fileformat2.html#record_format
- PostgreSQL heap tuple layout and page: https://www.postgresql.org/docs/current/storage-page-layout.html
- PostgreSQL `attmissingval` (fast add column with default): https://www.postgresql.org/docs/current/catalog-pg-attribute.html
- FlatBuffers internals (vtable, alignment): https://flatbuffers.dev/internals/
- Cap'n Proto encoding (data and pointer sections): https://capnproto.org/encoding.html
- Apache Arrow columnar format (validity bitmaps, offsets): https://arrow.apache.org/docs/format/Columnar.html
- RFC 9562 (UUID byte order): https://www.rfc-editor.org/rfc/rfc9562
- ULID specification: https://github.com/ulid/spec
- Unaligned loads on current x86-64 (Intel optimization manual, split-line penalty only) and ARM64: Intel 64 and IA-32
  Architectures Optimization Reference Manual, memory access chapter.
