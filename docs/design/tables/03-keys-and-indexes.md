# Keys and indexes: technical design

Document 3 of the [tables design series](README.md). It chooses the order-preserving encoding of typed column values
into the byte keys of Krummi's trees, the structure of primary and secondary indexes, and the statistics a cost-based
planner (document 4) reads. Written 2026-11-09 from the code and PRDs of the repository and from the Voron source;
reconciled the same day with the user's decisions and the [fact-check](verification.md). Nothing was built or measured
for it, so every cost below is an estimate and says so.

## Summary

1. **Decided:** keys of tables and index entries use **FoundationDB's tuple encoding**
   ([tuple.md](https://github.com/apple/foundationdb/blob/main/design/tuple.md)), not a format of Krummi's own: a
   standard, self-describing format with existing implementations, compared with Krummi's unsigned byte-wise order
   (B5). Each element is a type code followed by its bytes: variable-length big-endian integers, strings and byte
   strings with `00` escaped as `00 FF` and ended by `00`, floats with the sign trick, `uuid` with the UUID code.
   Extensions inside the specification: `ulid` with the UUID code, `uuid` in SQL Server order by permuting its 16
   bytes, `timestamp`/`date`/`time`/`duration` as integers, `decimal` with the user type code `0x40` (`0x41`–`0x4E`
   reserved for future types, `0x4F` for the null of an ascending column with nulls last), and a
   descending column by inverting the bytes of its element, with the terminator `00 01` for strings and byte strings
   and a three-byte null when it sorts last (Descending columns below; corrected 2026-11-10).
2. A table's row is stored under the **tuple of its primary key**. A secondary index is one ordinary tree whose key is
   **`tuple(index columns) ‖ tuple(primary key)`** (no row id) and whose value holds the included (covering)
   columns. Concatenating tuples is the tuple of all the elements, so an equality on the first k columns is a byte
   prefix.
3. Nulls first by default, first or last per index column; in a unique index nulls are distinct; an index key over the
   tree's 2,048-byte limit is rejected with a typed error before anything changes.
4. **Statistics are estimates, not exact counts:** per-column histograms that exist also for columns without an index,
   created and refreshed automatically in the background and forced by an explicit `analyze`; plus the exact entry
   count Krummi already keeps, plus a range estimate by descent. No counts in branch nodes.

## Requirements

- **R1.** Index and primary keys compare in the typed order using only Krummi's byte comparison; no application
  comparators (B5, [trees.prd.md:45](../../prd/krummi/trees.prd.md)).
- **R2.** Keys fit the 2,048-byte maximum (B5); an oversized key must fail cleanly, never corrupt.
- **R3.** Equality on a prefix of the columns, and a range on the next column, are one tree range scan (the tree
  already has prefix and bounded scans, ascending and descending).
- **R4.** All the types of document 1 that make sense as keys: bool, integers, floats, decimal, string, bytes, date,
  time, timestamp, duration, uuid, ulid, nullable variants. Vectors are not indexable (decided: a vector is never a
  primary or index key).
- **R5.** Unique and non-unique indexes, multi-column, descending columns, covering columns from the first version;
  partial indexes reserved in the definition.
- **R6.** Update maintains an index only when its key or covered columns change.
- **R7.** The planner gets estimates for a table, an index, and a key range, and per-column distribution statistics,
  without a later change of page format "for performance".

## Options compared

### Key encoding

| System | Integers | Floats | Strings | Composite / null / desc |
|---|---|---|---|---|
| FoundationDB tuple layer (**chosen**) | type code + variable-length big-endian (the code carries the length), negatives one's complement | type code + IEEE with sign twiddle | `00`-escaped as `00 FF`, terminated `00` | Typed elements, each self-describing; no descending in the specification; null is the type code `00` |
| CockroachDB | varint with type markers; ordered | sign-flip float64 | `00` escaped `00 FF`, terminated `00 01` | Per-column direction by inverting bytes; NULL marker byte per column (asc `00`, desc `FF`) |
| MySQL/MyRocks memcomparable | fixed width, sign bit flipped | sign flip / invert | 8-byte groups plus a count byte (binary collations only for indexed columns); other collations as padded chunks | Nullable marker byte; reverse-order column families |
| Voron (RavenDB) | not typed: the application builds the `Slice` | none | none | `IndexEntryKeyGenerator` is a marker attribute only (`IndexEntryKeyGenerator.cs:1-9`); a fixed-size index is a single `long` |

Voron has no typed encoding; every RavenDB caller builds its own bytes. Krummi must supply one because tables are typed
and the planner must reason about order. Two families fit: CockroachDB's own rules (escape `00 FF`, terminator
`00 01`, per-column inversion, marker byte) with MyRocks-style fixed-width numbers, which this document proposed in
its first draft, and the FoundationDB tuple layer. The user chose the tuple layer: it is a published standard with
implementations in other languages (C# among them), which can serve as a test oracle for the Rust encoder and for the
C# binding, and it is compared with `memcmp` as today. The comparison below is the record of why the alternatives
were not taken.

Variable-length parts, two ways:

- **Length prefix.** Rejected: "ab" (length 2) would sort after "b" (length 1) under byte order, and a prefix scan
  by value prefix (`LIKE 'ab%'`) cannot be expressed.
- **Escape plus terminator.** Chosen (both FoundationDB and CockroachDB). It keeps order and keeps a string prefix a
  byte prefix, at 2 bytes per string (type code and terminator) plus 1 per embedded `00`.

The terminator: FoundationDB's bare `00` is correct. The first draft claimed that a bare `00` breaks order when the
next column can start with `FF` (the string "a" followed by an `FF` byte would read as the escaped "a\0"), which is
true for untyped concatenation. It does not apply here: in a tuple **every element starts with a type code, and no
type code is `FF`**, so after a `00` terminator the next byte is never `FF` and "a" < "a\0" holds whatever follows
(fact-check 3.5). CockroachDB's `00 01` terminator solves the same problem for keys whose next column has no type
code. The argument fails wherever the byte after a terminator can be the second byte of an escape: after an ascending
element, a byte `FF`; after an inverted (descending) element, a byte `00`. Descending columns below avoids both.

Integers, fixed width against variable length. The first draft rejected variable-length integers because "varints
need a decode to compare columns". That is true of LEB128-style varints but not of the tuple layer's integers: the
type code carries the byte count and orders by it, so `memcmp` compares them correctly with no decoding. The cost is
that a key's size depends on its values (a `u64` takes 1 to 9 bytes), which helps small values and is checked
against the 2,048-byte limit at write. Krummi's branch cells store full keys (`node.rs:19-21`), so shorter keys also
raise the fan-out. Reading a key is sequential: to reach column k the decoder walks the elements before it (document
1's row format, by contrast, reaches any column in O(1)).

### What the index entry points to

| Option | Lookup after index | Index cost on row move | Index cost on PK change | Key size cost |
|---|---|---|---|---|
| **Primary key bytes (clustered table, as InnoDB, SQL Server, CockroachDB) — decided** | one descent in the table tree | none | every index entry rewritten | pk length in every entry |
| Row id (heap; as Voron, PostgreSQL ctid) — rejected, see document 2 | fetch by id | none if the id is stable | none | 8 bytes |

Voron is the second: its primary key tree maps key to a `long` row id, and a secondary index maps each key to a
fixed-size tree of row ids (`Table.cs:940-964`, `UpdateValuesFromIndex`). Krummi follows SQL Server instead
(document 2): the locator of a row is its encoded primary key, `tuple(primary key)`.

### Index structure in Krummi trees

| Option | Notes |
|---|---|
| A. Multivalue tree: key = columns, values = primary keys (Voron's shape) | A key's values are an ordered set in the entry (0–2,048 bytes each, never off-leaf), or a subtree above the thresholds of M2 ([trees-multivalue.prd.md:41](../../prd/krummi/trees-multivalue.prd.md)). The API gives "number of values of one key", exact and O(1) after one descent, but "no total of values", and the tree count is the number of keys (M6). Counting a range of keys needs a walk. |
| B. Normal tree: key = `tuple(columns) ‖ tuple(pk)`, value = covering columns (**decided**) | One entry per row; entries with equal columns sort by primary key. Duplicates of the columns are stored once per row, by key bytes (4-byte cell header and 2-byte slot, about 6 bytes of cell overhead, per `node.rs:16`). |
| C. Fixed-size tree (u64 key, fixed value up to 2,048 bytes) | Not applicable: the key is `columns ‖ pk`, not a `u64`. |

B wins because a multivalue tree saves only the repeated column bytes (small for the low-cardinality columns where
duplicates are common), adds the two-representation machinery to every index, and cannot hold covering columns per row.
B also matches CockroachDB, FoundationDB, MyRocks, InnoDB and SQL Server's nonclustered indexes. A's per-key value count is
an exact equality count on a non-unique index; B gets the equality estimate from the histogram or the boundary
descent instead (statistics below).

### Statistics for a planner

| Approach | Exact? | Write cost | Format change | Used by |
|---|---|---|---|---|
| Entry count per tree | exact | O(1), already kept (A5, [trees-keys.prd.md:40](../../prd/krummi/trees-keys.prd.md)); a count of the whole tree only | none | Voron `TableSchemaStats.NumberOfEntries` (`TableSchemaStats.cs:5-12`) |
| Order-statistic B+ tree (count in every branch cell) | exact range counts, O(height) | counts in each ancestor change on every write | branch cell +8 bytes | not chosen |
| Descent estimate (count children between two boundary paths) | exact within one leaf, otherwise approximate | none | none | "index dive" in MySQL (`eq_range_index_dive_limit`) |
| Sampled statistics: row counts, distinct counts, most common values, histograms (SQLite `sqlite_stat1/4`, PostgreSQL `pg_statistic`, SQL Server statistics, CockroachDB histograms) | approximate | none online; sampling cost when built | a stats tree | all four |

Voron keeps only the entry count and the overflow page count (`TableSchemaStats.cs:5-12`) and has no planner.

## Decided design

### Key encoding, per type

A key is the concatenation of its elements, in column order (index columns, then the primary key columns for a
secondary index). Each element is a **type code** followed by its bytes. Order below is ascending and byte-wise. The
codes are those of the tuple specification and were verified against
[tuple.md](https://github.com/apple/foundationdb/blob/main/design/tuple.md) on 2026-11-09 (null `00`, byte string
`01`, unicode string `02`, nested tuple `05`, integers `0C`-`1C` with `14` for zero, float `20`, double `21`, false `26`,
true `27`, UUID `30`, user codes `40`-`4F`). Krummi does not use the nested tuple (`05`, whose inner nulls are written
`00 FF`), the arbitrary-precision integers (`0A`/`0B`, `1D`/`1E`, reserved in the specification; Python and Java use
`0B` and `1D` for 9 to 255 bytes) or the versionstamps (`32`, `33`). The `40`-`4F` range "may be used by third
party extenders without coordinating with us", and the specification advises opening a directory with a specific layer
name to avoid conflicts.

| Krummi type | Element | Order rule |
|---|---|---|
| NULL (any nullable column) | `00` (ascending, nulls first); see Nulls for the other three | before every value of any column (nulls first); see Nulls |
| bool | `26` false, `27` true | false < true |
| i8–i64, u8–u64 | integer: `14` for 0; positive: code `14 + n` followed by the n significant bytes big-endian (n = 1..8, codes `15`–`1C`); negative: code `14 − n` followed by the n bytes of the one's complement of the absolute value (codes `13` down to `0C`) | numeric. All eight widths use the same encoding; a `u64` above `i64::MAX` takes 8 bytes (`1C`). |
| f32 | `20` + 4 bytes: IEEE bits big-endian; if the sign bit is set invert all bits, else flip the sign bit | `-inf` < negatives < `0` < positives < `+inf` < NaN |
| f64 | `21` + 8 bytes, same rule | same |
| date (i32 days), time (i64 µs), timestamp (i64 µs, UTC), duration (i64 µs) | integer as above | chronological / numeric |
| decimal(p, s) | user code `40` + 16 bytes: the scaled i128 with its top bit flipped, big-endian | numeric; all values of a column share the scale |
| uuid | `30` + 16 bytes: the RFC 9562 bytes, or the SQL Server permutation per column option (below) | byte-wise (RFC order) or SQL Server order |
| ulid | `30` + 16 bytes big-endian (48-bit Unix milliseconds, 80-bit randomness) | equals the order of its Crockford text form |
| string | `02` + UTF-8 bytes, each `00` replaced by `00 FF`, then `00` | code point order (UTF-8 byte order equals it) |
| bytes | `01` + bytes, each `00` replaced by `00 FF`, then `00` | byte-wise |
| vector | not indexable | n/a |

Codes Krummi reserves in the tuple layer's user range `40`–`4F` (fixed 2026-11-10 after an external review; the
first draft let `decimal` take any code of the range, which could have collided with the null below):

| Code | Use |
|---|---|
| `40` | `decimal` |
| `41`–`4E` | reserved for future value types; a new type takes the lowest free one |
| `4F` | the null of an ascending column whose nulls sort last; never a value |

So the null `4F` is above every value code; a value code is at most `4E`, which inverts to `B1`, above the
descending nulls-first `B0`; and no value code is `00`, whose inversion `FF` the encoding keeps out of every element
start. A future type must also be self-delimiting, and its encoding is checked by the property tests below before it
is added.

Examples: `5` is `15 05`; `-5` is `13 FA`; `-256` is `12 FE FF`; `0` is `14`; the string "ab" is `02 61 62 00`; "a\0b" is
`02 61 00 FF 62 00`. `ulid` shares the UUID code because it is 16 raw bytes with the time first, and a column has one
type, so the two never mix in a position.

Floating point rules (applied to rows at write, so rows and keys agree; decided):

- `-0.0` is stored as `+0.0`, because SQL equality says they are equal; keeping both would make `x = 0` miss half the
  zeros.
- All NaNs are stored as one canonical positive quiet NaN, which sorts above `+inf` and equals itself, PostgreSQL's
  rule. Without it every NaN payload would be a distinct key.

Nulls:

- A not-null column, and every primary key column, has no null element. Each index column chooses whether its nulls
  come first (the default) or last in the forward order of the index, whatever its direction (N5 of the
  [tables PRD](../../prd/krummi/tables.prd.md); decided: "nulls first by default, first or last per index column").
  The schema says which. A nullable column's NULL is:

  | Direction | Nulls first | Nulls last |
  |---|---|---|
  | ascending | `00`, below every type code | `4F`, the top user code, above every code any type uses (`decimal`'s `40` included) |
  | descending | `B0` (`4F` inverted), below every inverted type code (a value code is at most `4E`, which inverts to `B1`) | `FE FF FF`, above every inverted element (below) |

- Correction of 2026-11-10: this document first said that a descending column puts its nulls last by default, as
  PostgreSQL does. N5 fixes nulls first by default in either direction, so the default of a descending column is
  `B0`. No byte above every inverted element except `FF` is free, and `FF` is the escape byte (Descending columns);
  hence the three bytes of the descending nulls-last element.

Descending columns:

- Invert every byte of the column's element: type code, content and terminator. A descending string or byte string
  is terminated by `00 01` before the inversion (`FF FE` after it), CockroachDB's descending terminator; its escape
  stays `00 FF` (`FF 00` inverted). "a" descending is `FD 9E FF FE` and "ab" is `FD 9E 9D FF FE`; the third byte `FF`
  against `9D` puts "a" after "ab", as descending requires. This is the one place where a key is not self-describing:
  the schema says which columns are inverted.
- Every combination of direction and null order is allowed (N5). The first draft of this section rejected one, and
  it missed a second that broke too; both came from the byte after a bare terminator reading as the second byte of
  an escape:
  - an ascending string or bytes column followed by a descending column with nulls last: the null was `FF`, so
    ("a", NULL) read as "a\0" and sorted after ("a\0", x);
  - a descending string or bytes column followed by an ascending nullable column with nulls first, the default:
    "a" ended in `FF` and the null `00` read as the inverted escape `FF 00`, so ("a", NULL) sorted before
    ("a\0b", NULL) where descending puts it after.

  With `FF FE` as the descending terminator, the byte after an inverted escape's `FF` is `00` and after its
  terminator's `FF` is `FE`, so the following element no longer matters; with `FE FF FF` as the descending null, no
  element starts with `FF`, so the byte after an ascending terminator is never an escape. The cost is one byte per
  descending string or byte string and two per descending null that sorts last.
- Rule that follows and that the range bounds below rely on: **no element starts with `FF`**, and an element is
  never followed by a byte `FF` unless it is an ascending string or byte string whose escaped content goes on.
- Checked on 2026-11-10 by scripts outside the repository, against a typed model of N5:
  - first, every shape of one and two index columns of types `string`, `bytes` and an integer, each direction and
    null order, followed by an integer primary key: the first draft's encoding broke both cases above, this one gave
    the same order in all 156 shapes;
  - then, after an external review asked for the whole domain, every indexable type (`bool`, signed and unsigned
    64-bit integers with their limits, `f32` and `f64` with infinities, subnormals and NaN, `decimal` at ±(10^38 − 1),
    `uuid` with `00` and `FF` bytes, `string` with `00`, `é` and U+10FFFF, `bytes` with `00` and `FF` runs): all 2,916
    pairs of consecutive column specifications (type, direction, nullability, null order) with every value of each,
    and 300 random keys of three or four index columns and a primary key of one or two columns of any type. For
    each it checked the order, that every key decodes back to its row with the schema alone, the range bounds below
    (every bound on the first and second column, and `IS NULL`), the uniqueness probe and `LIKE` prefixes, by
    comparing the exact set of rows inside the byte bounds with the set the model selects: 0 failures.
  - The check was itself checked with mutants, each a deliberate defect that must produce failures: a descending
    null that sorts last as `FF` (the first draft) or as a bare `FE`, a descending bare terminator (the first draft),
    the ascending nulls-last code moved onto `decimal`'s `40`, an inclusive upper bound without `FF` (the first
    draft), a uniqueness probe by byte prefix, negative integers without the one's complement, and floats with only
    the sign bit flipped: all 8 were caught. A first version of the check, with random rows only, missed the
    descending terminator; the exhaustive pairs exist because of it.
  - 7.1 and 7.2 turn this into property tests in Rust through the public API (AC-N1 and AC-N7): random rows, keys of
    one to four columns of every indexable type, extreme values and keys at the size limit, against a reference
    model of N5. The scripts are a design check, not those tests.
- A descending index whose columns are all descending is not needed: the tree scans backwards. Inversion exists for
  mixed directions, such as `(country ASC, created DESC)`. Primary key columns are ascending.

Strings and collation:

- Binary UTF-8 order only (decided: "binary order, no collation"). It is deterministic, locale-free and stable across
  platforms and library upgrades; collation-dependent sort keys have caused silently corrupt indexes in systems that
  changed their collation library (PostgreSQL with glibc).
- Case-insensitive lookups are an expression index later (index the lower-cased or case-folded value; document 4's
  AST supplies the expression). No normalisation (NFC) is applied by the engine.

UUID and ULID:

- Both are 16 bytes. The row stores them in RFC 9562 byte order; the key encoding is where order is chosen.
- **UUID order per column** (decided): by default the key carries the RFC 9562 bytes (for v7 this is time order). A
  column option selects SQL Server's `uniqueidentifier` order, for data generated by `NEWSEQUENTIALID()` or by a
  sequential GUID generator that targets SQL Server: the 16 bytes are permuted before encoding so that `memcmp` yields
  that order, and unpermuted on decode.
- **Which byte numbers** (verified 2026-11-09, see `verification.md`, "Follow-up checks"): SQL Server compares
  a `uniqueidentifier` by the bytes {10-15}, then {8-9}, then {6-7}, then {4-5}, then {0-3} of the
  `Guid.ToByteArray()` layout (the layout SQL Server stores), where the first three fields are little-endian;
  `SqlGuid` in dotnet/runtime has `rgiGuidOrder = [10, 11, 12, 13, 14, 15, 8, 9, 6, 7, 4, 5, 0, 1, 2, 3]`. Calling
  `r0..r15` the RFC 9562 bytes, `ToByteArray()` is `r3 r2 r1 r0 r5 r4 r7 r6 r8 .. r15`, so the bytes of the key,
  most significant first, are `r10..r15, r8 r9, r7 r6, r5 r4, r3 r2 r1 r0`. Applying the same numbers directly to
  the RFC bytes (`r10..r15, r8 r9, r6 r7, r4 r5, r0..r3`) is wrong. The PRD still pins the permutation with test
  vectors taken from SQL Server.
- `ulid` is its own type (decided), 16 bytes with the time first, generated monotonically (48 bits of Unix
  milliseconds, 80 random bits incremented within a millisecond) by a generator the product provides.
- A random UUID (v4) used as a primary key or index key sends every insert to a random leaf; a time-ordered key (ULID,
  v7, sequential GUID) appends to the last leaf and fills it (the append split, document 2). Krummi's own
  sequential-against-random load in A15 ([trees-keys.prd.md:50](../../prd/krummi/trees-keys.prd.md)) is the way to
  quantify it. The engine accepts v4 and documents the cost.

Composite keys and prefix scans:

- Concatenation needs no separator between elements because each element is self-delimiting: the type code fixes the
  width of fixed-width ones, and strings and byte strings end at the terminator.
- An equality on the first k columns is the byte prefix `P` of their encodings, but a byte prefix of an ascending
  string or byte string also matches longer values ("a" is `02 61 00`, a prefix of "a\0", `02 61 00 FF 00`). The
  entries whose column k equals v are therefore the byte range `[P ‖ enc(v), P ‖ enc(v) ‖ FF)`: every entry with that
  value continues with another element or ends, and no element starts with `FF`, while a longer string continues with
  its escape, `FF`. This is the tuple layer's own `range(tuple)` end.
- Corrected 2026-11-10: the first draft wrote a range on column k+1 as `[P ‖ enc(lo), P ‖ enc(hi)]`, which loses the
  entries equal to `hi`, since each continues with the primary key and sorts after `P ‖ enc(hi)`, and said "the
  successor of the encoded prefix" for an exclusive bound, which is wrong for strings for the reason above. A range
  on column k+1, with `[start, end)` as byte bounds:

  | Bound on the values | Ascending column | Descending column |
  |---|---|---|
  | `lo` inclusive | `start = P ‖ enc(lo)` | `end = P ‖ enc(lo) ‖ FF` |
  | `lo` exclusive | `start = P ‖ enc(lo) ‖ FF` | `end = P ‖ enc(lo)` |
  | `hi` inclusive | `end = P ‖ enc(hi) ‖ FF` | `start = P ‖ enc(hi)` |
  | `hi` exclusive | `end = P ‖ enc(hi)` | `start = P ‖ enc(hi) ‖ FF` |
  | no bound at the low byte end | `start = P ‖ null ‖ FF` when the nulls come first, else `P` | same |
  | no bound at the high byte end | `end = P ‖ null` when the nulls come last, else `P ‖ FF` | same |

  `null` is the column's null element; a not-null column has no null and uses `P` and `P ‖ FF`.

  A bound of the form `B ‖ FF` can be one byte longer than the 2,048-byte maximum key, which the tree's range API
  rejects (corrected 2026-11-10 after the Codex review, R5: a `bytes` primary key of 2,046 bytes without `00` encodes in
  exactly 2,048 bytes, so `enc(v) ‖ FF` is 2,049). When `B` itself is 2,048 bytes, no stored key extends it, so the
  keys at or after `B ‖ FF` are exactly the keys after `B`: a start `B ‖ FF` becomes `B` excluded, and an end `B ‖ FF`
  (excluded) becomes `B` included. Every bound passed to the tree is therefore at most 2,048 bytes, and the tree's
  included and excluded bounds express the same set of keys. A comparison with
  a value never matches a null, so no range includes the nulls; `IS NULL` is the range
  `[P ‖ null, P ‖ null ‖ FF)`. A backward scan reads the same byte range in reverse. The same bounds serve the
  primary key of 7.1: a range on a prefix of a composite primary key, or on a whole one, where the table tree's key
  is `enc(pk)` with nothing after it and the bounds above still hold.
- A string-prefix match (`name LIKE 'ab%'`) uses the type code and the escaped bytes of "ab" without the terminator,
  `Q = P ‖ 02 61 62` (inverted for a descending column), and the range `[Q, successor(Q))`, where the successor drops
  the trailing `FF` bytes of `Q` and increments its last byte (FoundationDB's `strinc`; `Q` starts with a type code,
  so it is never all `FF`).
- The key can be decoded back to values by walking its elements, so an index-only scan needs no row read, with the
  float canonicalisation above as the only information the key loses (and it loses nothing the row keeps).

Size limit:

- The maximum key is 2,048 bytes (B5). The index key is `tuple(columns) ‖ tuple(pk)`, so
  `len(columns) + len(pk) ≤ 2,048` for secondary indexes and `len(pk) ≤ 2,048` for the primary key. Escaping can grow
  a string by one byte per `00`; a `u64` takes up to 9 bytes, a `uuid`, `ulid` or `decimal` 17. A descending string
  or byte string takes one byte more than an ascending one (its two-byte terminator), and a descending null that sorts
  last takes three bytes (2026-11-10); the size check and the definition-time check below count them.
- If an entry exceeds it, the insert or update fails with a typed error naming the index, before any change (decided).
  At definition time, indexes over a vector are rejected, as are indexes over a column whose declared maximum already
  cannot fit.
- The leaf and branch structures already guarantee two entries per leaf and three keys per branch with the longest
  key (`node.rs:44-55`), so an at-limit key is safe, only wasteful.

### Index structure

**Secondary index** (one tree per index, named by the catalog):

- Key: `tuple(c1, ..., cn) ‖ tuple(pk columns)`. The primary key part is last and needs no terminator because its
  elements are self-delimiting.
- Value: the encoded included (covering) columns in document 1's row format, or empty. Values larger than half a
  leaf go off-leaf by the existing large-value rule (`node.rs:16-19`); an index with many wide included columns is a
  warning case. Covering columns are in the first version (decided).
- Entries with equal columns are ordered by primary key. A scan returns rows in primary-key order within each value.

**Unique index.** Same key, with one more rule: before inserting, the writer seeks the first entry at or after
`tuple(columns)` and fails with a unique violation if that entry lies in `[tuple(columns), tuple(columns) ‖ FF)`, the
entries whose columns equal the new ones (Composite keys and prefix scans). A plain byte-prefix test is wrong
(corrected 2026-11-10): `enc("a")` is a byte prefix of `enc("a\0")`, so inserting "a" would fail when "a\0" exists.
On an update, an entry in that range whose primary key is the row's own is not a violation. The seek reaches the
leaf where the insert goes, so the probe adds a key comparison with a neighbour, not a descent. **Nulls are distinct
from each other** (decided, the SQL standard and PostgreSQL and SQLite, not SQL Server): entries with any NULL column
are never compared, so many NULLs are allowed. Using the same layout for unique and non-unique indexes keeps one code
path for scans, estimates and maintenance.

Considered on 2026-11-10 and kept for 7.2: SQL Server stores the clustering key only in the leaves of a *unique*
nonclustered index, as an included column, and in every level of a non-unique one, as part of the key ("Is the
Entire Clustering Key Duplicated in the Nonclustered Indexes?", ITPro Today, fetched 2026-11-10:
https://itprotoday.com/sql-server/is-the-entire-clustering-key-duplicated-in-the-nonclustered-indexes-). Krummi's layout is
SQL Server's non-unique one for both. The unique alternative, key `tuple(columns)` and value `tuple(pk) ‖ included`,
gives shorter separators in branches, an exact lookup as the uniqueness probe, and a value replace when the primary
key changes; the leaves hold about the same bytes, since the primary key only moves from key to value. But nulls are
distinct here, so an entry with a NULL column still needs the primary key in its key, and the tree would mix two
layouts that every scan and decode must tell apart. Branches are about one page in every fan-out pages, so the gain
is small; it
is an optimization to measure with 7.2's bench loads (N11), not a first-version layout.

**Primary key.** The table tree is the primary index, with key = `tuple(pk columns)` and value = the row without those
columns (document 2). The primary key is mandatory and its columns are never null (decided).

**Multi-column indexes.** No limit on columns other than the key size. The planner can use any prefix of the columns.

**Covering columns.** The definition has an `include` list from the first version. The cost is write maintenance when
an included column changes (below), and size. An index-only scan is possible when every column a query needs is a key
column, a primary key column or an included one.

**Partial indexes.** The definition reserves a `filter` field (a predicate over the row's columns). An entry exists
only while the predicate is true; maintenance evaluates it on the old and new row. It is built after document 4's AST
exists (decided); the planner may use the index only when the query's predicate syntactically implies the filter.

**Fixed-size indexes.** Not used for secondary indexes: their key is `columns ‖ pk`, not a `u64`. (Voron uses a
fixed-size tree of `long` row ids under each key, `TableSchema.IndexDefinitions.cs:529-556` and `Table.cs:958-961`;
document 2 has no row ids.)

**Maintenance on update.**

- The update knows which columns it assigns. Only indexes whose key columns, included columns or filter columns
  intersect that set are examined; for them, the old and new encoded key and value are compared and the index is
  touched only if they differ (the same test Voron makes with `SliceComparer.AreEqual` on old and new
  index values, `Table.cs:937-958`).
- When the key changes, the index entry is deleted and a new one inserted. When only the included value changes,
  the entry is replaced in place.
- A primary-key change rewrites every secondary entry (the primary key is in each key); decided as an accepted cost,
  because primary keys rarely change. A delete encodes each index key from the old row, which the delete already read.
- All of this happens in the one write transaction, so a row and its index entries commit together.

### Statistics

Keep three things, in this order of cost. The planner works on **estimates**; there are no counts in branch nodes
(decided: "Estimaciones, los histogramas no necesitan ser exactos ni estar siempre al día, pero tienen que existir
incluso si no hay índices para las columnas, algo como hace SQL Server con las estadísticas").

1. **Counts that exist or are free.** The table row count (the table tree's entry count, O(1) per A5) and each
   index's entry count (also O(1); for a partial index it is the number of matching rows). Voron's equivalent is
   `NumberOfEntries` (`TableSchemaStats.cs:5-12`, updated at `Table.cs:756-762`). These are the only exact counts, and
   they count whole trees; there is no exact count of a range.
2. **A range estimate by descent, no stored state.** The tree gains one read operation `estimate_range(lo, hi)` that
   descends the lower and upper boundary paths together. Entries in the first and last leaf are counted exactly; the
   leaves between are counted from the branch cell positions at the lowest level where the paths diverge and extended
   upward by the tree's average fan-out. The result is `{rows, exact}`: exact when both boundaries land in one leaf
   (equality on a unique index, narrow ranges), otherwise an estimate whose error is about one leaf's fill variation
   per divergent level. Cost: two descents of 3-4 pages, normally in cache. It changes no page format. The tree has
   no rank structure (`node.rs`, `lib.rs`), so this operation does not exist yet.
3. **Per-column statistics, kept in the table's `stats` tree, for every column whether or not it has an index.** Per
   column: the row count and sample size at build time, the null fraction, an estimate of the number of distinct
   values, a histogram of equi-depth buckets (upper bounds as the column's encoded element bytes, so one histogram
   code serves every type and compares with `memcmp`) and a short list of most common values with their frequencies.
   Per index: the number of distinct prefixes for each prefix length (SQLite's `sqlite_stat1`, "average rows per key
   for the first k columns"). The numbers of buckets and common values are constants to measure (proposal: 100 and
   10), and a string bound is truncated to a fixed length (proposal: 32 bytes). Vector columns have none.

**Created and refreshed automatically, in the background** (decided, like SQL Server's asynchronous mode):

- A modification counter per table, in the table's `meta` tree (third review round, 2026-11-10: H7 and H8). It is
  table data like any other: each operation that inserts, updates or deletes rows adds to it in its write
  transaction's own view, so it commits with the rows it counts, an abort discards it, and every snapshot reads the
  counter of exactly the rows it sees (MVCC). It never decreases: nothing resets it, and an operation that removes
  many rows (a range delete, emptying the table) adds the number of rows it removed, since counting it as one would
  leave the statistics of a table that lost all its rows looking fresh. It survives closing the store, and a backup
  carries it with the statistics it measures. Cost: one more dirty page in a transaction that changes the table;
  measure it (N11). The first draft allowed a counter kept in memory as a fallback; that fallback is withdrawn,
  because a counter that restarts at zero cannot tell how old the statistics are: built at 600, restarted at 0, and
  601 modifications later the difference reads 1 (counterexample of the review's H7). If the measurement rules out
  the persistent counter, 7.3 must replace it with one that keeps both properties (never decreasing, visible per
  snapshot), not with an in-memory one.
- A query whose planner finds a column's statistics missing or stale plans with a **default estimate** and asks a
  thread of the store to build them. The thread builds them from a sample with a read transaction and stores them with
  a short write. **No query waits for sampling.** At most one job per table is queued; closing the store stops the
  thread and drops the queue.
- Statistics sampled from a snapshot describe that snapshot (corrected 2026-11-10 after an external review). Each
  column's statistics keep the table's modification counter, its row count and the schema version of the snapshot
  they were sampled from, not those of the write that stores them, so the modifications made while sampling count
  toward staleness.
- Newer statistics are never replaced by older ones (N8), by comparing counters: the write that stores a build
  compares the build's counter with the counter recorded with the column's stored statistics, if any, and drops the
  build if the stored counter is higher. Since the counter never decreases and each snapshot reads its own, a higher
  counter means a later snapshot; equal counters mean the same rows, so either result is right. Builds need no other
  ordering: they may overlap, and none waits for another. (The second round of this correction serialized builds
  instead; the review's H8 showed that an `analyze` waiting for a running build while it holds the write turn
  deadlocks with that build, which needs the turn to store its result. With the persistent counter, the comparison
  is sound again and the serialization is withdrawn.)
- The write that stores a build also drops it if its table is no longer the same table (dropped and recreated, B9)
  or the column no longer exists, identified by its column id, not its name, so a rename keeps its statistics.
- One lock order, the write turn last: sampling reads a snapshot and holds nothing; storing takes the write turn
  like any writer (T4) and holds nothing else; no build ever waits for another build. `analyze` from a read
  transaction, or outside one, samples a new snapshot and then stores with its own short write, waiting for the turn
  like any writer (T4); a thread that already holds the write transaction gets T5's misuse error at once, without
  waiting, as for any second write of the same thread, so it never hangs on itself. `analyze`
  in an open write transaction samples what that transaction sees, its own changes included, and stores within it,
  with the counter of its own view; it takes no other turn, and a background build from an older snapshot that stores
  later is dropped by the counter comparison. Which of these forms 7.3 offers is its decision (the PRD lists where
  `analyze` can be asked for); each form keeps this order.
- Tests: an `analyze` and a background build of the same table overlapping, finishing in both orders, with the newer
  result kept; `analyze` inside a write transaction that changed rows while a background build of an older snapshot
  is pending, without deadlock and with the transaction's statistics kept; a column removed or renamed and a table
  dropped and recreated while sampling (7.3 fixes what happens to the statistics of a removed column); writes during
  the sample counted after it; statistics built, the store closed and reopened, and more modifications than the
  threshold, then stale; and emptying a table of more rows than the threshold makes its statistics stale. Added in
  the fourth review round (H18), all in AC-N8: a write transaction that changes rows, runs `analyze` and aborts
  leaves rows, counter and statistics as before; one that runs `analyze`, changes more rows and commits keeps the
  counter of the moment of `analyze` with the statistics, so the later changes count toward staleness; and a bulk
  load that fails while a background build samples leaves in the stored statistics only the rows of the load that
  were committed, as 7.1 fixes: none if the load fails whole, since its rows were never committed and no snapshot
  sees them (corrected 2026-11-10 after the Codex review, R1: the first wording assumed the whole-load rule that 7.1
  has not decided yet).
- "Stale" means the modification counter since the build exceeds SQL Server's dynamic threshold, as N8 fixes: 500 if
  the table had up to 500 rows at build, otherwise the lesser of 500 + 20 % of the rows and √(1000 × rows) (README,
  correction of 2026-11-09). Corrected 2026-11-10: this line still gave the first draft's proposal, the larger of 500
  and 20 % of the rows, the older SQL Server rule.
- The sample: a table up to the sample size (proposal: 30,000 rows) is read whole; a larger one is sampled by
  reading randomly chosen leaves in full, since the clustered table is ordered by the primary key. For a column that
  leads an index, the equi-depth bounds come from an ordered stride walk of the index instead, which needs no random
  sampling.
- An explicit **`analyze`** (a table, or one column) forces the same build, synchronously, in the caller's thread.
- Defaults when nothing exists (constants in the planner's `CostParams`, document 4): equality on an indexed column
  about 10 rows, as SQLite assumes; for an unindexed column or a range, fixed fractions of the row count (PostgreSQL
  uses 0.5 % for equality and a third for an inequality, from knowledge, not fetched here).

The statistics need be neither exact nor current. A stale histogram gives a worse plan, never a wrong result.

**Why not order-statistic counts.** Putting a count in every branch cell would give exact range counts in O(height)
(a subtraction of two ranks), but it was refused with the choice of estimates. Estimated costs, not measured:

- Branch cell: `2 + key + 8` bytes (`node.rs:44`); adding an 8-byte count adds 31 % to a 16-byte-key cell and 16 % to a
  40-byte one, so fan-out falls by the same order (about 290 to 230 children for 16-byte keys, 8 KiB pages,
  `format.rs:33`).
- Write amplification: counts in each ancestor change on every insert and delete, enlarging the journal page diffs by
  a few bytes per level, and complexity: counts must be correct across split, merge, empty-child removal, large-value
  pages, the integrity checks and the node-check cache; a wrong count is a silent planner error, not a crash.
- It would need a format version bump (the format version is `10` today, `FORMAT_VERSION` in
  `crates/krummi-pager/src/format.rs:41`), which Krummi has done before, so declining it is not a lock-in: the count is
  derivable from the data and can be rebuilt on upgrade.

## Costs and risks

- **Key bytes per entry.** `tuple(columns) ‖ tuple(pk)` repeats the primary key in each secondary index. A 16-byte ULID
  (17 bytes as an element) over four indexes costs 68 bytes per row beyond the columns; with a long composite primary
  key this is the main cost of the clustered layout. Cell overhead is about 6 bytes (`node.rs:16`). An 8-byte
  auto-increment key takes 1 to 9 bytes as an element.
- **One type-code byte per element**, against the first draft's fixed-width numbers without one; integers shrink for
  small values. Net size is data-dependent and measured in the PRD.
- **Extensions beyond the specification.** `ulid` and the SQL Server permutation reuse or reorder the UUID element,
  `decimal` and nulls-last use user codes, and a descending column is not decodable without the schema. A stock tuple
  library decodes the standard elements only; it is a test oracle for them, not for the extensions, which need
  golden vectors of their own.
- **Escaping expansion.** A binary string full of zeros grows by 100 %. Uncommon in text, common in raw bytes;
  bytes columns that hold random data should not be indexed (or only through a hash column).
- **UUID v4 keys** fragment trees and amplify writes (see above). The engine cannot fix it without changing the key.
- **No collation.** Users expecting case-insensitive or locale order will find binary order surprising; the
  expression index is the planned answer, not a first-version feature.
- **Uniqueness probe** is not atomic with the insert in isolation, but Krummi has one writer at a time, so no race
  exists; if concurrent writers are ever added it needs a lock on the prefix. The background statistics write takes
  the write lock briefly; measure its effect on commit latency.
- **Estimate error.** The descent estimate errs for ranges that cover many leaves in a tree with uneven fill (after
  bulk deletes), bounded by average fan-out; histograms err when stale or when predicates are correlated (the
  independence assumption). The planner treats `exact: false` as a range, and the plan explanation shows estimated
  against actual rows (document 4).
- **Index creation on an existing table** requires a scan of the table and a sorted bulk load; its design is for the
  PRD, not here.

## Product questions

1. **Key encoding.** *Decided 2026-11-09:* FoundationDB's tuple encoding with the extensions above. README: "after
   '¿Tú que opinas?': 'Sí'". The first draft recommended CockroachDB-style rules and fixed-width numbers.
2. **Where do NULLs sort?** *Decided 2026-11-09:* first by default, first or last per index column (README: "nulls
   first by default, first or last per index column"). This was recommendation (c).
3. **Are NULLs distinct in a unique index?** *Decided 2026-11-09:* distinct, many NULLs allowed (README: "as in standard
   SQL (not SQL Server)").
4. **What happens when an index key exceeds 2,048 bytes?** *Decided 2026-11-09:* reject with a typed error before
   anything changes (README).
5. **String collation.** *Decided 2026-11-09:* binary UTF-8 order only, no collation; case-insensitive search is an
   expression index later (README: "`string` (UTF-8, binary order, no collation)"; case-insensitive collation is out
   of the first version).
6. **Float canonicalisation.** *Decided 2026-11-09:* rows and keys both store `-0` as `+0` and one canonical NaN
   sorting after `+∞` (README).
7. **Exact range counts in O(log n)?** *Decided 2026-11-09:* no; estimates for the planner and a scan for an exact
   `COUNT` (README: "no counts in branch nodes").
8. **Covering and partial indexes.** *Decided 2026-11-09:* covering (included) columns from the first version; partial
   indexes reserved in the definition and built once the query tree exists (README).
9. **`analyze`: explicit or automatic?** *Decided 2026-11-09:* both: statistics are built and refreshed automatically
   in the background and `analyze` forces them (README).
10. **UUID order.** *Decided 2026-11-09:* one `uuid` type, RFC 9562 bytes by default and SQL Server's order per column
    (README: "Sí, me sirve"). *Settled 2026-11-09:* the permutation is `r10..r15, r8 r9, r7 r6, r5 r4, r3 r2 r1 r0` on the RFC bytes (see UUID and ULID above).
11. **UUID v4 as a primary key.** *Still open, low stakes.* Options: (a) allowed with documentation, (b) allowed but the
    API warns, (c) the API only generates v7/ULID for new keys. Recommendation: (a) for storing, (c) for generation;
    the product's ULID generator and the user's remark that keys are usually auto-increment, ULID or GUID point the
    same way.

## Sources

Repository (paths from `C:\projects\Krummi`):

- `docs/design/tables/README.md` (agreed scope and decisions).
- `docs/prd/krummi/trees.prd.md:45` (B5: keys 1-2,048 bytes, unsigned byte order, prefix before extension, no custom
  comparators), `:46` (B6: values to 256 MiB).
- `docs/prd/krummi/trees-keys.prd.md:40` (A5: entry count without a walk), `:50` (A15 sequential/random load
  measurement), `:66` (the annotation that sets the value maximum to 268,435,456 bytes and supersedes the provisional
  1,024 bytes of A11).
- `docs/prd/krummi/trees-multivalue.prd.md:40-41` (M1 values up to 2,048 bytes; M2 two thresholds), `:45` (M6: count
  is keys; per-key value count; no total of values).
- `docs/prd/krummi/trees-fixed-size.prd.md:40` (X1: values 0-2,048 bytes), `:44` (X5: u64 keys).
- `crates/krummi-tree/src/node.rs:1-22` (page layout, cell format, branch cell with full key), `:44-55` (maximum
  branch cell and fit guarantees); `crates/krummi-pager/src/format.rs:33` (8 KiB pages), `:41` (format version 10).

Voron (`C:\projects\ravendb\src\Voron\Data\Tables`):

- `IndexEntryKeyGenerator.cs:1-9` (a marker attribute only; keys are built by user code).
- `TableSchemaStats.cs:5-12` (`NumberOfEntries`, `OverflowPageCount`).
- `TableSchema.IndexDefinitions.cs:200-215` (`DynamicKeyIndexDef`, `SupportDuplicateKeys`), `:529-556`
  (`FixedSizeKeyIndexDef`, a single `long` column).
- `Table.cs:756-762` (stats update on insert), `:927-964` (`UpdateValuesFromIndex`: compare old and new key, then
  delete and add the row id in the index's fixed-size tree), `:1155-1180` (fixed-size tree of `long` per key).

External (fetched in the fact-check of 2026-11-09 unless marked):

- FoundationDB tuple layer: https://github.com/apple/foundationdb/blob/main/design/tuple.md (strings, integers and the
  full table of type codes verified on 2026-11-09).
- CockroachDB key encoding (`pkg/util/encoding`): https://github.com/cockroachdb/cockroach/blob/master/pkg/util/encoding/encoding.go
- MyRocks / MySQL memcomparable format: https://github.com/facebook/mysql-5.6/wiki/MyRocks-record-format
- SQLite `sqlite_stat1/4`: https://www.sqlite.org/fileformat2.html
- PostgreSQL planner statistics (`pg_statistic`, `n_distinct`, MCV, histograms): https://www.postgresql.org/docs/current/planner-stats.html
  (the null fraction and correlation fields are not on that page)
- MySQL index dives (`eq_range_index_dive_limit`): https://dev.mysql.com/doc/refman/8.0/en/range-optimization.html
- RFC 9562 (UUID, obsoletes RFC 4122; same byte order): https://www.rfc-editor.org/rfc/rfc9562; ULID specification:
  https://github.com/ulid/spec (not fetched).
- SQL Server `uniqueidentifier` comparison order (verified 2026-11-09): https://learn.microsoft.com/en-us/archive/blogs/sqlprogrammability/how-are-guids-compared-in-sql-server-2005
  and `SqlGuid.cs` in dotnet/runtime; statistics thresholds: from knowledge, not fetched.
