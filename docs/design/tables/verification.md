# Fact-check of the tables design documents (01 to 07)

Date of the check: 2026-11-09. Scope: the seven documents in this folder, read as written; none was edited.
Method: external claims were checked against primary sources fetched during this check (official documentation,
source files on GitHub, crates.io and docs.rs); Krummi claims against the repository (read only); Voron claims against
`C:\projects\ravendb\src\Voron` and `Corax` (read only). Nothing was built, tested or benchmarked; no cargo command was run.

Verdicts: **confirmed**, **wrong**, **partly right**, **not verifiable**. "Not verifiable" means that no primary source
was reached in this check (the claim is usually standard knowledge, and the note says so) or that the claim is an
estimate or a forecast. Wrong and partly right verdicts give the correct fact.

## Summary

142 claims checked: 86 confirmed, 13 wrong, 17 partly right, 26 not verifiable (some claims appear in two tables,
for example the format version and T8, so the true number of distinct errors is 11 wrong and 17 partly right).

| Table | Checked | Confirmed | Wrong | Partly right | Not verifiable |
|---|---|---|---|---|---|
| Claims (external systems, crates, and some local facts), docs 1-7 | 97 | 49 | 10 | 13 | 25 |
| Krummi code claims | 22 | 16 | 2 | 3 | 1 |
| Voron claims | 23 | 21 | 1 | 1 | 0 |

The Voron analysis in the documents is the most reliable part: 21 of 23 line-level claims check out. Most "not
verifiable" rows are claims that the documents themselves say were written from memory and that were not fetched here
(SQLite C API, InnoDB, .NET attributes, PostgreSQL `BitmapAnd`, some CockroachDB details); none of them changes a decision.
The last section lists 16 contradictions between the documents, three of which (index structure, table kind and locator, exact
against estimated counts) affect the starting state of the PRDs.

### The five most consequential errors

1. **Doc 6, T8 is not open.** T8 of `transactions.prd.md` is decided: closing the store while a transaction is open
   "no compila" (compile-time rule; `Store::close(self)` consumes the store and transactions borrow it). The engine has
   no runtime rule that `krummi_store_close` could "forward". The MISUSE-and-stay-open behavior is a new decision for the
   C interface, not a pending one; question 8 and the handle section should say so.
2. **Doc 4 is built on a layout that doc 2 rejects.** Doc 4 says the table is "clustered by primary key (doc 2)", that
   primary-key ranges are table ranges, and that secondary entries carry the primary key (covering check). Doc 2
   recommends a heap keyed by an 8-byte row id and its secondary entries hold only the row id. Doc 4 also relies on
   "exact range counts from the B+ tree (doc 3)"; doc 3 says exact range counts do not exist (only O(1) entry count
   per tree and per-key value count) and proposes an approximate `estimate_range`. The tree has no rank or count
   structure (`node.rs`, `lib.rs`), so an exact range count is a leaf walk.
3. **Doc 3 versus doc 2 on the non-unique index.** Doc 2: multi-value tree, value = 8-byte row id. Doc 3: rejects the
   multi-value tree and uses a normal tree with key = columns || locator. Both cannot be the PRD's starting state.
   (Capability check: the multi-value tree gives an O(1) per-key value count, ordered 0-2,048-byte values, no off-leaf
   values, no total of values, so doc 3's argument about range counts is right, but it does not mention that equality
   counts on a non-unique index are exact and O(1) after one descent.)
4. **Doc 5, pgvector indexing limit and `half` hardware conversion.** "4,000 for indexing" is wrong for `vector`: the
   indexing limit is 2,000 dimensions (4,000 is `halfvec`, 64,000 `bit`, 1,000 non-zero elements `sparsevec`). The
   statement that `half` converts in hardware "only with a nightly feature" is wrong for half 2.6 and 2.7.1: F16C is
   used on stable x86-64 with runtime detection under the default `std` feature. Because the `half` argument is the
   reason doc 5 hand-writes the f16 conversion, its cost/benefit should be re-read.
5. **Doc 6, csbindgen emits `DllImport`, not `LibraryImport`.** The README of csbindgen 1.9.x says it generates
   `DllImport` code (with function pointers for callbacks); the sources contain no `LibraryImport` generator. Doc 6
   marks this "to verify" and recommends `LibraryImport` over a csbindgen-generated header; the raw declarations would
   have to be rewritten by hand or by another tool.

Also worth fixing before a PRD: doc 3 says Krummi's format version is `9` (it is `10`, `FORMAT_VERSION` in
`krummi-pager/src/format.rs`); doc 1 says Voron's offsets are relative to the end of the offset table (they are relative
to its start); doc 5 says a 1,536-dim f32 vector "fits a leaf only when the leaf holds one row" (it exceeds the 4,078-byte
leaf cell limit, so it is always off its leaf); doc 1 says "4 KB pages" (Krummi pages are 8 KiB); doc 7 would make the
table layer need lz4_flex's `safe-encode`, which by Cargo feature unification can switch the pager's journal encoder to
the slower safe implementation (the D12 annotation of 2026-11-06 says it is 30-50 % slower).

## Claims

Source key: URLs are primary sources fetched in this check. "knowledge" means not fetched.

### Doc 1: row format and types

