# Vector columns and filtering by distance, without vector indexes

Technical design, document 5 of the [tables design set](README.md). Scope agreed with the user: "Me interesa que se
pueda filtrar por distancia con los vectores, con fuera de alcance de momento los índices vectoriales." Status:
reconciled 2026-11-09 with the user's decisions (README: "Me valen") and the [fact-check](verification.md); nothing
here is implemented.

Figures marked *estimate* are derived from first principles or from published material and must be measured with the
engine's own benchmark method ([plan.md](../../performance/plan.md)) before a PRD promises them. No benchmark was run
for this document.

## Summary

1. A vector is a column type with a fixed element type and a fixed dimension declared in the schema: `f32`, `f16`,
   `i8` (scalar-quantized by the caller, with a stored scale) and `bit`. There is no `bf16`. The maximum dimension is
   **4,096** for every element type, `bit` included (decided).
2. Stored inline in the row as a plain little-endian array, with no per-row header and no padding (one layout, shared
   with document 1). A cosine column also stores the vector's L2 norm beside it. The row is one tree value: a 768-dim
   `f32` row (3,134 bytes) fits a leaf, two to a leaf; a 1,536-dim one (6,144 bytes) exceeds the 4,078-byte leaf cell
   limit and always lives off its leaf, in the large-value path (up to 256 MiB); the cap is a policy, not the page.
3. Distances are `l2_squared`, `dot` (negated, so smaller is nearer), `cosine` and `hamming` (for `bit`). Kernels are
   safe Rust: a fixed number of independent accumulator lanes over `as_chunks`, exactly like `byte_sum`, with the
   summation order written in the code. That order makes the result bit-identical on every platform and between runs
   (decided), so a distance filter returns the same rows under any plan.
4. Queries without an index are an exact scan: "distance < t" as a row filter and "k nearest" with a bounded heap
   (ties broken by primary key, `k` up to 10,000), both applied after every index-able predicate (pre-filter), one
   thread per query. The planner costs them as `rows x dim x bytes` of streaming.
5. NaN and infinities are rejected on write and in queries, and so are zero vectors in cosine columns. A vector is
   never a primary or index key. No new dependency in v1; `half` is only a test oracle for the in-engine f16
   conversion.

## Requirements

From the root PRD and the scope of the set:

- **R1, filter by distance.** A query can keep rows whose vector is within a distance of a query vector, and can
  return the k nearest. Vector indexes are out of scope for now (user's words above); the design must not close the
  door on adding one later.
- **R2, D11 zero-copy reads.** A read returns a reference into a page. A vector read for a distance must not be copied
  into an owned buffer per row.
- **R3, D12.** `unsafe` only in the pager, in Krummi's own code. Distance kernels are safe Rust and need no dependency.
  The policy for third-party code that contains `unsafe` is one for all documents (document 4, question 5): D12 governs
  the engine's own code, a dependency is judged case by case and recorded by annotation (the pager's `lz4_flex`, the
  benchmark tool of 2026-11-03). The vector crates below are excluded for determinism and cost, not on that ground.
- **R4, determinism.** The same data and the same query give the same rows on every platform (Krummi files are portable
  between machines; a result that depends on the CPU is a support problem). The planner (doc 4) also needs a stable
  answer for the same predicate evaluated through a different plan.
- **R5, predictable cost.** The planner must be able to estimate the cost of a distance predicate from the schema and
  the row count alone, to place it after cheaper filters.
- **R6, types.** The user asked for vector next to ULID and UUID among the column types
  ([01-row-format-and-types.md](01-row-format-and-types.md) owns the row encoding; this document gives it the vector
  payload).

## Options compared

### 1. Representation

**Element type.** What the systems offer:

| System | Types | Max dimension | Notes |
|---|---|---|---|
| pgvector | `vector` (f32), `halfvec` (f16), `bit`, `sparsevec` | 16,000 (`vector`, `halfvec`); indexing limits: 2,000 (`vector`), 4,000 (`halfvec`), 64,000 (`bit`), 1,000 non-zero elements (`sparsevec`) | Typed dimension is optional (`vector(768)`); values over about 2 KiB go out of line (TOAST) |
| sqlite-vec | `float[N]`, `int8[N]`, `bit[N]` in a `vec0` virtual table | 8,192 | Fixed N per column; rows live in chunk blobs |
| DuckDB | `FLOAT[N]` (ARRAY, fixed size) | not limited by the type | `array_distance`, `array_cosine_distance`, `array_inner_product`; fixed N checked |
| LanceDB | fixed-size list of f16/f32/f64/u8, binary | none by type | Columnar (Lance); distance in the query engine, indexes optional |
| RavenDB / Voron | `Single`, `Int8`, `Binary`, `Text` (`VectorEmbeddingType`) | none | Embeddings stored inside an HNSW graph in Voron, see below |

Observation: every system fixes the dimension per column, and f32 plus a compact type (f16 or i8 and bit) is the common
set. RavenDB's `Int8` stores the quantized bytes followed by a 4-byte magnitude (`CosineDistanceI8` reads
`sizeof(float)` trailing bytes), so a quantized vector carries its own scale.

**Size against the page.** Krummi pages are 8 KiB, and a leaf cell holds at most 4,078 bytes (`MAX_LEAF_CELL`,
`crates/krummi-tree/src/node.rs`), so a value over 4,074 bytes minus its key lives off its leaf. A 768-dim f32 row
(3,134 bytes) is inline, two to a leaf. A 1,536-dim f32 vector is 6,144 bytes: it is always off its leaf, in a
contiguous run of pages, whatever else the leaf holds. Two choices:

- *Inline with the row* up to the normal leaf-value limit; above it the existing large-value mechanism stores the
  vector in pages of its own (V2: a single contiguous slice without copying). No new storage code.
- *A separate vector region per column* (sqlite-vec chunks, Lance columns): contiguous scan, but a second storage
  structure to log, free and verify. Out of proportion without an index.

Recommended: inline, with large-value spill as it exists. The cost of spill is one extra page read per row. A table
whose rows are mostly one big vector scans as fast as a dedicated region would, since the vector pages are also
contiguous when written together. Document 2 settled the layout: the table is clustered by primary key and the row,
vector included, is the value of one tree entry, so a spilled row is one contiguous run reached from its key; a scan
that needs only primary-key columns never reads the runs. Measure a scan of a table with 1,536-dim vectors before the
PRD states a figure.

**Alignment.** A page payload is not 4-byte aligned in general: a row starts at an arbitrary offset in the leaf, so
`bytemuck::cast_slice::<u8, f32>` would panic on an unaligned slice (`try_cast_slice` returns an error instead).
Options:

| Option | Cost | Fits R2/R3 |
|---|---|---|
| Copy to an aligned `Vec<f32>` per row | A 3-6 KiB memcpy per row, roughly doubling memory traffic | No (breaks R2 in spirit) |
| `f32::from_le_bytes` on 4-byte chunks inside the kernel | Free on x86 and arm64: the compiler turns `from_le_bytes` of a `[u8;4]` taken from `as_chunks::<4>()` into an unaligned load, and it vectorizes | Yes |
| Pad each vector to a 4-byte offset in the row | Row-format complexity, and still not aligned to the page | Unneeded |

Decided: the kernel reads bytes (`&[u8]`) and converts in-register with `from_le_bytes`. This is the same idea as
`byte_sum` taking `&[u8]`. The stored format is little-endian regardless of the host, which is also what R4 needs.
Document 1's first draft proposed padding vectors to 4 and 2 bytes "for the fast aligned path"; it is dropped, because
the row start is not aligned in the page and the padding could not make the vector aligned (contradiction 5 of the
fact-check).