| # | Section | Claim | Verdict | Correct fact or note | Source |
|---|---|---|---|---|---|
| 1.1 | Options: SQLite record | Serial types: integers 0,1,2,3,4,6,8 bytes; constants 0 and 1 take no body; text `13+2n`, blob `12+2n`, NULL type 0; header is a varint length plus one serial type per column | confirmed | Constants 0/1 (types 8, 9) exist only for schema format 4 and later. 64-bit float is serial type 7 (8 bytes), not mentioned | https://www.sqlite.org/fileformat2.html |
| 1.2 | Options: SQLite record | "A short record is read as NULL for missing columns" | partly right | Missing trailing values take the column's default value from the schema (NULL only when there is no default) | https://www.sqlite.org/fileformat2.html |
| 1.3 | Options: PostgreSQL tuple | 23-byte header, null bitmap present only if a column is null, `t_hoff` aligned to MAXALIGN; 1 bit per column | confirmed | Header fields in the doc omit `t_infomask2` and the `t_cid`/`t_xvac` overlay but the 23 bytes are right. In the bitmap 1 = not null (the doc's Krummi bitmap uses 1 = null; irrelevant to the comparison) | https://www.postgresql.org/docs/current/storage-page-layout.html |
| 1.4 | Options: PostgreSQL tuple | `attmissingval` makes "add column with default" free | confirmed | Only for non-volatile defaults | https://www.postgresql.org/docs/current/catalog-pg-attribute.html |
| 1.5 | Options: PostgreSQL tuple | `attcacheoff`: fixed columns before the first variable one are O(1) | not verifiable | Internal field, not documented on the pages fetched; matches known PostgreSQL behavior (knowledge) | none |
| 1.6 | Comparison / overhead | PostgreSQL "about 28-30 bytes minimum per row" | confirmed | 23-byte header padded to 24 plus a 4-byte item pointer = 28 | https://www.postgresql.org/docs/current/storage-page-layout.html |
| 1.7 | Options: fixed offsets | "Arrow's row format places a validity bitmap, fixed-width fields at schema-known offsets and an offset/length table" | wrong | The Arrow specification defines only a columnar format. arrow-rs has a "row format" (`arrow-row`) that is byte-comparable (memcmp), made for sorting and grouping, not a fixed-offset row. The described layout resembles Spark `UnsafeRow` | https://docs.rs/arrow-row/latest/arrow_row/ |
| 1.8 | Options: fixed offsets | FlatBuffers vtable, Cap'n Proto data and pointer sections | not verifiable | Not fetched (knowledge; low risk) | https://flatbuffers.dev/internals/ |
| 1.9 | Column types: f16 | Rust `f16` is not stable | confirmed | Nightly-only, tracking issue 116909 | https://doc.rust-lang.org/std/primitive.f16.html |
| 1.10 | Column types: decimal | C# `Int128` is .NET 7+; `decimal` converts "if s <= 28" | partly right | `decimal` has a 96-bit mantissa (about 7.9e28), so an i128 with p up to 38 only converts when the magnitude fits, not by scale alone. `Int128` in .NET 7: not fetched (knowledge) | none fetched |
| 1.11 | Column types: time, uuid | `TimeOnly` and `DateTime` tick = 100 ns; `Guid` is mixed-endian in memory | not verifiable | Not fetched (knowledge; low risk) | none |
| 1.12 | Column types: uuid, ulid | RFC 9562 byte order; ULID = 48-bit ms timestamp + 80-bit random, Crockford base32 | not verifiable | Not fetched (knowledge; low risk) | https://github.com/ulid/spec |
| 1.13 | Reading | `bytemuck::try_cast_slice::<u8,f32>` is safe and fails on misalignment | confirmed | `cast_slice` panics; `try_cast_slice` returns an error. Doc 5 states the same | https://github.com/Lokathor/bytemuck (src/lib.rs) |
| 1.14 | Reading | An unaligned load that does not cross a cache line costs the same as an aligned one on x86-64 and ARM64 | not verifiable | Hardware claim, not measured here | none |
| 1.15 | Costs: max row size | The tree's large-value limit is 256 MiB, "V2 of the trees PRD" | partly right | 256 MiB is right (`MAX_VALUE = 268_435_456`, B6, V1 of the large-values PRD). V2 is the contiguous zero-copy read rule | `docs/prd/krummi/trees-large-values.prd.md` V1 |
| 1.16 | Costs: large vectors | "with 4 KB pages that means one row per page" | wrong | Krummi pages are 8 KiB (`PAGE_SIZE = 8192`); a 3,134-byte row fits two to a leaf | `crates/krummi-pager/src/format.rs:33` |
| 1.17 | Estimated overhead | Typical row: 3 + 1 + 2 = 6 bytes; Voron 11; total 3,134 / 3,139 | confirmed | Arithmetic checked against the layout and `TableValueBuilder` (1 count byte + 5 offsets of 2 bytes) | local |
| 1.18 | Max dimension | `dim` 1..=16,384 proposed | confirmed as a proposal | Conflicts with doc 5 (4,096); see Contradictions | local |

### Doc 2: row storage

| # | Section | Claim | Verdict | Correct fact or note | Source |
|---|---|---|---|---|---|
| 2.1 | Options B | SQLite guide: WITHOUT ROWID suits rows under about 1/20 of a page and large rows are slower | confirmed | Quote: "less than about 1/20th the size of a database page"; "traditional rowid tables tend to work faster for large row sizes" because content is also stored in intermediate nodes | https://www.sqlite.org/withoutrowid.html |
| 2.2 | Options B | "Krummi's branch nodes hold only keys, so that warning applies less" | confirmed | Branch cell = key length, key, child page (8 bytes) (`node.rs:19-21`); SQLite's reason is content in intermediate nodes | https://www.sqlite.org/withoutrowid.html |
| 2.3 | Sources | SQLite rowid tables and InnoDB clustered/secondary indexes | not verifiable | The document says it cites them from knowledge; not re-read here either | https://dev.mysql.com/doc/refman/8.0/en/innodb-index-types.html |
| 2.4 | Options B/C | Estimates: journal about 1.5 KB per row against 0.6 KB; a split one in 8 against one in 130; 63,000 leaves | not verifiable | Derived estimates; the fan-out arithmetic (about 400 children for 8-byte keys, 3 levels for a million rows) is consistent with the node format | local |

### Doc 3: keys and indexes

| # | Section | Claim | Verdict | Correct fact or note | Source |
|---|---|---|---|---|---|
| 3.1 | Options: key encoding | FoundationDB tuple: strings `00`-escaped as `00 FF`, terminated by `00`; type codes; negatives one's complement; no descending | confirmed | Quote: "byte strings are null terminated with null values occurring in the string escaped"; terminator is `00` not followed by `FF`; negative integers "big endian one's complement"; the spec says nothing about descending order | https://github.com/apple/foundationdb/blob/main/design/tuple.md |
| 3.2 | Options: key encoding | CockroachDB: `00` escaped `00 FF`, terminated `00 01`; NULL marker byte; descending by inverting bytes | confirmed | `escape=0x00`, `escapedTerm=0x01`, `escaped00=0xff`; NULL asc `0x00`, desc `0xFF`; descending terminator `FF FE` (matches the doc's `9E FF FE` example) | https://github.com/cockroachdb/cockroach/blob/master/pkg/util/encoding/encoding.go |
| 3.3 | Options: key encoding | CockroachDB integers "varint with type markers", floats "sign-flip float64" | not verifiable | Beyond the part of the file read; plausible | same |
| 3.4 | Options: key encoding | MyRocks: fixed-width numbers with sign bit flipped, float bit tricks, nullable marker byte, strings in padded chunks, reverse-order column families | confirmed | Wiki: NULL flag byte `0` = NULL; binary strings in 8-byte groups plus a count byte (1..9), space-padded collations in chunks with a flag byte; column families starting with `rev:` use a reverse comparator. The wiki says indexed columns were limited to binary collations, so "collation sort key" applies only to its restore-data path | https://github.com/facebook/mysql-5.6/wiki/MyRocks-record-format |
| 3.5 | Options: key encoding | A bare `00` terminator breaks order when the next column can start with `FF` ("a" + `FF..` collides with escaped "a\0") | confirmed | The reasoning holds for untyped concatenation. FoundationDB avoids it because no element begins with `FF`; CockroachDB's `00 01` avoids it | tuple.md, encoding.go |
| 3.6 | Statistics | SQLite `sqlite_stat1` = row count then average rows per key for the first k columns; `sqlite_stat4` = samples | confirmed | Quote: "The N-th integer (for N>1) is the estimated average number of rows ... same value for the first N-1 columns" | https://www.sqlite.org/fileformat2.html |
| 3.7 | Statistics | MySQL index dives and `eq_range_index_dive_limit` | confirmed | "the optimizer makes a dive at each end of a range"; the default value (200) is not on the page | https://dev.mysql.com/doc/refman/8.0/en/range-optimization.html |
| 3.8 | Statistics | PostgreSQL `pg_statistic`: MCV, histograms, `n_distinct`, null fraction, correlation | partly right | MCV, histogram bounds and `n_distinct` confirmed on the planner-stats page; null fraction and correlation not on that page (known fields, knowledge) | https://www.postgresql.org/docs/current/planner-stats.html |
| 3.9 | Statistics | CockroachDB keeps histograms; order-statistic B+ trees are used by "few engines" | not verifiable | Not fetched | none |
| 3.10 | Recommended: floats | PostgreSQL treats NaN as equal to itself and greater than all other values | not verifiable | Not fetched (knowledge; documented in the numeric types page) | https://www.postgresql.org/docs/current/datatype-numeric.html |
| 3.11 | Recommended: strings | Collation changes silently corrupted indexes in PostgreSQL with glibc | not verifiable | Not fetched (well-known, documented in the PostgreSQL wiki) | none |
| 3.12 | Recommended: UUID | "RFC 4122 order" and "UUID v7: RFC 9562" | partly right | RFC 9562 obsoletes RFC 4122 and defines v7; byte order is the same. Cosmetic | https://www.rfc-editor.org/rfc/rfc9562 |
| 3.13 | Questions | NULLs distinct in unique indexes in SQLite and PostgreSQL, one NULL in SQL Server | not verifiable | Not fetched (knowledge) | none |
| 3.14 | Costs: version | Format version "`9` today" | wrong | `FORMAT_VERSION: u32 = 10` | `crates/krummi-pager/src/format.rs:41` |

### Doc 4: query tree and planner

| # | Section | Claim | Verdict | Correct fact or note | Source |
|---|---|---|---|---|---|
| 4.1 | Planner models: SQLite | NGQP keeps the N best partial paths; logarithmic cost scale | confirmed | N=1 simple, 5 for two-way joins, 10 for three or more, later 12 or 18 (star schema); "the cost here is logarithmic" | https://www.sqlite.org/queryplanner-ng.html |
| 4.2 | Planner models: SQLite | Without statistics, equality on an index prefix is assumed to return about 10 rows | confirmed | "the default guess is that there are an average of 10 duplicates for every value in the left-most column" | https://www.sqlite.org/optoverview.html |
| 4.3 | Planner models: SQLite | Covering indexes, multi-index `OR`, skip-scan, sort avoidance, automatic transient index | confirmed | Skip-scan is never used without `ANALYZE`; automatic index guesses N = 1,000,000 | https://www.sqlite.org/optoverview.html |
| 4.4 | Planner models: SQLite | Costs "combine rows estimated and rows scanned" | not verifiable | Not on the pages read | none |
| 4.5 | Planner models: PostgreSQL | Defaults: `seq_page_cost` 1.0, `random_page_cost` 4.0, `cpu_tuple_cost` 0.01, `cpu_index_tuple_cost` 0.005, `cpu_operator_cost` 0.0025 | confirmed | Exact | https://www.postgresql.org/docs/current/runtime-config-query.html |
| 4.6 | Planner models: PostgreSQL | Custom plans for the first five executions, then generic if its estimated cost "is not worse" than the average custom cost (`plan_cache_mode` auto) | confirmed | Rule is "not so much higher than the average custom-plan cost as to make repeated replanning seem preferable"; "not worse" is a slight simplification | https://www.postgresql.org/docs/current/sql-prepare.html |
| 4.7 | Planner models: PostgreSQL | `BitmapAnd` / `BitmapOr`, independence assumption for `AND` | not verifiable | Not fetched (knowledge) | none |
| 4.8 | Planner models: DuckDB | Standard vector size 2048 | confirmed | "The default STANDARD_VECTOR_SIZE is 2048 tuples" | https://duckdb.org/docs/2.0/internals/vector.html |
| 4.9 | Planner models: DuckDB | "push-based pipeline model since 0.3" | partly right | DuckDB switched from pull to push-based execution in a 2021 pre-release (PR #2393; talk of 2021-11-26). The version number was not confirmed | https://duckdb.org/library/push-based-execution-in-duckdb/ |
| 4.10 | Planner models: DuckDB | Cost-based join ordering with min/max zone maps and HyperLogLog distinct counts | not verifiable | Not fetched | none |
| 4.11 | Planner models: Corax | `Count` and `QueryCountConfidence` (`Low`, `Normal`, `High`); `AND`, `OR`, `AND NOT` binary matches | confirmed | Enum in `Corax/Querying/Matches/Meta/IQueryMatch.cs:11-16`; `BinaryMatch*.cs`, `AndNotMatch.cs` exist | local Corax |
| 4.12 | Serialization | Protobuf has no canonical serialization; FlatBuffers Rust crate uses `unsafe`; Cap'n Proto C# support weak | not verifiable | Not fetched (knowledge) | none |
| 4.13 | Cost model | "exact range counts from the B+ tree indexes (doc 3)" and count(*) "answered from the count alone" | wrong (against Krummi) | The tree keeps only the entry count of the whole tree (A5) and, for multi-value trees, the value count per key. A range count is a walk of the leaves (or doc 3's approximate `estimate_range`, which does not exist yet). See Krummi code claims | `crates/krummi-tree/src/catalog.rs`, `multi.rs` |

### Doc 5: vectors

| # | Section | Claim | Verdict | Correct fact or note | Source |
|---|---|---|---|---|---|
| 5.1 | Options: representation | pgvector: `vector`, `halfvec`, `bit`, `sparsevec` | confirmed | Also a typed dimension is optional | https://github.com/pgvector/pgvector |
| 5.2 | Options: representation | pgvector maximum 16,000 dimensions for `vector`, "4,000 for indexing" | partly right | `vector` up to 16,000 is right. Indexing limits: `vector` 2,000, `halfvec` 4,000, `bit` 64,000, `sparsevec` 1,000 non-zero elements. `halfvec` also stores up to 16,000 | https://github.com/pgvector/pgvector (README) |
| 5.3 | Options: representation | "a row over a page uses TOAST", values over about 2 KiB go out of line | not verifiable | The README does not discuss TOAST; PostgreSQL's TOAST threshold is a general fact (knowledge) | https://www.postgresql.org/docs/current/storage-toast.html |
| 5.4 | Distance functions | pgvector operators `<->`, `<#>` (negative inner product), `<=>`, `<+>` (L1), `<~>` (Hamming), `<%>` (Jaccard) | confirmed | Exact | pgvector README |
| 5.5 | Distance functions | pgvector rejects NaN and infinity on insert ("NaN not allowed in vector") | confirmed | `vector.c:117` "NaN not allowed in vector", `:122` "infinite value not allowed in vector"; `VECTOR_MAX_DIM 16000` | https://github.com/pgvector/pgvector/blob/master/src/vector.c |
| 5.6 | Query semantics | pgvector exact search by default, perfect recall; HNSW and IVFFlat optional approximate indexes | confirmed | "By default, pgvector performs exact nearest neighbor search, which provides perfect recall" | pgvector README |
| 5.7 | Options: representation | sqlite-vec: `float[N]`, `int8[N]`, `bit[N]` in `vec0`; maximum 8,192 dimensions; rows live in chunk blobs | confirmed | `#define SQLITE_VEC_VEC0_MAX_DIMENSIONS 8192`; shadow `_chunks` table | https://github.com/asg017/sqlite-vec (sqlite-vec.c) |
| 5.8 | What others do | sqlite-vec: KNN `WHERE embedding MATCH ? AND k = 10`; brute force, "no ANN index in the released versions" | partly right | True for the latest stable v0.1.9 (no DiskANN code). The v0.1.10-alpha.* tags and main contain DiskANN, a quantized-rescore variant and an experimental IVF | sqlite-vec.c at v0.1.9 and v0.1.10-alpha.4 |
| 5.9 | What others do | sqlite-vec `vec_distance_l2`, `vec_distance_cosine`, `vec_distance_hamming` on plain blobs | confirmed | Also `vec_distance_l1`; hamming only for bit vectors | sqlite-vec.c |
| 5.10 | Options: representation | DuckDB: fixed-size `FLOAT[N]` array, `array_distance`, `array_cosine_distance`, `array_inner_product` | confirmed | Also `array_negative_inner_product` and `array_cosine_similarity`; both arguments must have the same size | https://duckdb.org/docs/2.0/sql/functions/array.html |
| 5.11 | What others do | DuckDB `vss` extension adds an HNSW index | confirmed | Experimental; only 32-bit `FLOAT` arrays; metrics `l2sq`, `cosine`, `ip` | https://duckdb.org/docs/2.0/core_extensions/vss.html |
| 5.12 | What others do | LanceDB: exact "flat" search by default, metrics L2, cosine, dot, hamming, pre-filter is the default | confirmed | `l2` is squared Euclidean; `hamming` listed for binary vectors and `IVF_FLAT`; types float16/32/64 are named only for multivector columns | https://docs.lancedb.com/search/vector-search |
| 5.13 | Fast distance: `half` | "hardware conversion only with a nightly feature" | wrong | half 2.6.0 and 2.7.1: F16C on x86-64 is used on stable, with runtime detection when the default `std` feature is on; `use-intrinsics` is deprecated; `nightly` only adds loongarch64. Its public API is safe, but the intrinsics make its internals `unsafe` | https://github.com/VoidStarKat/half-rs (README, Cargo.toml) |
| 5.14 | Fast distance: `wide` | "public API safe; internally uses `safe_arch`"; no runtime dispatch | partly right | Build-time detection only (README says runtime detection does not work with `wide`). wide 1.x depends only on `bytemuck`, not on `safe_arch`; it calls `core::arch` intrinsics | https://github.com/Lokathor/wide |
| 5.15 | Fast distance: `pulp` | Safe abstraction with runtime dispatch | confirmed | README: "safe abstraction over SIMD ... dispatch ... based on the features detected at runtime". Its internal `unsafe` was not inspected | https://github.com/sarah-quinones/pulp |
| 5.16 | Fast distance: `simsimd` | C 99 library with Rust bindings, CPU-dependent kernels | confirmed | The project is now named NumKong (C 99 and C++ 23 core, 30+ SIMD backends); crate `simsimd` last at 6.5.16 (2026-03). "callers cast `f16` slices with `from_raw_parts`" not checked | https://github.com/ashvardanian/SimSIMD |
| 5.17 | Fast distance | `std::simd` is nightly-only | confirmed | Not re-fetched; consistent with f16 status. `slice::as_chunks` is stable (in Rust release notes; the workspace uses rust 1.89 and the engine already uses it in `byte_sum`) | https://github.com/rust-lang/rust/blob/master/RELEASES.md |
| 5.18 | Determinism | Rust never fuses multiply-add and LLVM does not reorder float additions without fast-math | not verifiable | Language/compiler semantics, not fetched; consistent with documented behavior (knowledge) | none |
| 5.19 | Throughput | 3 M vectors/s/core at 768-dim f32, etc. | not verifiable | The document marks them as estimates | none |
| 5.20 | Options: alignment | `bytemuck::cast_slice::<u8,f32>` "fails or needs unsafe" on an unaligned slice | confirmed | `cast_slice` panics; `try_cast_slice` returns an error | bytemuck src/lib.rs |
| 5.21 | Options: size against the page | "A 1,536-dim f32 vector (6,144 bytes) fits a leaf only when the leaf holds one row" | wrong | A leaf cell is at most 4,078 bytes (`MAX_LEAF_CELL`), so a 6,144-byte value always lives off its leaf (run of contiguous pages). A 768-dim row (3,134 bytes) fits inline, two per leaf | `crates/krummi-tree/src/node.rs:41` |
| 5.22 | Distance functions | i8 accumulation fits `i32` at 4,096 dimensions ("127^2 x 4,096 = 66 M") | confirmed | Worst case with -128 is 16,384 x 4,096 = 67.1 M, still far below 2^31 | arithmetic |

### Doc 6: C interface and C# binding

| # | Section | Claim | Verdict | Correct fact or note | Source |
|---|---|---|---|---|---|
| 6.1 | Boundary style | LMDB `MDB_val` valid until the transaction ends or the next write (update) | confirmed | Header: "valid only until a subsequent update operation, or the end of the transaction" | https://github.com/LMDB/lmdb (lmdb.h) |
| 6.2 | Boundary style | LMDB: `mdb_strerror(code)` is a pure function; transactions belong to their thread unless `MDB_NOTLS` | confirmed | `char *mdb_strerror(int err)`; "Each transaction belongs to one thread. The MDB_NOTLS flag changes this for read-only transactions" | lmdb.h |
| 6.3 | Boundary style | RocksDB C API: `char** errptr` out-parameter, malloc'd message, caller frees | confirmed | Header comment: `*errptr points to a malloc()ed null-terminated error message` | https://github.com/facebook/rocksdb/blob/main/include/rocksdb/c.h |
| 6.4 | Boundary style | RocksDB `rocksdb_get` returns a malloc'd copy ("a copy per read") | partly right | `rocksdb_get` returns a copy; a zero-copy `rocksdb_get_pinned` exists in the same header. Iterator key/value pointers: not read in detail | c.h |
| 6.5 | Boundary style | SQLite: `sqlite3_errmsg` per connection, column pointers valid until `step`/`reset`, `sqlite3_free` | not verifiable | Not fetched (knowledge) | https://www.sqlite.org/c3ref/intro.html |
| 6.6 | Rust to C# bridges | csbindgen generates `LibraryImport` "in recent versions" | wrong | README (v1.9.x): "Automatically generates C# `DllImport` code"; all examples are `[DllImport(..., CallingConvention.Cdecl)]`; function pointers for callbacks; no `LibraryImport` generation in the 1.9.8 sources | https://github.com/Cysharp/csbindgen |
| 6.7 | Rust to C# bridges | csbindgen is C# only; NuGet `runtimes/<rid>/native/` layout and `NativeLibrary.SetDllImportResolver` | confirmed | README shows both | csbindgen README |
| 6.8 | Rust to C# bridges | interoptopus generates C, C# and Python from one description | partly right | README lists C# (stable) and Python (experimental) backends; the C backend was not listed in the part read | https://github.com/ralfbiedert/interoptopus |
| 6.9 | Rust to C# bridges | uniffi C# support is third party (`uniffi-bindgen-cs`); `RustBuffer` per call contradicts zero-copy | partly right | `uniffi-bindgen-cs` is a separate NordSecurity project (v0.11.1 for uniffi 0.31, needs .NET 8+). The `RustBuffer` claim was not checked | https://github.com/NordSecurity/uniffi-bindgen-cs |
| 6.10 | C# binding | `LibraryImport` exists from .NET 7 and is source generated | confirmed | The source generator ships with the .NET 7 SDK | https://learn.microsoft.com/dotnet/standard/native-interop/pinvoke-source-generation |
| 6.11 | C# binding | `SafeHandle` + `DangerousAddRef`, `ref struct` scoping with C# 11 `scoped` rules, `Int128` | not verifiable | Not fetched (knowledge); the document itself marks the `scoped` point "to verify" | https://learn.microsoft.com/dotnet/api/system.runtime.interopservices.safehandle |
| 6.12 | ABI | `uint32_t size` as first field of an options struct "(the Win32 / SQLite pattern)" | not verifiable | The size-prefix pattern is Win32; SQLite's extensible structs use `iVersion` fields. Not checked at source | none |
| 6.13 | Packaging | SQLitePCLRaw bundles = "one package per RID plus a meta package" | not verifiable | Not checked | none |
| 6.14 | Handles | "the per-transaction tree cache the engine already has (B9)" | wrong | B9 of the trees PRD is "several trees open at once". `open_tree(&str)` in `crates/krummi/src/lib.rs:439,680` opens by name each call; no per-transaction name cache was found. The C API would pay a catalog lookup per call or must keep its own cache | `docs/prd/krummi/trees.prd.md` B9 |
| 6.15 | Handles | `krummi_store_close` "forwards the engine's rule ... (T8, to be decided in the transactions PRD)"; "T8 is still open" (question 8) | wrong | T8 is decided: closing the store with open transactions does not compile. `Store::close(self)` consumes the store, transactions borrow it (`ReadTransaction<'s>`). No runtime rule exists to forward | `docs/prd/krummi/transactions.prd.md` T8; `crates/krummi/src/lib.rs:163,179,187` |
| 6.16 | Errors | `ErrorKind` list (16 kinds) matches the engine | confirmed | Access ... TreeAlreadyExists, all 16 present and in the same order | `crates/krummi-pager/src/error.rs:14-50` |
| 6.17 | Threads | Read transactions Send + Sync and shareable, write transaction stays on its thread (T6); second write on the same thread is Misuse (T5); abandoned write aborts (T10) | confirmed | | `transactions.prd.md` T5, T6, T10 |

### Doc 7: dictionary compression

| # | Section | Claim | Verdict | Correct fact or note | Source |
|---|---|---|---|---|---|
| 7.1 | Summary / options | `ruzstd` decodes; dictionary encoding is roadmap; `dict_builder` makes only raw-content dictionaries | confirmed | README: encoder "usable" at `Fastest` only, "[ ] Dictionaries" unchecked under compression, "only provides support for creating raw content dictionaries". The summary omits that ruzstd has a (Fastest-level) encoder | https://github.com/KillingSpark/zstd-rs (ruzstd/Readme.md) |
| 7.2 | Summary | The `zstd` crate binds C | not verifiable | Not fetched (well known: `zstd-sys`) | https://crates.io/crates/zstd |
| 7.3 | Options | `lz4_flex` block API has `compress_with_dict`, `decompress_with_dict` and `_into` variants | confirmed | Present in 0.12.0 (the version Krummi pins) and in 0.14.0; `decompress_with_dict` needs the uncompressed size from the caller | https://docs.rs/lz4_flex/latest/lz4_flex/block/index.html |
| 7.4 | Options | LZ4 dictionary = byte string of samples, only the last 64 KiB is referenced | confirmed | `assert!(ext_dict.len() <= super::WINDOW_SIZE)` | lz4_flex `src/block/compress.rs` |
| 7.5 | Options / Design | lz4_flex is "pure Rust, safe, already audited in this repository" for the table layer | partly right | The pager enables only `safe-decode` and `checked-decode` with `default-features = false`; its encoder runs in the non-`safe-encode` implementation (D12 annotation of 2026-11-06). A table-layer encoder outside the pager would need `safe-encode`; Cargo unifies features per build, so enabling it anywhere would also switch the pager's journal encoder to the safe one (30-50 % slower per the D12 annotation). Check with `cargo tree -e features` before deciding (not run here) | `crates/krummi-pager/Cargo.toml:17`; `krummi.prd.md` D12 annotation |
| 7.6 | Options | `fsst-rs`: pure Rust, 1-3 GB/s, "not production ready", little-endian only, at most 255 symbols | confirmed | README wording matches; 255 symbols is from the FSST paper, not the README. Version 0.7.0 | https://github.com/spiraldb/fsst |
| 7.7 | Options | `fsst-rs` "`unsafe` use unverified" | confirmed | The 0.7.0 crate source has 88 occurrences of `unsafe` and no `forbid(unsafe_code)` | crates.io fsst-rs 0.7.0 |
| 7.8 | Options | No Rust zstd trainer except ruzstd's raw-content builder | not verifiable | Plausible; the C `zstd` crate also exposes the trainer | none |
| 7.9 | Sources | `lz4_flex` is at "Cargo.toml line 13" (root) | wrong | Root `Cargo.toml` has no `lz4_flex`; it is at `crates/krummi-pager/Cargo.toml:17` | local |

## Krummi code claims

| # | Document | Claim | Verdict | Evidence |
|---|---|---|---|---|
| K1 | 2, 3 | Maximum key 2,048 bytes (B5), minimum 1 | confirmed | `MAX_KEY = 2048` (`node.rs:35`); B5 in `trees.prd.md:45` ("de 1 a 2.048 bytes; la clave vacía no es válida") |
| K2 | 1, 2, 3 | Maximum value 256 MiB (268,435,456 bytes) | confirmed | `MAX_VALUE = 268_435_456`; B6 and V1. The provisional 1,024-byte limit of A11 is superseded (annotation `trees-keys.prd.md:66`); doc 3 cites line 66 as "A11: value maximum 268,435,456", which is the annotation, not the A11 row |
| K3 | 2 | A value over "4,078 bytes minus its key" lives off its leaf | partly right | `MAX_LEAF_CELL = (PAYLOAD - 16)/2 - 2 = 4,078`, and the cell includes a 4-byte header, so the value limit is 4,074 minus the key length (`fits_inline`: `4 + key + value <= 4078`) |
| K4 | 3 | Branch cells store full keys; no suffix truncation | confirmed | Branch cell = 2-byte key length, key, 8-byte child (`node.rs:19-21`). At a leaf split the separator is the full first key of the right node: `node::cell_key(cells[at], true)` copied whole (`lib.rs:~890-897`). A branch split moves a whole existing key up. No truncation code anywhere in `krummi-tree/src` |
| K5 | 2 | "Row overhead 14 bytes in the leaf (4-byte header, 8-byte key, 2-byte offset)" against "6 in clustered" | partly right | 4 + 8 + 2 = 14 is right for an 8-byte key. The "6" for a clustered table excludes the primary key while the 14 includes the row id key, so the comparison mixes two accountings |
| K6 | 2, 3 | A tree keeps an exact entry count in O(1) (A5) | confirmed | `count` field in the catalog entry (`catalog.rs:41,86`); A5 "sin recorrer el árbol". It is a count of the whole tree only: there is no range count and no rank structure |
| K7 | 3 | Order-statistic counts would need +8 bytes per branch cell; fan-out about 290 to 230 for 16-byte keys | confirmed | Branch cell = 2 + 16 + 8 = 26 (+2 offset) gives 8,160/28 = 291; +8 gives 226. 8 / 26 = 31 %, 8 / 50 = 16 % |
| K8 | 3 | Multi-value trees: values 0-2,048 bytes, the value set is inline or a subtree (M2), "number of values of one key" exists, "no total of values", tree count = number of keys | confirmed | M1, M2, M6; `multi.rs:1-30` (count stored in the blob, read without the subtree). Also: values are never off-leaf (M1), ordered byte-wise |
| K9 | 2, 3 | Multi-value tree can hold the 8-byte row ids of a non-unique index | confirmed | Capability only. A key with a huge set becomes a subtree (M2) |
| K10 | 2, 3 | Fixed-size trees: u64 keys, values 0-2,048 bytes | confirmed | X1, X5. Consequence for doc 3 ("natural place for a heap's row-id to location map") and doc 2: a fixed-size tree cannot store rows (up to 256 MiB); doc 2 correctly uses a normal tree |
| K11 | 3 | The tree already has prefix and bounded scans, ascending and descending | confirmed | `range.rs` (`Interval` with bounds and prefix); double-ended scans (`front`/`back`) in `range.rs` and `fixed.rs` |
| K12 | 3 | Two entries per leaf and three keys per branch always fit, even at the longest key | confirmed | Compile-time asserts in `node.rs` (`HEADER + 2*(MAX_LEAF_CELL+2) <= PAYLOAD`, `HEADER + 3*(MAX_BRANCH_CELL+2) <= PAYLOAD`) |
| K13 | 2 | Appends fill the rightmost leaf completely; merge at a quarter; a changed page keeps its number (no path copy) | confirmed | `lib.rs:11-18`; `UNDERFULL = PAYLOAD_SIZE / 4` |
| K14 | 3 | Page size 8 KiB | confirmed | `PAGE_SIZE = 8192`, `format.rs:33` |
| K15 | 3 | Format version `9` | wrong | `FORMAT_VERSION = 10` (`format.rs:41`) |
| K16 | 6 | T8 (closing a store with open transactions) is "still open" in the transactions PRD | wrong | T8 is a compile-time rule, see 6.15. T5, T6 and T10 are as described |
| K17 | 6 | Rust crates in `krummi-capi` would need `panic = "unwind"` and `unsafe` allow-list of two paths | not verifiable | Proposal. D12 today: "el Pager ... es la única parte del motor que puede usar código no seguro" (`krummi.prd.md:52`); workspace lint `unsafe_code = "forbid"` with only the pager opting out (`Cargo.toml:11-13`) |
| K18 | 7 | Journal compresses with LZ4 and falls back to raw when it does not shrink | confirmed | `journal.rs:347-356`: `Ok(n) if n < raw_len => Some(n)`, else raw |
| K19 | 7 | D11/D12 at `krummi.prd.md:51-52`; module 7 text with "compresión de valores por diccionario" at line 216 | confirmed | Lines 51, 52 and 216 match |
| K20 | 5 | `byte_sum` uses 16 lanes with `as_chunks` and a test at offsets and lengths | confirmed | `krummi-bench/src/data.rs:91-106` and test at `:197-212` (lengths 0..65,537; offsets 0, 1, 7; doc 5 says offsets `0..7`, the test uses 0, 1, 7) |
| K21 | 2 | Commit is more than half of write time (plan) | confirmed | `docs/performance/plan.md:109-128`: 55.5 % (W2) and 59.4 % (W21) |
| K22 | 4 | Planner needs "exact counts ... with a cap"; "table row count is kept per transaction" | partly right | Row count: yes, A5 (entry count of the table's tree, O(1)). Range counts: no, see 4.13 |

## Voron claims

All checked in `C:\projects\ravendb\src\Voron` (and `Corax` for doc 4).

| # | Document | Claim | Verdict | Evidence |
|---|---|---|---|---|
| V1 | 1 | `TableValueBuilder.CopyTo` writes a varint count, then one start offset per column of width 1, 2 or 4, then the columns | confirmed | `TableValueBuilder.cs:133-202` |
| V2 | 1 | "Offsets are relative to the end of the offset table (`pos = count * elementSize`, line 145)" | wrong | The first offset equals `count * elementSize`, so offsets are relative to the start of the offset table (just after the count varint). The reader returns `dataPtr + position` where `dataPtr = Pointer + _offset` (`TableValueReader.cs:67-99`). The offset table is included in the offsets |
| V3 | 1 | Width from total size: above 255 bytes 2, above 65,535 bytes 4 | confirmed | Reader `TableValueReader.cs:36-41`; builder `ElementSize` (`TableValueBuilder.cs:28-48`) computes with the offsets included (`size + count*2 + 1 > ushort.MaxValue`), a slightly different threshold than the reader |
| V4 | 1 | `Read(index, out size)` reads `offset[i]` and `offset[i+1]` or the row end: O(1) | confirmed | `TableValueReader.cs:67-99` |
| V5 | 1 | Fixed size asserted only in debug (line 51) | confirmed | `Debug.Assert(sizeof(long) == size)` in `ReadLong` |
| V6 | 2 | Section: header page + N data pages; N = 32 under "16 MB", growing up to about 4,000; each new section doubles, capped at 32 MB | partly right | `GetNumberOfPagesInSmallSection` returns 32 while `NumberOfAllocatedPages <= 1024*4` (32 MiB at 8 KiB pages; the code comment says 16 MB), then 128, 512, 1024 and `(PageSize - 96)/2 = 4,048` above 32,768 pages. Doubling capped at 32 MB (1 MB on 32-bit): `Table.cs:1186-1190` |
| V7 | 2 | Header: `ReservedHeaderSpace` 96, one `ushort` of available space per page | confirmed | `RawDataSection.cs:16,83-90` |
| V8 | 2 | Entry = 4 bytes of sizes (`AllocatedSize`, `UsedSize` with bit 15 compressed, `-1` freed) | confirmed | `RawDataSection.cs:25-62` |
| V9 | 2 | Row of `(pageSize - header)/2` or more goes to its own overflow pages, id = `firstPage * pageSize` | confirmed | `MaxItemSize = (PageSize - header)/2`; `Table.cs:725-748` (`builder.Size + sizeof(entry) < MaxItemSize`, else `AllocatePageForLargeValue`, `id = page.PageNumber * PageSize`) |
| V10 | 2 | Every insert rewrites the stats entry (`NumberOfEntries`, `OverflowPageCount`) | confirmed | `Table.cs` insert ends with `_tableTree.DirectAdd(StatsSlice, ...)`; `TableSchemaStats.cs:5-12` |
| V11 | 2 | `OnDataMoved` deletes the old id from every index and inserts the new one | confirmed | `Table.cs:110-125` (`DeleteValueFromIndex`, `InsertIndexValuesFor`) |
| V12 | 2 | Delete-then-insert on update when the row does not fit in place | confirmed | `Table.cs:436-444` ("can't fit in place, will just delete & insert instead") |
| V13 | 2 | Density: above 0.5 stays; 0.15-0.5 candidate; below 0.15 `ReleaseNearlyEmptySection` | confirmed | `Table.cs:579-593` |
| V14 | 3 | Primary key tree maps key to a `long` id; secondary index maps each key to a fixed-size tree of ids; compare old and new key slices before touching an index | confirmed | `Table.cs:927-964` (`UpdateValuesFromIndex`, `SliceComparer.AreEqual`, `GetFixedSizeTree(...).Delete/Add(id)`); also `DynamicKeyIndexes` and `FixedSizeIndexes` (key `long` to id) |
| V15 | 3 | `IndexEntryKeyGenerator.cs:1-9` is only a marker attribute; Voron has no typed encoding | confirmed | File defines `StorageIndexEntryKeyGeneratorAttribute` (empty attribute). The file name in the doc is slightly different from the class |
| V16 | 3 | `TableSchema.IndexDefinitions.cs`: `DynamicKeyIndexDef`, `SupportDuplicateKeys`, `FixedSizeKeyIndexDef` | confirmed | Lines 200-212 and following |
| V17 | 4 | Corax `QueryCountConfidence` Low/Normal/High, binary matches | confirmed | see 4.11 |
| V18 | 5 | `VectorEmbeddingType`: `Single`, `Int8`, `Binary`, `Text` | confirmed | `Voron/Data/Graphs/VectorEmbeddingType.cs` |
| V19 | 5 | I8 vectors carry a trailing 4-byte magnitude; cosine kernel casts bytes to float with `MemoryMarshal.Cast` | confirmed | `Hnsw.SimilarityMethods.cs:24-36` (`a.Length - sizeof(float)`) |
| V20 | 5 | `Corax/Mappings/VectorOptions.cs`: `NumberOfCandidates`, `NumberOfEdges`; `VectorSearch(..., isExact, ...)` | confirmed | `VectorOptions.cs:7-8`; `IndexSearcher.VectorSearch.cs:19` |
| V21 | 6 | `env.WriteTransaction()`, `tx.OpenTable(schema, "name")`, `table.Set(...)`, `ReadByKey(..., out TableValueReader)` | confirmed | `StorageEnvironment.cs:649,655`; `Transaction.cs:233,241`; `Table.cs:174,2098` |
| V22 | 7 | Zstd level 3 with a trained dictionary; `ZstdLib.Compress` at `TableValueCompressor.cs:80`; dictionary 1 page; retrain if ratio worse by more than 10 points and etag multiple of 1024; training 256 rows or 1 MiB, skip rows over 32 KiB, abort after 16 Ki skipped | confirmed | `:80,84,124,159-190,209,221`; `CompressionDictionaryInfo` is one byte (`ExpectedCompressionRatio`) |
| V23 | 7 | Dictionary replaces the current one only if it compresses at least 10 % smaller; stored in tree `CompressionDictionaries`; rollback removes the cached dictionary (RavenDB-17758) | confirmed | `TableValueCompressor.cs:242-261,421-455`; `TableSchema.cs:21-22,50-51` |

## Contradictions between the documents

Positions are listed without resolving them.

1. **Index structure for non-unique indexes.** Doc 2 (Recommended design table): normal tree `ix:<name>` for unique
   and a **multi-value tree** for non-unique, key = encoded index key, value = 8-byte row id. Doc 3 (Index structure,
   option B): a **normal tree** with key = `encoded columns || locator` and value = covering columns; it rejects the
   multi-value tree because it loses exact range counts and adds two representations. Doc 3 also says covering
   ("include") columns live in the index value; doc 2's index values are only the row id.
2. **Table kind and locator.** Doc 2: heap by 8-byte row id, `pk` tree maps key to row id, primary key lookup = two
   descents, secondary entries hold only the row id. Doc 3: assumes the locator is the primary key bytes (clustered
   table) and "works unchanged with an 8-byte row id"; it says doc 2 "was not read". Doc 4: "the table is clustered by
   primary key (doc 2), so key ranges are table ranges", covering "or the primary key is, since secondary index entries
   carry it". Doc 3 also says a fixed-size tree is the natural place for a heap's row-id to location map "(document 2)";
   doc 2 stores rows in a normal tree keyed by the row id.
3. **Exact counts (doc 4) against estimates (doc 3).** Doc 4: the planner is "fed by exact range counts from the B+
   tree indexes (doc 3)", `count(*)` over an index range is "answered from the count alone", counts have confidence
   `Exact`/`Capped`/`Estimated`, and no persisted statistics are needed in v1 (Q10). Doc 3: only the whole-tree entry
   count is exact and O(1); range counts are an approximate `estimate_range` (not implemented) and exact range counts
   would need order-statistic counts in branch cells, which it defers (question 6); it also recommends an explicit
   `analyze` that stores distinct-prefix counts and a most-common-values list in a stats tree in the first release.
4. **Maximum vector dimension.** Doc 1: `dim` 1..=16,384 proposed (question 5 repeats it). Doc 5: default maximum 4,096
   for float and 32,768 bits for `bit` (question 2 offers 1,536, 4,096 or 16,000).
5. **Vector alignment.** Doc 1: vector payloads are padded to a 4-byte (f32) or 2-byte (f16) boundary relative to the
   row start, vector columns are ordered first, "for the fast aligned path". Doc 5: padding is "unneeded"; kernels read
   `&[u8]` with `from_le_bytes` and the stored format needs no alignment. Doc 1 itself says unaligned loads cost the
   same and that the row start in the page is not aligned (cell offsets are arbitrary), so the padding only helps if
   doc 2 aligns values.
6. **Vector storage format.** Doc 1: `dim * width(elem)` bytes with no per-value header; `bit` requires `dim` a multiple
   of 8; no scale for `i8`. Doc 5: `i8` followed by a 4-byte f32 scale, an optional 4-byte norm for cosine columns
   (`store_norm`), `bit` stored as `ceil(dim/8)` bytes, optional `bf16`. The schema-fixed length of doc 1 changes if doc
   5's scale and norm are adopted.
7. **Cursor and result API.** Doc 4: `Prepare(bytes)` returns a prepared handle; `NextBatch(out span of row refs)` fills
   a caller buffer of `{ptr,len}` pairs (batches of 256), valid until the next call. Doc 6: no prepare function in the C
   sketch; `krummi_cursor_next_row(cursor, row, has)` returns one row; batches copied into a caller buffer are the
   rejected option (question 7b).
8. **Wire format description.** Doc 4: `magic(2) version(1) flags(1) table, node_count, nodes (tag, len, payload),
   param_block`, a chain Scan -> Filter -> Aggregate -> Sort -> Limit -> Project. Doc 6 describes it as "magic, format
   version, node count, flat pre-order nodes with ids, parameters in a separate section".
9. **Where the compression flag and dictionary id live.** Doc 1: `flags` bit 2 says the bytes after the 3-byte header
   (null bitmap, fixed section, offsets, variable data) are one compressed body; per-column compression is possible
   without changing the header. Doc 7: the dictionary id is a varint "after the offset table", read from the end of the
   row, and the offset table "stays readable only after decompression"; whole-row only. The two place the id and the
   offset table on different sides of the compressed boundary.
10. **Policy on dependencies that contain `unsafe`.** Doc 5 (R3): a dependency is acceptable only if it is safe Rust;
    `wide`, `pulp` and `simsimd` are excluded for that reason, while `half` is called "safe" (its hardware path is
    `unsafe` internally). Doc 4 (question 5) asks whether a dependency with `unsafe` inside (FlatBuffers, `memchr`,
    `zerocopy`) is acceptable and recommends yes for audited crates. Doc 7 rejects the C `zstd` crate as "Fails: C
    dependency" in its table while the same cell says "the engine itself stays safe", and rejects `fsst-rs` for
    unverified `unsafe` while accepting `lz4_flex`. Doc 6 adds an `unsafe` crate (`krummi-capi`).
11. **Page size.** Doc 1 (Costs): "with 4 KB pages that means one row per page". Docs 2, 3 and 5: 8 KiB pages.
12. **Inline limit of a row.** Doc 1 treats a row as one tree value with no limit short of 256 MiB; doc 2 gives the
    inline threshold ("over 4,078 bytes minus its key lives off its leaf"); doc 5 says a 6,144-byte vector "fits a leaf
    only when the leaf holds one row". Not the same rule (see claim 5.21 and K3).
13. **Row overhead accounting.** Doc 2 compares 14 bytes per row (heap, key included) with 6 (clustered, key
    excluded). Doc 1 counts overhead of the row value without the tree's cell header.
14. **UUID specification.** Doc 1: RFC 9562 byte order. Doc 3: "RFC 4122 order" for the key, RFC 9562 for v7. Same
    order; different citations.
15. **Doc 7 versus doc 2 on the index entry.** Doc 7 says "a secondary index entry holds key plus row address, not the
    row". Consistent with doc 2, but it contradicts doc 3's covering-column values (an index entry can hold row columns),
    which a compressed-row design would have to treat separately.
16. **Statistics storage.** Doc 4 (question 10 and cost model): no persisted statistics in v1, counts per query. Doc 3:
    an explicit `analyze` writes a stats tree "of the table" in the first release. (Related to item 3.)


## Reconciliation (2026-11-09)

The seven documents were edited to match the decisions the user took after the first drafts (README, "Decisions taken
with the user after the first drafts", including the last one, the tuple key encoding) and to correct every wrong or
partly right claim above. The comparison of options stays in each document as the record of why; the sections
"Recommended design" and "Open product questions" became "Decided design" and "Product questions", and each question
says "Decided 2026-11-09" with the README quotation or stays open with its options. This file was not edited above this
section; the tables record what the first drafts said. No cargo command, build, test or benchmark was run, and no
source was fetched again: the corrected facts are the ones already given in the tables above.

### Per document

**01 Row format and types.**
- Type set fixed to the decided one; nullability default flipped to nullable (`NOT NULL` declared), primary key
  columns never null and not repeated in the row (they live in the key; an internal choice, still open to measurement).
- One vector payload: no padding, `&[u8]` read with `from_le_bytes` (padding dropped; no reason survived in
  verification: the row start is not aligned in the page), `i8` with a 4-byte `f32` scale, cosine column with a
  4-byte `f32` norm, `bit` as `ceil(dim / 8)` bytes, maximum 4,096 dimensions (was 16,384).
- Compression hook placed in one place with doc 7: `flags` bit 2, then `dictionary_id` and `raw_len` varints and an LZ4
  block holding the body.
- Fixed: Voron offsets are relative to the start of the offset table (V2), SQLite short record takes the schema
  default (1.2), Arrow row format claim replaced by Spark `UnsafeRow` with the `arrow-row` correction (1.7), C#
  `decimal` conversion limited by the 96-bit mantissa (1.10), 256 MiB is V1 of the large-values PRD and V2 is the
  contiguous read (1.15), "4 KB pages" corrected to 8 KiB with the 3,134-byte and 6,144-byte cases (1.16), PostgreSQL
  null bitmap polarity noted (1.3), inline limit rule `4 + key + value <= 4,078` stated.

**02 Row storage.** Rewritten around the clustered table (row = value of the primary key tree, mandatory primary key,
secondary indexes keyed by `columns ‖ primary key`). The heap stays as rejected option C; the user's argument and the
append split (`odd/tasks/trees-append-split*.md`, no PRD exists) explain why clustering is cheap for increasing keys; the
random UUID cost is a risk to measure. Fixed: Voron section sizes (V6), inline limit 4,074 minus the key (K3), the
per-row overhead accounting (K5, contradiction 13: now 6 + key for clustered against 28 + key for the heap).
Questions: table kinds, primary key and scan order decided; row ids moot; maximum row size open.

**03 Keys and indexes.** Rewritten around the FoundationDB tuple encoding (see below), the index key
`tuple(columns) ‖ tuple(primary key)`, uuid order per column, ulid as its own type, nulls first or last per index
column, distinct nulls in unique indexes, typed error for a key over 2,048 bytes, and statistics as estimates:
per-column histograms (also without an index) created and refreshed automatically in the background plus `analyze`,
the O(1) entry count, and a boundary-descent range estimate; "explicit analyze only" and the exact-count plans are
gone. Fixed: format version `10` (3.14, K15), RFC 9562 cited (3.12), A11 annotation line (K2), the fixed-size tree
no longer called a place for a row-id map (K10), equality counts on a multi-value tree noted as exact and O(1) (summary
point 3).

**04 Query tree and planner.** Exact range counts replaced by estimates (statistics, histograms, boundary descent,
confidence `Exact`/`Histogram`/`Default`); `count(*)` with a filter is a walk, never an estimate; scope aligned with
the decided first version (`<>` as `NOT (=)`, keyset paging out of V1); cursor returns batches, no `Prepare` in the
first C surface; clustered primary key and covering through the key's primary key; DuckDB push-based note corrected
(4.9); wire format declared the one description, referenced by doc 6. New open question: null in comparisons.

**05 Vectors.** One dimension limit (4,096, `bit` included), `bf16` dropped, one payload layout with doc 1, norm
stored for cosine columns (for `i8` the scale is not the norm), `store_norm` option replaced by the cosine flag.
Fixed: pgvector limits (5.2), `half` hardware conversion on stable (5.13) with the cost/benefit re-read and the
hand-written conversion kept for dependency and simplicity reasons, `wide` (5.14), `simsimd`/NumKong (5.16),
sqlite-vec alpha tags (5.8), 6,144-byte vector always off-leaf (5.21), `i8` bound with -128 (5.22), `byte_sum` offsets
0, 1, 7 (K20), top-k heap memory counts the primary key, arm64 golden-vector run replaced by x86-64 on two operating
systems because CI has no arm64.

**06 C interface and C# binding.** Decided design recorded (platforms win-x64 and linux-x64 only, `LibraryImport`
declarations written by hand because csbindgen emits `DllImport` (6.6), batch cursor `krummi_cursor_next_batch`,
`krummi_row` as key plus value). T8 corrected (6.15, K16): it is a compile-time rule, so the store-close behavior for
C handles is stated as a new decision (question 8, still open, `MISUSE` recommended). Fixed: B9 claim about a tree
cache (6.14), RocksDB `rocksdb_get_pinned` (6.4), interoptopus backends (6.8), uniffi-bindgen-cs facts (6.9),
`iVersion` in SQLite (6.12).

**07 Dictionary compression.** Deferred with hooks (decided), LZ4 with dictionary, opt-in per table, whole row; hook
placement agreed with doc 1; `raw_len` added because `decompress_with_dict` needs it; per-table `dict` tree. The
`lz4_flex` feature-unification risk is now stated as a check to make before building (7.5) with the alternative of
exposing the dictionary function from the pager. Fixed: ruzstd has a `Fastest` encoder (7.1), `fsst-rs` unsafe count
(7.7), the Cargo.toml citation (7.9), the "pure Rust, safe, audited" claim (7.5), the C `zstd` reason (R2, not D12).

### Contradictions

| # | Resolution |
|---|---|
| 1 | Normal tree per index, key `tuple(columns) ‖ tuple(primary key)`, value = included columns (README decision). Doc 2 and doc 3 agree; no multi-value tree. |
| 2 | Clustered by primary key; locator = encoded primary key. Doc 2 rewritten, doc 3 assumption removed, doc 4 already assumed it; the fixed-size-tree row-id map sentence deleted. |
| 3 | Estimates, no counts in branch nodes (README). Doc 4 uses statistics and range estimates; doc 3 owns them. |
| 4 | 4,096 dimensions for every element type, `bit` included. Docs 1 and 5. |
| 5 | No padding; vectors read as `&[u8]` with `from_le_bytes`. Doc 1's padding and the `bytemuck` fast path dropped. |
| 6 | One layout in doc 1 and doc 5: `i8` + scale, cosine norm, `bit` = `ceil(dim / 8)`. |
| 7 | Batches: doc 6 changed to `krummi_cursor_next_batch`; doc 4 keeps its batch cursor but drops `Prepare` (plan cache is transparent). Choice implied by the decision "a cursor that returns batches of rows". |
| 8 | Doc 4 is the one description of the wire format; doc 6 refers to it. |
| 9 | One place: `flags` bit 2 plus `dictionary_id`/`raw_len` right after the 3-byte header, body compressed whole (offset table inside it). Docs 1 and 7. |
| 10 | One policy: D12 governs Krummi's own code; a dependency containing `unsafe` is judged case by case and recorded by annotation. Docs 5 and 7 no longer exclude crates "for `unsafe`" (`half` had been called safe while its hardware path is unsafe inside); they are excluded for determinism, cost or the C toolchain. Doc 4 question 5 keeps it open for the user to confirm; this resolution is the one the decisions imply (the `lz4_flex` unsafe encoder is already accepted by the D12 annotation of 2026-11-06). |
| 11 | 8 KiB pages everywhere. |
| 12 | One rule: a row is a tree value, inline while `4 + key + value <= 4,078` bytes, otherwise a contiguous run (docs 1, 2, 5). |
| 13 | Both documents count the cell header and slot: clustered 6 + key, heap 28 + key. |
| 14 | RFC 9562 cited in docs 1 and 3. |
| 15 | Only table rows are compressed; secondary entries are `tuple(columns) ‖ tuple(primary key)` with uncompressed included columns. Doc 7 states it. |
| 16 | A stats tree per table exists (doc 3); persisted statistics are decided; doc 4 question 10 is decided accordingly. |

### Tuple key encoding (last decision)

README, last bullet: keys of tables and index entries use FoundationDB's tuple encoding, with `ulid` on the UUID code,
`uuid` in SQL Server order by permuting its 16 bytes, `timestamp`/`date`/`time`/`duration` as integers, `decimal` on a
user code (`0x40`-`0x4F`) and a descending column by inverting its element's bytes. Applied in doc 3 (encoding table,
examples, descent, nulls, key-size limit); doc 1 now refers to it from the type table and keeps its own row format;
doc 4 has predicates encoded by Rust into tuple elements and the index key as two concatenated tuples; docs 6 and 7
refer to the key as `tuple(...)`. CockroachDB's rules stay in the comparison as the first draft's option. Corrections
made to doc 3 because of it:
- The claim that FoundationDB's bare `00` terminator breaks order (3.5) is withdrawn for tuples: every element starts
  with a type code and no type code is `FF`. It does hold in one place, which the document now handles: an inverted
  NULL is `FF`, so a descending nullable column with nulls last cannot follow an ascending string or bytes column; the
  index validator rejects that combination with a typed error. *Superseded on 2026-11-10:* an external review found
  that the rejection breaks N5 of the tables PRD, which admits every combination; a brute-force check found a second
  case (a descending string or bytes column followed by an ascending column with nulls first). Doc 3 now ends
  descending strings with `FF FE` and writes a descending null that sorts last as `FE FF FF`, and admits every
  combination (doc 3, Descending columns).
- "Varints need a decode to compare" does not apply to the tuple integers (the type code carries the length), only to
  LEB128-style varints.
- Nulls last uses the user code `4F`; `decimal` is proposed as `40` plus 16 bytes of the biased `i128`.
- The code table (`0x20`/`0x21` floats, `0x26`/`0x27` bool, `0x30` UUID, integer codes `0x0C`-`0x1C`) was recalled, not
  fetched; the fact-check confirmed only the string and integer rules (3.1). The PRD verifies each code against
  `tuple.md`.

### Questions still open for the user

1. SQL Server uuid order: which byte numbering do "10-15, 8-9, 6-7, 4-5, 0-3" follow (doc 3 reads them in the
   `Guid.ToByteArray()` layout, which reproduces SQL Server's order; read on the RFC bytes the key differs)?
2. Dependencies containing `unsafe` (doc 4, question 5): confirm the proposed policy.
3. C interface: closing a store with open transactions (doc 6, question 8).
4. Maximum row size (doc 2, question 5); wire format (doc 4, question 4); names or ids on the wire, plan visibility,
   resource limits (doc 4, questions 6, 7, 9); null in comparisons (doc 4, question 11); distance units (doc 5,
   question 6); UUID v4 generation (doc 3, question 11); primary key columns repeated in the row (doc 1, question 9);
   dictionary benchmark corpus and compaction (doc 7, questions 5 and 6).


## Follow-up checks (2026-11-09)

Method: primary sources fetched; nothing built or run. The generator is `C:\projects\inercya\landpro\WebApi\WebApi.Entities\Guids\SqlSequentialGuid.cs` (read only).

### F1. FoundationDB tuple type codes (doc 3)

Source: https://github.com/apple/foundationdb/blob/main/design/tuple.md

| Claim in doc 3 | Verdict | Correct fact (quoted) |
|---|---|---|
| null `00` | confirmed | "Typecode: `0x00`", 0 bytes |
| bytes `01`, string `02`, `00` escaped as `00 FF`, ended by `00` | confirmed | `b'\x01' + value.replace(b'\x00', b'\x00\xFF') + b'\x00'`; string: `b'\x02' + value.encode('utf-8').replace(...) + b'\x00'` |
| integers `0C`-`1C`, `14` zero, positive big-endian, negative one's complement | confirmed | "`0x0c` is an 8 byte negative number" ... "`0x14` is a zero" ... "`0x1c` is an 8 byte positive number"; "Negative numbers are big-endian one's complement, so -1 is `0x13 0xfe`"; `pack(-5551212) == b'\x11\xabK\x93'` |
| float `20`, double `21`, sign trick | confirmed | float 4 bytes big-endian IEEE: "If the sign bit is set, every bit is flipped. Otherwise, only the sign bit is flipped"; double `0x21`, 8 bytes, same transform |
| false `26`, true `27` | confirmed | "false will sort before true with the given encoding" |
| UUID `30`, 16 bytes | confirmed | "`0x30`, 16 bytes, Network byte order as defined in the rfc" |
| user codes `40`-`4F` for `decimal` and nulls last | confirmed | "`0x40` - `0x4f` ... These type codes may be used by third party extenders without coordinating with us"; open a directory with a specific layer name to avoid conflicts |
| (absent from doc 3) nested tuple | added | `0x05`, ends with `00`, inner null written `00 FF`; not used by Krummi |
| (absent) arbitrary precision | added | `0x0a`/`0x0b` negative, `0x1d`/`0x1e` positive, "Reserved; `0x0b` used in Python and Java"; 9 to 255 bytes with a length byte; not used |
| (absent) versionstamps | added | `0x32` 80-bit (reserved), `0x33` 96-bit (Python, Java, Go); not used |

The extensions (ulid and permuted uuid under `30`, timestamp/date/time/duration as integers, decimal under `40`, descending by inverting bytes, nulls last `4F`) are Krummi's, not in tuple.md. A permuted uuid or a ulid under `30` is not the standard UUID value, so a stock tuple decoder reads it as a UUID with other bytes. No code in doc 3 was wrong; its "recalled, to verify" notes were replaced by the verification.

### F2. SQL Server `uniqueidentifier` order

| Claim | Verdict | Correct fact |
|---|---|---|
| SQL Server compares bytes 10-15, 8-9, 6-7, 4-5, 0-3 | confirmed | https://learn.microsoft.com/en-us/archive/blogs/sqlprogrammability/how-are-guids-compared-in-sql-server-2005 : "The comparison is made by looking at byte "groups" right-to-left, and left-to-right within a byte "group". ... we look at bytes {10 to 15} first, then {8-9}, then {6-7}, then {4-5}, and lastly {0 to 3}." |
| .NET `SqlGuid` uses the same order | confirmed | https://github.com/dotnet/runtime/blob/main/src/libraries/System.Data.Common/src/System/Data/SQLTypes/SQLGuid.cs : `rgiGuidOrder = [10, 11, 12, 13, 14, 15, 8, 9, 6, 7, 4, 5, 0, 1, 2, 3]`; loop `byte b1 = xBytes[rgiGuidOrder[i]]; byte b2 = yBytes[rgiGuidOrder[i]]; if (b1 != b2) return (b1 < b2) ? LT : GT` |
| numbering is that of `Guid.ToByteArray()` | confirmed | https://learn.microsoft.com/en-us/dotnet/api/system.guid.tobytearray : "The order of the beginning four-byte group and the next two two-byte groups is reversed, whereas the order of the last two-byte group and the closing six-byte group is the same." Example: `35918bc9-196d-40ea-9779-889d79b753f0` gives `C9 8B 91 35 6D 19 EA 40 97 79 88 9D 79 B7 53 F0`. `SqlGuid.Compare` fills both byte spans with `TryWriteBytes` (the same layout). |
| Microsoft support post "Lesson Learned #497": "byte by byte left to right" | wrong for SQL Server | contradicts the two sources above (seen in search results only; not relied on) |

In RFC 9562 bytes `r0..r15` (text order): `ToByteArray()` = `r3 r2 r1 r0 | r5 r4 | r7 r6 | r8 r9 | r10..r15`. The key whose bytewise order reproduces SQL Server's is `r10 r11 r12 r13 r14 r15 | r8 r9 | r7 r6 | r5 r4 | r3 r2 r1 r0`. Doc 3's first reading was right and is now stated as settled; its second reading (numbers applied to the RFC bytes) is wrong.

Note on the blog's example: g1 `55666BEE-B3A0-4BF5-81A7-86FF976E763F`, g2 `8DD5BCA5-6ABE-4F73-B4B7-393AE6BBB849`. The last six text bytes are `86FF976E763F` and `393AE6BBB849`; byte `86 > 39`, so by the stated rule g1 > g2, yet the post says "@g2 is indeed greater". The example's conclusion looks mistaken; the rule and `SqlGuid.cs` agree with each other. The PRD should pin test vectors from a real SQL Server `ORDER BY`.

### F3. The user's generator

`SqlSequentialGuid.CreateGuid` builds a 16-byte span and calls `new Guid(bytes)` (the `ToByteArray()` layout), little-endian branch:
- bytes 10-15: the low six bytes of the Unix milliseconds, big-endian (the 8-byte big-endian time is written at bytes 8-15; bytes 8-9 are then overwritten by `r0`).
- bytes 8-9: `r0`, 16 bits big-endian.
- bytes 6-7: `r1 >> 48`; bytes 4-5: `(r1 >> 32)` low 16 bits; bytes 0-3: low 32 bits of `r1`; each big-endian.

SQL Server compares {10-15} (time), {8-9} (`r0`), {6-7} (`r1` bits 63-48), {4-5} (bits 47-32), {0-3} (bits 31-0): the order is (time, r0, r1), confirmed, with `r1` in numeric order. The textual form (`ToString()`) is not time ordered; only the SQL Server order is.