**Normalization stored or not.** pgvector stores what is inserted and offers `l2_normalize`; RavenDB computes cosine on
raw values. Options: (a) store as given and compute cosine with both norms every time (3 accumulations per element
pair: dot, |a|², |b|²); (b) store a normalized copy (cosine becomes a dot product, one accumulation), at the cost of
modifying user data or a per-column flag; (c) store the norm beside the vector (8 bytes, `f64` since R3) so only dot is needed per
row. Decided (c) for a cosine column (a column flag, called `cosine` here; its name is the PRD's): the norm is stored
beside the vector as an `f64` (R3, below), user data is intact, the scan pays one accumulation, and the query's norm is computed
once. The norm is computed at write by the same fixed-order kernel (then one `sqrt`), so it is bit-identical on every
platform. For `i8` the scale is not the norm: the stored norm is that of the dequantized vector (the exact integer sum
of squares, times the scale squared, then `sqrt`). A cosine column of `f32`, `f16` or `i8` elements pays the 8 bytes;
`bit` columns use Hamming only.

### 2. Distance functions

| Function | Formula | Element types | Order |
|---|---|---|---|
| `l2_squared` | Σ(aᵢ-bᵢ)² | f32, f16, i8 | smaller is nearer; no square root needed for comparison |
| `dot` | -Σaᵢbᵢ (pgvector's `<#>` is also negated) | f32, f16, i8 | smaller is nearer |
| `cosine` | 1 - Σaᵢbᵢ/(‖a‖‖b‖) | f32, f16, i8 | smaller is nearer; zero norm is an error or NULL, see questions |
| `hamming` | popcount(a xor b) | bit | integer, exact |

A threshold on `l2` is stored and compared as squared distance (the planner compares with t² and avoids a `sqrt` per
row). The `i8` kernels accumulate in `i32`: with 4,096 dimensions the worst sum of products (128² x 4,096 = 67.1 M,
with `-128` times `-128`) fits `i32` with room, so integer results are exact and order-independent.

**NaN and infinity.** Floating-point storage refuses them (decided). pgvector rejects both on insert ("NaN not allowed
in vector", "infinite value not allowed in vector", `vector.c`). Krummi rejects NaN and ±inf on write for
`f32`/`f16`; the dimension check and this check are both write-time errors. The query vector gets the same
validation.

**Arithmetic in `f64`** (corrected 2026-11-10 after the Codex review, R3). Finite inputs do not make finite results
in `f32` arithmetic: identical vectors `[1e20]` square to +inf, so cosine computes `1 - inf/inf`, NaN; identical
`[1e-30]` underflow to `0/0`, NaN; and the dot product of `[1e20, 1e20]` and `[1e20, -1e20]` adds +inf and -inf, NaN.
The first version claimed that a distance over stored data is never NaN; Codex compiled and ran these cases. So every
floating kernel converts each element to `f64` (exact for `f32` and `f16`) and accumulates in `f64`:

- the product of two `f32` values is exact in `f64` (24 + 24 significant bits, under 53), so no product rounds;
- the largest product, `f32::MAX`², about 1.2e77, times 4,096 terms is far below `f64::MAX` (1.8e308): no overflow;
- the smallest nonzero product, the `f32` subnormal minimum squared, about 2e-90, is far above `f64`'s smallest
  subnormal (4.9e-324): a nonzero product never underflows to zero;
- `i8` kernels keep the exact `i32` sum and multiply by the scales in `f64` (scale² up to about 1.2e77 times the sum,
  at most 6.7e7: no overflow).

Then, for vectors the write validation admits (finite, and nonzero in a cosine column), the sum of squares is
positive and finite, the norm is positive and finite, and every distance is finite: never NaN, never infinite. The
norm is stored as `f64` (8 bytes) and distances are `f64`. The order "smaller distance first" is total over finite
values (`f64::total_cmp` for the heap, no panics). Cost: `f64` lanes are half as wide as `f32` lanes, so a scan's
arithmetic may take up to about twice as long (estimate, not measured); the golden test below pins the bits, and the
measurement decides whether a faster path is worth a second, exactly equivalent kernel.

**Determinism.** Float addition is not associative. Two scans that add in a different order can differ in the last
bits, and a row at `distance == t - ε` could appear in one run and not in the other. Does the filter need bit-exact
results? For a query result that two plans (a scan with a pre-filter, a future index) must agree on, yes; otherwise the
set of rows depends on the plan, which is a bad property in an engine that is a base for a cost-based planner.

The good news: in safe Rust this comes almost free. LLVM does not reorder float additions (no fast-math in stable Rust),
so a plain `for` loop summing floats is neither vectorized nor reordered, and a manually written 8-lane or 16-lane
accumulator array is vectorized *and* has a fixed order defined by the source. Rust also does not fuse
multiply-add by itself. So the specification is: lane `j` accumulates elements `j, j+L, j+2L, ...`; lanes are then
added in index order with a fixed tree; the tail elements are added after, in order. With `L` fixed in the format
(proposed `L = 16`, a multiple of both 4-wide NEON and 8-wide AVX2 registers) every platform computes identical bits,
and a unit test can pin it against a scalar reference that follows the same order. This deliberately forbids hardware
`fma` (different rounding) and runtime dispatch to kernels with a different `L`.

A scalar reference with the same order only proves that the kernel matches the reference on the platform that runs
the test. Identical bits across platforms also depend on the conversions (`f16` to `f32` and `i8` to the accumulator
type are exact; `sqrt` and division are correctly rounded by IEEE 754) and on the compiler never contracting or
reordering, which no source review proves (added 2026-11-10 after an external review). So the property is checked by
results, not by the code: a test with fixed vectors, including values near overflow, subnormals and lengths that leave
a tail, and the cases of R3 above (identical `[1e20]` and `[1e-30]` under cosine, giving 0 or within a few ulps of it,
and the dot product of `[1e20, 1e20]` and `[1e20, -1e20]`, giving 0), asserts the exact `u64` bit pattern of every
distance and norm, and that none is NaN or infinite, and CI runs it on every platform it
builds (today win-x64 and linux-x64); a new platform joins only when that test passes on it.

Cost of that choice: with `L = 16` on AVX2 the compiler holds 2 ymm accumulators; throughput is limited by the
multiply-add dependency chain, about the same as `L = 32`. *Estimate:* within 10-20 % of a hand-tuned kernel with
4 accumulators; this must be measured.

### 3. Fast distance in safe Rust

| Approach | Safe | Stable | Runtime CPU dispatch | Fixed summation order | Fit |
|---|---|---|---|---|---|
| Fixed-lane loops over `as_chunks` (like `byte_sum`) | yes | yes | no (compiled for the baseline target, SSE2 on x86_64) | yes, by construction | **Recommended** |
| `std::simd` (portable SIMD) | yes | **no, nightly only** (feature `portable_simd`) | no | yes | Blocked by stable toolchain |
| [`wide`](https://crates.io/crates/wide) | public API safe; wide 1.x depends only on `bytemuck` and calls `core::arch` intrinsics itself | yes | no: build-time detection only (its README says runtime detection does not work with `wide`) | yes | Possible later; adds a dependency |
| [`pulp`](https://lib.rs/crates/pulp) | safe abstraction (built for `faer`), runtime dispatch | yes | yes | depends on the kernel written | Possible later; its internals use `unsafe` inside the dependency |
| `simsimd` (now NumKong; crate `simsimd` last at 6.5.16) | **no**: C 99 core with Rust bindings; callers cast `f16` slices with `from_raw_parts` | yes | yes (C side) | no: CPU-dependent instruction choice | Excluded: C toolchain, `unsafe` at the call and CPU-dependent results (R4) |
| `half` | safe public API; on x86-64 it uses the F16C hardware conversion on stable Rust with runtime detection under its default `std` feature (half 2.6 and 2.7.1; the `use-intrinsics` feature is deprecated and `nightly` only adds loongarch64), so its internals contain `unsafe` | yes | yes (conversion only) | n/a: f16 to f32 is exact | Candidate for f16; kept as test oracle (below) |

On the baseline target, `target-cpu` is not `native`, so auto-vectorization produces SSE2 code (128-bit). An AVX2 build
is possible with `RUSTFLAGS="-C target-feature=+avx2"` or `-C target-cpu=x86-64-v3` in the distributed binary, but it
makes the library fail on older CPUs; this is a packaging decision and belongs to document 6 (C interface and
binding). The fixed-order design is *independent* of the vector width: AVX2 and SSE2 produce the same bits for the same
`L`, which is why the width can be a build option later without changing results.

**f16.** There is no `bf16` (decided). The first draft argued for a hand-written conversion because `half` converts
in hardware "only with a nightly feature"; the fact-check found that wrong (above), so the argument is re-read. The
conversion `f16 -> f32` is exact for every finite value, so any implementation gives the same bits and determinism
(R4) is no argument either way. What remains: a hand-written conversion is a few integer operations on a `u16`,
safe, vectorizes with `as_chunks`, and adds no dependency (about 20 lines plus an exhaustive test over all 65,536
patterns against `half`, kept as a `dev-dependency` oracle). The F16C path of `half` could be faster on x86-64, both
CI platforms (win-x64, linux-x64) have F16C in practice, but it brings a dependency with `unsafe` inside. Decision:
write the conversion in the engine for v1; revisit with `half` if a benchmark shows f16 scans bound by the conversion.

**Published throughput for brute force.** I did not find a trustworthy per-vector figure from a published benchmark
that I could cite for 768-dim f32 on AVX2, and I did not run any. The estimate below is derived:

- 768 f32 = 3,072 bytes, 96 AVX2 multiply-adds (8 lanes). At 2 FMA ports and ~4 cycles latency with 4+ accumulators the
  compute bound is about 50-100 cycles per vector, so *estimate* 30-60 M vectors/s/core if the data is in L1/L2.
- Real scans are bandwidth-bound. With data in the OS page cache and a single core streaming at roughly 8-12 GB/s,
  3,072 bytes per vector gives about 3 M vectors/s/core. For 1,536-dim f32 (6 KiB) it is about 1.5 M/s.
- Smaller types help directly: f16 halves the bytes (about 6 M/s), i8 quarters them (about 12 M/s), bit-vectors of 768
  bits are 96 bytes (hundreds of millions per second, popcount-bound).

So 1 M x 768-dim f32 costs about 0.3-0.5 s on one core, 10 M about 3-5 s: acceptable for an exact filter after other
filters have cut the candidate set, not acceptable as an interactive full-table scan at 100 M. That is the stated
trade-off of leaving indexes out of scope. pgvector's documentation describes the same: exact search is the default
and it is "perfect recall" but linear; the index is an approximation added for speed.

### 4. Query semantics without an index

- **Threshold filter**: `distance(col, q) < t` (and `<=`). A row-level predicate, usable inside any `And`/`Or` of the
  query tree of doc 4. It cannot use a secondary index (the planner has none for it), so it is always a *residual
  filter*.
- **Top-k nearest**: `order by distance(col, q) limit k`. A scan feeding a bounded max-heap of size k: one comparison
  against the heap's top per row, `O(n log k)` worst case, usually `O(n)` since most rows do not enter. Ties broken by
  primary key so the result is deterministic. Memory is `k x (distance + key)`.
- **Combination with other filters**: the planner runs index-backed predicates first (pre-filter), producing a set or a
  stream of primary keys, then evaluates the distance only on those rows. This is the right order because the distance is
  the most expensive predicate by one to three orders of magnitude (3 KiB of arithmetic against a key comparison).
  pgvector's approximate indexes struggle here (post-filtering leaves fewer than k rows); an exact scan with a
  pre-filter never loses recall, which is a selling point of this design.
- **Top-k plus pre-filter**: exact by definition; the heap sees only rows that pass.
- **Distances with a threshold and k together**: heap bounded by k, and a row is added only if `d < t`; once the heap is
  full its top acts as a tighter threshold. With `l2_squared` and f32 an early-exit (stop summing when the partial sum
  exceeds the current bound) is valid because all terms are non-negative, but it is a data-dependent branch inside the
  kernel and conflicts with the fixed-lane layout; check it every 64-256 elements, and only if measurement shows a gain.
  It is not applicable to `dot`/`cosine` (terms can be negative).
- **Cost model for doc 4**: `cost(distance) = rows_in x dim x bytes_per_element / bandwidth`, i.e. a per-row constant
  per column, known at plan time (the dimension and type are in the schema). Vector columns have no histogram (doc 3), so the
  selectivity of `d < t` is unknown; use a default (for example 0.1 %-1 %) unless the user passes a hint, and place the predicate last among the
  filters. For top-k the output cardinality is exactly `min(k, rows_in)`, which the planner can use for joins and
  limits. A planner that sees a missing pre-filter on a large table should expose the estimated rows scanned in the
  explanation, so a user sees why a query is slow.
- **Parallelism**: the scan is trivially data-parallel by key range; whether Krummi's read transactions can be split
  across threads is a decision of doc 4 and the engine's concurrency model. The design here does not require it.

### 5. What the other systems do, in short

- **pgvector** (documented behaviour): types `vector`, `halfvec`, `bit`, `sparsevec`; operators `<->` (L2), `<#>`
  (negative inner product), `<=>` (cosine), `<+>` (L1), `<~>` (Hamming), `<%>` (Jaccard); exact search by default (`ORDER BY
  embedding <-> $1 LIMIT 5`) and filtering with `WHERE`; HNSW and IVFFlat are optional, approximate indexes. The types'
  rows go through TOAST over about 2 KiB, so a 1,536-dim vector is stored out of line.
- **sqlite-vec**: `CREATE VIRTUAL TABLE v USING vec0(embedding float[768])`; KNN query `WHERE embedding MATCH ? AND k = 10`;
  brute force over chunked blobs, no ANN index in the latest stable release (v0.1.9; the v0.1.10-alpha tags and main contain DiskANN and an experimental IVF); `vec_distance_l2`, `vec_distance_cosine`,
  `vec_distance_hamming` also callable on plain blobs. Types: float32, int8, bit.
- **DuckDB**: fixed-size `FLOAT[N]` array type with `array_distance`, `array_cosine_distance`, `array_inner_product`;
  vectorized execution over contiguous child arrays; the `vss` extension adds an HNSW index. A plain scan is exact.
- **LanceDB**: columnar fixed-size-list vectors; brute-force "flat" search is the default until an index is built;
  distance types L2, cosine, dot, hamming; pre-filter is the default for `where` (post-filter optional).
- **RavenDB**: see below.

All four agree on the shape this design proposes: fixed dimension, exact scan as the baseline, filter before distance.

### 6. RavenDB: what Voron and Corax store

Read from `C:\projects\ravendb\src`:

- `Voron/Data/Graphs/VectorEmbeddingType.cs`: `Single`, `Int8`, `Binary`, `Text`.
- `Voron/Data/Graphs/Hnsw.cs` and siblings: an HNSW graph held in Voron containers/trees. `Hnsw.SimilarityMethods.cs` defines
  `CosineSimilaritySingles`, `CosineSimilarityI8`, `HammingDistance`; the I8 vector is stored with a trailing 4-byte
  magnitude. A vector is read through `Hnsw.NodeReader.ReadVector` as a span over page memory, and the cosine kernel
  casts it to `float` via `MemoryMarshal.Cast` (unaligned access is fine in .NET on x86/arm64).
- `Corax/Mappings/VectorOptions.cs`: per-field `NumberOfCandidates`, `NumberOfEdges` and the embedding type.
- `Corax/Querying/IndexSearcher.VectorSearch.cs`: `VectorSearch(metadata, vector, minimumMatch, numberOfCandidates,
  isExact, isSingleVectorSearch)`: the `isExact` flag chooses an exhaustive scan against the HNSW traversal, and
  `minimumMatch` is a similarity threshold, i.e. RavenDB too has both "filter by threshold" and "nearest" semantics.
- Distance kernels use `System.Numerics.Tensors` and hardware intrinsics with runtime dispatch (`AdvInstructionSet`).
  This is the unsafe, CPU-dependent route; Krummi forbids it (D12) and wants identical results (R4).

Lesson: RavenDB's index is the HNSW graph, and the exact mode is a fallback. Krummi starts from the exact mode and a
plain column; the graph is a later, separate index kind.

## Decided design

**Type.** `vector(elem, dim)` with `elem` in `{f32, f16, i8, bit}`. `dim` is part of the column type; the engine
checks it on every write. The maximum dimension is 4,096 for every element type, `bit` included (decided; the first
draft allowed 32,768 bits for `bit`, which is dropped for one rule; a `bit` vector of 4,096 dimensions is 512 bytes).
The cap is a constant of the schema validator; the storage allows more through large values.

**Storage format of one value** (inside the row, document 1 owns the framing):

```text
f32 : dim x 4 bytes, little-endian IEEE 754 binary32
f16 : dim x 2 bytes, little-endian IEEE 754 binary16
i8  : dim x 1 byte, then scale: 4 bytes f32 (value = byte x scale)
bit : ceil(dim / 8) bytes, bit i = (byte[i / 8] >> (i % 8)) & 1; unused high bits of the last byte are zero
+ for a cosine column: 8 bytes, the L2 norm as f64, after the elements and the scale
```

No length prefix: the length is a function of the column (element, dimension, cosine flag). Fixed length helps the row
codec (doc 1), which repeats this layout, and the key-value size accounting. Quantization is the user's responsibility in v1 (the engine stores `i8`/`bit` given by the caller; it
does not quantize); a helper `quantize` can come in the bindings.

**Kernels.** One generic function per distance written over `&[u8]` with `as_chunks::<{ 4 * L }>()`, `L = 16`, that
converts with `from_le_bytes` and then to `f64` into `[f64; L]` accumulators (R3), adds lanes in a documented fixed order, then the tail
(`< L` elements) element by element. `f16` goes through the in-engine bit conversion; `i8` accumulates in `i32` (exact);
`hamming` uses `u64::count_ones` over `u64` chunks (the compiler emits `popcnt`/`cnt` where the target has it and a bit
trick otherwise; result is exact). Kernels live in a `distance` module with no `unsafe` and no dependency.

**Tests that pin it** (per the repository's test discipline): each kernel against a scalar reference that follows the
same lane order, bit for bit, on lengths `0, 1, 15, 16, 17, 255, 4096` and offsets `0, 1, 7` (as the `byte_sum` test does, lengths 0..65,537 and
offsets 0, 1 and 7: alignment must not matter); `f16` conversion over all 65,536 patterns; a golden vector (a fixed
input and the expected bit pattern of its distance). CI builds and tests only win-x64 and linux-x64 today
(`.github/workflows/ci.yml`: `windows-latest`, `ubuntu-latest`) and there is no arm64 build or test, so the
cross-platform bit-identity is guaranteed by construction and verified on x86-64 under two operating systems; the golden
vector is committed so that an arm64 job, when one exists, checks it at once. Also: rejection of NaN/inf, of a zero
vector in a cosine column, and of a wrong dimension.

**Query surface** (feeds doc 4): expression `Distance(column, query_vector, metric)` returning `f64` (R3), usable in
`Where(expr < t)`, in `OrderBy(expr)` with `Limit(k)`, and projectable. The query vector is validated like a stored one
(finite, right dimension). A bounded heap implements `OrderBy+Limit` on a distance expression when no index delivers
the order.

**Planner rules** (for doc 4): (1) a distance predicate is a residual filter, placed after every index-backed one;
(2) cost per row = `dim x bytes_per_element` streamed; (3) `Where` with threshold and `OrderBy+Limit` combine as above (ties broken by primary key, `k` up to 10,000);
(4) the explanation prints rows scanned and bytes streamed.

**Future-proofing for an index.** A vector index (HNSW, IVF) would be a new secondary index kind (doc 3) holding primary keys;
the query tree needs no change because `Distance < t` and `OrderBy(Distance)` are exactly what an index would accelerate.
The kernels are reused as the index's distance function and for re-ranking, and since they are bit-exact, a future
approximate index re-ranks candidates to the same distances the scan produces.

## Costs and risks

| Item | Cost or risk | Mitigation |
|---|---|---|
| Full scan speed | About 3 M vectors/s/core at 768-dim f32 when bandwidth-bound (*estimate*); 10 M vectors is several seconds; one thread per query in v1 | Pre-filter, f16/i8/bit types, planner explanation; measure with the benchmark method before the PRD states a figure |
| SSE2 baseline | Without `target-cpu`, autovectorization is 128-bit; AVX2 could be 1.3-2x faster on compute-bound kernels | Packaging decision (doc 6); results are width-independent by design |
| Fixed lane order | No hardware `fma`, no runtime dispatch to different-order kernels; some peak speed given up | Chosen deliberately for R4; measure the gap and report |
| Rust codegen | Auto-vectorization is not guaranteed across compiler versions | A micro-benchmark gate (like the engine's performance method) and a test on the generated speed class; fall back to wider `L` if needed |
| Large vector rows | A row with a 6 KiB vector always spills off its leaf (cell limit 4,078 bytes), and a scan of its other columns reads the run; a 768-dim row stays inline | Doc 2's clustered layout keeps the row one contiguous run; a companion tree for large columns is a later remedy (doc 1) |
| Cosine on zero vectors | Division by zero | Rejected at write in a cosine column (decided); a zero query vector is rejected too |
| `i8` scale | Distances between vectors with different scales need rescaling | Compute `dot` as `Σ aᵢbᵢ x sa x sb` in f64 after the exact integer sum (R3); accuracy documented |
| Overflow | Huge finite f32 squares overflow to +inf | Treat +inf as "greater than any threshold"; documented |
| Dependency policy | `half`, `wide` and `pulp` are unnecessary for v1; each has `unsafe` or build-time dispatch inside | v1 has no dependency; revisit with a measured gain (policy in doc 4, question 5) |
| Memory for top-k | `k` is user-supplied; each heap entry holds a distance and the primary key (up to 2,048 bytes) | `k` is capped at 10,000 (decided): at most 10,000 x (4 + key length) bytes, about 20 MB for the longest keys, 0.2 MB for a 16-byte key |

## Product questions

1. **Element types in v1.** *Decided 2026-11-09:* `f32`, `f16`, `i8`, `bit`; no `bf16` (README: "Me valen").
2. **Maximum dimension.** *Decided:* 4,096, one value for every element type.
3. **Exact reproducibility of distances across platforms.** *Decided:* bit-identical, fixed lane order, no hardware
   `fma`, so a distance filter returns the same rows under any plan. It costs perhaps 10-20 % (*estimate*).
4. **Cosine and stored norm.** *Decided:* the norm is stored beside the vector of a cosine column.
5. **NaN, infinity and zero vectors.** *Decided:* NaN and infinities are rejected on write and in queries; zero vectors
   are rejected in cosine columns.
6. **Distance units for the threshold.** *Still open.* Options: the user writes the plain `l2` distance and the engine
   squares it internally; the user writes the squared distance. Recommendation: plain L2 distance (matches pgvector and
   the other systems); the engine compares squares.
7. **Quantization.** *Decided:* the caller quantizes `i8` and `bit` vectors; a helper in the binding may come later.
8. **Parallel scan.** *Decided for v1:* one thread per query; the heap and kernels are written so a split by key range
   is possible later.
9. **A result limit and `k` cap.** *Decided:* top-k by exact scan with a bounded heap, ties broken by primary key, `k`
   up to 10,000, as an error that says how to raise it.
10. **Vectors in a key or secondary index.** *Decided:* never; a vector has no useful order and the key encoding (doc
    3) stays simple.

## Sources

Repository files read:

- `C:\projects\Krummi\docs\design\tables\README.md` (scope, verbatim user quotes)
- `C:\projects\Krummi\docs\prd\krummi\krummi.prd.md` (D11, D12 and annotations; 8 KiB pages; large values up to 256 MiB)
- `C:\projects\Krummi\crates\krummi-bench\src\data.rs` (`byte_sum`, 16 lanes with `as_chunks`)
- `C:\projects\Krummi\docs\performance\plan.md` (performance method; referenced, measured figures are not used here)
- `C:\projects\ravendb\src\Voron\Data\Graphs\VectorEmbeddingType.cs`
- `C:\projects\ravendb\src\Voron\Data\Graphs\Hnsw.SimilarityMethods.cs` (and `Hnsw.cs`, `Hnsw.Constants.cs` by name)
- `C:\projects\ravendb\src\Corax\Mappings\VectorOptions.cs`
- `C:\projects\ravendb\src\Corax\Querying\IndexSearcher.VectorSearch.cs`

Web (searched 2026-11-09; the summaries below come from the search results, details of pgvector, sqlite-vec, DuckDB and
LanceDB are from the maintainers' documentation as I know it and were not re-fetched, so check them before quoting):

- simsimd: <https://crates.io/crates/simsimd/5.5.0>, <https://lib.rs/crates/simsimd>
- pulp: <https://lib.rs/crates/pulp>
- wide: <https://crates.io/crates/wide>, <https://docs.rs/wide>
- half: <https://lib.rs/crates/half>, <https://docs.rs/crate/half/latest>
- State of SIMD in Rust: <https://shnatsel.github.io/state-of-simd-rust-2026/>
- `std::simd`: <https://doc.rust-lang.org/std/simd/struct.Simd.html> (nightly-only)
- pgvector: <https://github.com/pgvector/pgvector>
- sqlite-vec: <https://github.com/asg017/sqlite-vec>
- DuckDB array functions: <https://duckdb.org/docs/sql/functions/array>
- LanceDB: <https://lancedb.github.io/lancedb/>
