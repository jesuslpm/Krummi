# 09 - The serialization format of query trees and plans

Part of the [tables design set](README.md). Written 2026-11-10 at the user's request, as an evaluation of the format
that [04-query-tree-and-planner.md](04-query-tree-and-planner.md) proposes for query trees and
[08-execution-plans.md](08-execution-plans.md) reuses for physical plans, against JSON and other alternatives. It
changes nothing in documents 4, 6 and 8 or in the PRDs until the user decides. Written from those documents, the README
decisions, `tables.prd.md` and the root PRD; external facts were fetched on 2026-11-10 from the sources at the end
(anything not fetched says "from knowledge, not fetched here"). One scratch microbenchmark was run outside the
repository; its figures are marked **measured**, everything else **estimate** or **reasoned**.

The request, in the user's words: "Utilizar JSON para representar tanto los AST lógicos como los planes físicos, con
valores tipados explícitamente cuando sea necesario. Por ejemplo: `{"type":"u64","value":"18446744073709551615"}`
[...] JSON sería exclusivamente el formato de intercambio. [...] El ejecutor no interpretaría JSON durante el
procesamiento de filas. Mantendríamos los parámetros separados de la estructura del AST". And: "Una motivación
importante es el desarrollo asistido por IA. [...] No queremos SQL ni otro lenguaje textual de consultas que requiera
un parser propio."

## Summary

1. **Recommendation: replace the custom binary with one explicitly typed JSON format** (strict I-JSON, RFC 7493) for
   query trees and plans, and use the same JSON for what the planner and `explain` return. Values whose JSON number
   would lose information (`i64`, `u64`, `decimal`, non-finite floats) are strings; types are declared per parameter.
   Inside Krummi the document becomes typed Rust structures once; the executor never sees JSON.
2. **The per-call hot path should not carry the document at all**: prepare once, bind values per call through typed C
   functions (as SQLite's `prepare`/`bind`/`step`), vectors and large byte strings by pointer and length, without
   Base64. This is the answer to small frequent queries in any format, and it is a product decision (question 2).
3. **Measured on this machine**: decoding a small query costs about 1.1-1.5 µs (point lookup) and 5.5-9 µs (five
   predicates, sort, limit, projection) in JSON against 0.25-1.6 µs in a hand-written binary; the documents are 4-6
   times larger. A key-value `get` of the bench costs about 1 µs. So JSON is slower, and the difference matters for
   one-shot point queries; it disappears from the per-call path with prepared statements.
4. **What decides it is not speed but the AI and inspection requirement.** An AI tool authors and reads JSON natively
   (tool calls are JSON objects described by JSON Schema). With the binary, any tool that writes or edits plans
   needs a JSON-to-binary translator: two formats in practice, the hybrid the user wants to avoid.
5. **Runner-up: CBOR** (RFC 8949): exact binary types, a defined deterministic encoding, a Microsoft .NET library. Choose
   it only if measurements show JSON's cost matters after prepared statements exist; it gives up direct readability.
6. **Not a reason for the binary**: canonical bytes. Untrusted input must be normalized anyway, so the plan cache key
   should be a hash of the normalized typed tree, whatever the wire format (doc 4 says both things; see "Claims").
7. Open for the user: changing a format the README records as approved for plans, prepared statements, whether plans
   from `explain` may be edited and resubmitted, the compatibility promise for stored documents, and admitting
   `serde_json`, which contains `unsafe`.

## What is already decided, and what this document may change

| Item | Status | Source |
|---|---|---|
| Queries are a typed tree (AST) built by a fluent API; no text language, no SQL | Decided | README scope: "no quiero hacer lenguaje de texto ni SQL, sino definir las consultas en un AST" |
| C# builds the tree, serializes it and sends it to Krummi | Decided (serialization, not its format) | README scope: "En c# se formaría el AST, se serializaría se enviaría a Krummi" |
| Values travel as separate parameters | Decided | README, query tree entry: "Values always travel as separate parameters" |
| The public interface accepts physical plans from the first version; strict validation | Decided | README, plans entry: "Sí" |
| Plans: "a format of Krummi's own, in the query tree's binary envelope, with operator names aligned with Substrait" | Approved ("Me sirven") | README, Plans entry. **This document proposes to change "binary envelope"**; "Krummi's own format" and the Substrait vocabulary stay |
| Wire format of the query tree | Open (doc 4, question 4) | doc 4 |
| Applications may store plan bytes; Krummi never persists them | Decided | doc 8, question 3 |
| A readable dump that "cannot be read back" (DD9 of the parked documents PRD) | Requirement taken by doc 4 | doc 4, requirement 5. **Questioned here** (question 1) |
| No `Prepare` in the first C surface; "one can be added later" | Proposal of doc 4 | doc 4, "How C# reads rows". **Questioned here** (question 2) |
| Dependencies with `unsafe` admitted case by case if widely used, pure Rust, "without a reasonable safe alternative", annotated | Decided | README; `tables.prd.md`, annotations to D12 |

## Requirements

- Q1. One contract for logical trees and physical plans (shared expression nodes, doc 8), for input and for what Krummi
  returns (planner output, `explain`).
- Q2. Exact representation of every column type of doc 1, including `u64` up to 2^64-1, `decimal(38, s)`, NaN, the
  temporal types and vectors of 4,096 dimensions, and nulls with their declared type.
- Q3. Untrusted input: a bad document is a typed error that locates the fault, never a panic or a wrong result (doc 8
  R1); bounded size, depth and node count.
- Q4. Parameters separate from the structure, so that a shape is reused with other values (README).
- Q5. Generated, read and edited by applications and AI tools, and stored by applications (doc 8, question 3).
- Q6. Usable from Rust, C and C# (.NET 8, AOT friendly, doc 6) with the dependency rule of the README and D12.
- Q7. Small frequent queries must not pay a format cost that dominates their execution.
- Non-requirement: the format is never read by the executor while it processes rows (the user's condition).

## Options compared

The six options the user named. Others considered and dropped in one line: FlatBuffers and Cap'n Proto (doc 4: zero-copy
is worthless at this size, weaker C# support); Rust-only formats such as `bincode` or `postcard` (no C# implementation
of record); building the tree by C calls node by node (no serialization, but no persistence, no tool authoring, and a
chatty boundary).

- **Conventional JSON**: numbers as JSON numbers, strings for the rest, the meaning of each value inferred from the
  schema. RFC 8259: only integers in [-(2^53)+1, 2^53-1] are interoperable, and "Infinity and NaN are not permitted".
- **Explicitly typed JSON**: the user's proposal. Every parameter carries its type; values that a double cannot hold
  exactly travel as strings, as I-JSON recommends ("it is RECOMMENDED to encode them in JSON string values"); bytes in
  Base64.
- **MessagePack**: binary, self-describing; integers up to 64 bits both signs, float32 and float64, `bin`, and `ext`
  types (application numbers 0-127, timestamp is -1). The specification defines no canonical form.
- **CBOR (RFC 8949)**: binary, self-describing; integers 0..2^64-1 and their negatives, half, single and double floats,
  byte strings, tags (bignums 2/3, decimal fraction 4, epoch time 1) and a deterministic encoding (§4.2.1: preferred
  serialization, no indefinite lengths, map keys sorted bytewise).
- **Protocol Buffers**: a schema compiled by `protoc` into code on each side; `oneof` for node kinds. "Protobuf
  serialization is not (and cannot be) canonical."
- **Custom binary (docs 4 and 8)**: magic, version, tagged length-prefixed nodes in pre-order, varints, a separate
  parameter block; hand-written writer in C# and reader in Rust.

### Matrix

| Criterion | Conventional JSON | Typed JSON | MessagePack | CBOR | Protobuf | Custom binary |
|---|---|---|---|---|---|---|
| Implementation and maintenance | Low, but type rules live in the reader's head | Low: derive in Rust, `System.Text.Json` in C#; one converter per type | Low-medium: `ext` codes per Krummi type by hand | Low-medium: tags per type by hand | Medium: `.proto`, `protoc` in both builds, generated types mapped to our AST | Medium: reader and writer per side (about 400 lines each, doc 4's estimate), plus a dump |
| Rust, C, C# | Universal | Universal; in the .NET shared framework (from knowledge) | Libraries on both sides; C# one is third party | `ciborium` 0.2.2; `System.Formats.Cbor` (Microsoft package) | Official on both sides; C not checked | Krummi's own code only; a C user writes an encoder |
| Generation and editing by applications and AI | Native, but silently lossy | Native: tool calls are JSON | Through a converter | Through a converter | Through its JSON mapping | Through a converter |
| Reading, debugging, persistence | Text files, diffs in review | Same | No | Diagnostic notation, with tools | Text or JSON mapping | A dump that cannot be read back (DD9) |
| Size and decode cost | About 5x binary; 1.1-9 µs per small document (measured, typed) | Same | Near binary (estimate) | Near binary (estimate) | Small (estimate) | Smallest; 0.25-1.6 µs (measured) |
| Untrusted input | Depth-limited mature parsers; duplicate keys and number range need strict rules | Same, rules fixed by the format | Mature libraries; not fetched | Well-formed versus valid distinction; decoders must bound nesting (§10) | Unknown fields kept silently | Our own parser, to fuzz |
| Evolution | Version field, unknown fields rejected | Same | Same | Same | Best built-in rules, but keeps unknown fields | Version byte, length-prefixed nodes |
| Exact Krummi types | No: `u64` > 2^53, NaN, decimals | Yes, with strings and Base64 | Integers, floats and bytes exact; decimal, `uuid`, time by `ext` | Integers, floats and bytes exact; tags for decimal and time | Integers, floats and bytes exact; the rest by convention | Yes, by construction |
| Canonical form, cache keys | None (JCS, RFC 8785, is Informational) | None needed: hash the normalized tree | None in the spec | Defined (§4.2) | Not canonical (official) | Only if the decoder rejects non-canonical input |
| One format for trees and plans | Yes | Yes | Yes | Yes | Yes | Yes |
| Dependencies and D12 | `serde_json` contains `unsafe` (3 uses in `read.rs`), or a hand-written reader | Same | `rmp-serde`; `unsafe` not checked | `ciborium`; `unsafe` not checked | `prost` plus `protoc` at build time | None |
| **Verdict** | Rejected: loses values | **Recommended** | Dropped: CBOR does the same with a standard and a Microsoft library | **Runner-up** | Rejected: build tooling, no canonical form, a mapping layer | Rejected unless measurement demands it |

## Performance: where the time goes

| Cost | Custom binary | Typed JSON | Kind |
|---|---|---|---|
| Serialization in the client | Rust encoder: not timed | Rust `serde_json` encode, including a clone of the tree: 0.6 µs (point), 2.8-3.6 µs (small query); C# `Utf8JsonWriter`: not measured | measured (Rust); C# open (B3) |
| Transfer through the C interface | A pointer and a length; no copy | Same | reasoned |
| Deserialization and validation in Rust | 0.25 µs (point), 1.4-1.6 µs (small query) | 1.1-1.5 µs (point), 5.5-9 µs (small query) | measured, structure only; schema binding and type checks not included |
| Planning | Not built. A cache hit needs decode, normalization and a hash of the tree; a miss enumerates a few access paths with histogram lookups | Same | estimate: a few µs, unmeasured |
| Physical execution | A point read by key: about 1 µs (bench load `lecturas-al-azar`, 1,045,630 operations per second, Windows, 2026-11-09, a tree `get`, not yet a table) | Same | measured by the bench |

**The measurement.** A scratch crate decoded the same trees from typed JSON (`serde_json` 1.0.154, `serde` derive) and
from a hand-written tag/varint binary, both into the same owned Rust structures, with the same depth limit; it does no
schema binding or normalization. Machine: i9-13980HX, Windows 11, rustc 1.99.0, release profile, **Balanced power plan,
not the bench plan**, one core, high priority; 15 samples, minimum and median. A first run without pinning was 1.3 to 3
times slower and noisy, so read the figures as orders of magnitude.

| Case | JSON bytes | JSON decode, µs (min / median) | Binary bytes | Binary decode, µs |
|---|---|---|---|---|
| Point lookup (`id = $0`, `u64` max) | 192 | 1.52 / 1.69 (`"op"` tag); 1.08 / 1.18 (outer tag) | 31 | 0.25 / 0.28 |
| Five predicates, sort, limit, project, five parameters | 725 | 9.0 / 9.7 (`"op"` tag); 5.5 / 6.0 (outer tag) | 151 | 1.37 / 1.52 |
| Top-k, `vector(f32, 768)` as JSON numbers | 8,983 | 19.6 / 22.7 | 3,109 | 0.52 / 0.61 |
| Same, vector in Base64 | 4,318 | 4.1 / 4.6 | | |
| Top-k, `vector(f32, 4096)` as JSON numbers | 46,994 | 113.6 / 118.8 | 16,421 | 0.61 / 0.85 |
| Same, vector in Base64 | 22,070 | 14.4 / 16.0 | | |

Readings (reasoned from the table):

- Both decoders are dominated by allocation (owned strings for names, boxed nodes): 151 bytes of binary taking 1.4 µs
  is about 20 allocations, not parsing. Borrowed names or column ids would cut both; the format is not the only lever.
- serde's internally tagged enums (`{"op":"filter",...}`, the friendlier shape) cost 1.4 to 1.6 times the externally
  tagged form (`{"filter":{...}}`), because serde buffers each node before choosing its variant.
- Vectors as JSON numbers are about 40 to 190 times the binary cost; Base64 is 8 to 24 times. Large values belong out of
  the document (bound by pointer), which removes both costs and the 33 % Base64 overhead.
- For a one-shot point query, JSON adds about 1 µs to an execution of about 1 µs plus planning; binary adds about
  0.25 µs. For anything that reads more than a few rows, the decode is noise.

**Prepared statements instead of a faster format.** Doc 4 already caches plans per shape, but a one-shot call still
decodes, normalizes and hashes the whole tree before the cache lookup. A prepared statement pays that once; each call
then binds values (a few nanoseconds per scalar, estimate) and executes. SQLite's `sqlite3_prepare_v2` and
`sqlite3_bind_*` (from knowledge, not fetched here) and PostgreSQL's `PREPARE` (doc 4) are the precedent. With it, the
format's decode cost leaves the per-call path for every query that repeats, which is the case Q7 cares about;
optimizing the interchange format instead would save at most about 1-8 µs per call and still leave normalization and
hashing.

## The type model

Rule: **the type decides the representation, never the value**. A `u64` is always a string, also when it is 20, so a
writer never needs to know where 2^53 is.

| Krummi type | JSON value | Exactness and notes |
|---|---|---|
| `bool` | `true` / `false` | Exact |
| `i8`-`i32`, `u8`-`u32` | number | Exact (inside ±2^53, RFC 8259) |
| `i64`, `u64` | decimal string: `"18446744073709551615"` | Exact; a JSON number would not survive a double-based reader (RFC 8259, I-JSON 2.2), including JavaScript tools and many LLM toolchains |
| `f32`, `f64` | number for finite values (shortest round-trip form); strings `"NaN"`, `"Infinity"`, `"-Infinity"` | Krummi stores `-0` as `+0` and one canonical NaN (doc 1), so `-0` and any NaN payload are accepted and normalized. Exact only with correctly rounded parsing: `serde_json` needs its `float_roundtrip` feature (its manifest: default parsing is best effort, the feature is about 2x slower for floats) |
| `decimal(p, s)` | string `"-1234.50"` | No exponent, at most `s` fractional digits, no rounding; more digits or more than `p` digits is invalid value (N4) |
| `string` | string | JSON escapes; lone surrogates must be rejected (to test) |
| `bytes` | Base64 string (RFC 4648, padded) | Bound out of band in the C API when large |
| `date` | `"2026-11-10"` | Proleptic Gregorian; years outside 0000-9999 need ISO 8601 expanded years (`"+012026-11-10"`), or `date` loses part of its `i32` range (design question D4) |
| `time` | `"13:45:07.123456"` | Microseconds |
| `timestamp` | `"2026-11-10T13:45:07.123456Z"` | RFC 3339, `Z` only; at most six fractional digits |
| `duration` | string of microseconds, `"-90061000005"` | No calendar units (doc 1), so ISO 8601 durations add nothing (D4) |
| `uuid` | `"0190f3c2-7a1e-7b3c-9a10-3f1e2d4c5b6a"` | RFC 9562 text. Removes the C# `Guid` byte-order trap of doc 1: `Guid.ToString()` already prints RFC order |
| `ulid` | 26 characters, Crockford Base32 | |
| `vector(elem, dim)` | Base64 of the element bytes in doc 1's layout (little-endian; `i8` followed by its `f32` scale; `bit` packed) | Bit-exact, which doc 5's bit-identical distances need. A JSON number array for `f32` is readable but costs 5 to 8 times Base64 (measured) and depends on correctly rounded parsing; accept it as input only if tools need it (D5) |
| null | `"value": null` with the declared type | The type stays declared, so a null `i64` and a null `string` are different shapes |

**Where types live.** Three options: on each value (self-describing), in a parameter declaration block, or inferred
from the schema. Recommended: the parameter block declares each parameter's type, including `decimal(p, s)` and
`vector(elem, dim)`, and is part of the shape (doc 4 already makes the parameter type part of the shape); a value is
optional and, when present, sits next to its declaration, which gives the user's
`{"type":"u64","value":"18446744073709551615"}`. Values inside the tree never appear (doc 4: no literals). Inference
from the schema is friendlier but makes a document's meaning depend on the schema it meets; the validator checks the
declared type against the column instead. The fluent APIs fill types in, so only hand or AI authors write them.

**Large parameters outside the document.** Vectors (up to 16 KiB for 4,096 `f32`) and byte strings are bound through
the C API by pointer and length and copied once at bind (estimate: under 1 µs for 16 KiB), so they are never Base64 or
text on the hot path. Inside a self-contained document (persistence, tests, tools) they stay Base64.

## AI-assisted development

**Situation A: Claude writes C# or Rust against the fluent API.** The compiler checks columns and types; Claude never
sees the format. A readable format adds little here: a `ToJson()` of a built query for a log line or a bug report, and
readable golden test files. The quality of the fluent API and of the `explain` output matters far more.

**Situation B: Claude builds, reads or edits queries and plans directly** (through a tool, or by writing a document
that an application stores). This is the README motivation: "Con la IA se podrían construir planes de ejecución para
consultas complejas sin necesidad de un procesador de consultas." Here the format is the interface. Tool calls are JSON
objects described by JSON Schema (MCP: `inputSchema` is "JSON Schema defining expected parameters"); a binary format
needs a translator, which would be a second, JSON, contract. Explain output in the same JSON can be read, changed and
sent back, which DD9 forbids today.

- **JSON Schema** helps tools generate well-formed documents, but cannot express the schema-dependent rules (column
  exists, types match, ranges sorted), and in draft 2020-12 `format` (for `uuid`, `date-time`) is an annotation unless
  a validator opts in. Krummi's validator stays authoritative; its errors should name a JSON Pointer path (RFC 6901)
  plus expected and found, which an AI corrects from more easily than doc 6's byte offset.
- **Code generation** of C# types from the schema is possible but unnecessary: the C# binding writes JSON through its
  own builder.
- **Differential tests** between a logical query and explicit plans run on typed Rust structures, independent of the
  format; JSON makes their fixtures readable in review and lets a failing case be pasted into a bug report.

So readability is worth little in A and a lot in B; B is a stated motivation, not an assumption.

## Product and design decisions, separated

| Decision | Level | Status |
|---|---|---|
| Applications submit logical trees and physical plans | Product | Decided |
| Applications obtain the optimizer's plan and may edit and resubmit it | Product | Open (question 1); DD9 says no today |
| Prepared statements with values bound per call, in Rust, C and C# | Product | Open (question 2) |
| Whether stored documents keep working across Krummi versions | Product | Open (question 3) |
| A published, versioned JSON Schema as part of the product | Product | Open (question 4) |
| A Krummi tool that runs and explains documents (command line, MCP server) | Product | Open (question 5) |
| Admitting `serde_json` under the dependency rule | Product (project policy) | Open (question 6) |
| The interchange format (JSON, CBOR, binary) | Design, but product-visible once documents are stored or authored by tools | Open (question 7) |
| Type encodings, where types live, tags, limits, error shape, libraries, cache key | Design | Proposed below; details for the PRD of module 17 |

The format must follow the product answers, not lead them: if the answer to questions 1 and 5 is no and nobody authors
documents by hand, the binary's drawbacks shrink to the dump and the golden files.

## Recommended design

**One document format, two root kinds.** Query trees and plans share the envelope, the expression nodes and the
parameter block, as docs 4 and 8 already decided for the binary:

```json
{
  "krummi": "query/1",
  "table": "customers",
  "root": { "op": "limit", "count": { "param": 3 },
    "input": { "op": "sort", "keys": [ { "col": "created", "desc": true } ],
      "input": { "op": "filter",
        "predicate": { "op": "and", "args": [
          { "op": "eq", "col": "country", "value": { "param": 0 } },
          { "op": "ge", "col": "created", "value": { "param": 1 } },
          { "op": "in", "col": "status", "values": { "param": 2 } } ] },
        "input": { "op": "scan" } } } },
  "params": [
    { "type": "string", "value": "ES" },
    { "type": "timestamp", "value": "2026-01-01T00:00:00Z" },
    { "type": "list", "element": "i32", "value": [1, 2, 5] },
    { "type": "u64" }
  ]
}
```

A plan is `"krummi": "plan/1"` with doc 8's `refs` (table id and name, `schema_version`, indexes) and its operators
(`index_scan`, `key_lookup`, `top_k`, ...); the planner and `explain` return this form, `explain` adding estimated and
actual rows per node in fields the validator ignores on input. Parameter 3 has no value: it is bound at execution.

**Rules.** I-JSON (UTF-8, no duplicate names); unknown fields and unknown `op` values rejected (doc 4: a V1 reader
rejects unknown kinds); limits of doc 4 (1 MiB, depth 64, 4,096 nodes) checked by Krummi, with the parser's own
recursion bound as a backstop (`serde_json`'s limit is 128 levels, from knowledge; `System.Text.Json` defaults to 64);
the five validation phases of doc 8 unchanged, errors located by JSON Pointer. Krummi writes documents in one
deterministic form (fixed field order, shortest numbers), without adopting JCS.

**Cache key.** A hash of the normalized typed tree (doc 4's normalization: flattened `And`, sorted operands, `Not`
pushed down), plus the declared parameter types, schema version and index set version. Two documents that differ only
in spacing, field order or operand order share a plan.

**C interface** (sketch for doc 6; names illustrative):

```c
krummi_status krummi_prepare(krummi_store *s, krummi_slice doc_utf8, krummi_stmt **out);   /* query or plan */
krummi_status krummi_bind_u64(krummi_stmt *st, uint32_t param, uint64_t v);                /* one per class */
krummi_status krummi_bind_bytes(krummi_stmt *st, uint32_t param, krummi_slice v);          /* bytes, vectors: copied */
krummi_status krummi_bind_null(krummi_stmt *st, uint32_t param);
krummi_status krummi_stmt_query(krummi_stmt *st, krummi_read_tx *t, krummi_cursor **rows);
krummi_status krummi_read_query(krummi_read_tx *t, krummi_slice doc_utf8, krummi_cursor **rows);  /* one shot */
```

Bind functions are not a second format: they are the C signature of the type table above, as `sqlite3_bind_*` is.

**Libraries.** Rust: `serde` derive and `serde_json` with `float_roundtrip`, if question 6 admits them; otherwise a
strict hand-written reader in safe Rust (estimate: the same order of code as the binary reader, using `str::parse` for
floats). C#: `System.Text.Json` (`Utf8JsonWriter` in the builder; source generation keeps AOT), no new package.

**When to choose CBOR instead.** If B1 and B2 below show, with prepared statements in place, that one-shot decode
dominates workloads users care about, the same document model maps onto CBOR with tags (deterministic encoding of
§4.2, `System.Formats.Cbor` on .NET). The typed model above transfers unchanged; tools would see CBOR through its
diagnostic notation.

## Risks

| Risk | Cost | Mitigation |
|---|---|---|
| Silent float rounding | A query vector or float bound differs by an ulp; distance filters no longer bit-identical across clients (doc 5) | `float_roundtrip`; vectors in Base64 or bound by pointer; test B4 |
| Writers emit `u64` as a JSON number | Values above 2^53 change silently in some tools | The format rejects numbers for 64-bit types; the error names the path |
| One-shot calls pay 1-9 µs of decode | Point queries in loops slow down | Prepared statements; measure (B1) before optimizing |
| Two contracts by stealth (a JSON dump next to a binary, or numbers accepted in two forms) | Drift | One format; each type has one representation (vector arrays only if D5 says so) |
| `serde_json` brings `unsafe` | Against the spirit of D12 | Annotation per the README rule, or a safe reader (question 6) |
| The format becomes a public contract | Changing it breaks stored documents and tools | `"krummi": "query/1"` versioning; before 1.0 no promise (question 3) |
| JSON Schema and validator drift | Tools accept what Krummi rejects | Schema generated or tested against golden documents in CI |

## Claims of documents 4, 6 and 8 found wrong or inconsistent

1. Doc 4, serialization table: MessagePack and CBOR "decode to a generic value tree (allocations), then validate
   shapes by hand". **Partly wrong**: serde-based decoders (`rmp-serde`, `ciborium`, and `serde_json` here) decode
   straight into typed structures; only internally tagged enums buffer per node (measured above for JSON).
2. Doc 4, wire format: "Shape key = hash of `table + nodes` bytes", while its notes say Rust normalizes on receipt and
   "the normalized tree is what is hashed". **Inconsistent**: input bytes are canonical only if the decoder rejects
   every non-canonical form (non-minimal varints, unsorted operands). The consequence is cache misses, not wrong
   results; hashing the normalized tree fixes it in any format.
3. Doc 4: one pass validates "column ids and types against the table schema". **Partly right**: doc 8's phases 3 and 4
   derive types and properties bottom-up over the whole tree, so the single pass covers structure only.
4. Doc 6 returns an invalid tree as `INVALID_ARGUMENT` "with the byte offset"; doc 8 returns `{ code, node index,
   expected, found }`. **Inconsistent**; a node path serves both formats.
5. Doc 4 lists the wire format as still open (question 4); the README's Plans entry records "the query tree's binary
   envelope" as approved. **Inconsistent**; question 7 settles both.

Confirmed while checking: protobuf has no canonical serialization (protobuf.dev), as doc 4 said from knowledge.

## Product questions

1. **Editable plans.** Should applications and AI tools be able to take the plan Krummi returns (planner output or
   `explain`), modify it and submit it back? DD9 currently says the readable dump cannot be read back.
2. **Prepared statements.** Should the first public API (Rust, C, C#) offer prepare once, bind values, execute many,
   besides the one-shot call? Doc 4 proposed no `Prepare` in the first C surface.
3. **Stored documents across versions.** What may an application expect of a stored query or plan when Krummi is
   upgraded: nothing before 1.0, and after 1.0 the same promise as the store format (D7), or another?
4. **Published schema.** Is a versioned JSON Schema of queries and plans part of the product, documented for
   application and tool authors?
5. **A tool that runs documents.** Should Krummi ship a way to run and explain a document against a store (a command
   of the module 12 tool, or an MCP server), or is that left to applications?
6. **`serde_json` and `unsafe`.** Should `serde` and `serde_json`, which contain `unsafe`, be admitted under the
   dependency rule with an annotation, or should Krummi use a hand-written JSON reader in safe Rust?
7. **The format itself.** Do you approve replacing the binary envelope of docs 4 and 8 (and the README's Plans entry)
   by the typed JSON of this document, keeping Krummi's own operators and the Substrait vocabulary?

Design details left for the PRD of module 17 (not product): D1 internally versus externally tagged nodes (1.4-1.6x decode
cost measured); D2 required types versus inference; D3 copying versus borrowing bound vectors; D4 expanded years for
`date` and the text form of `duration`; D5 whether `f32` vectors may also come as number arrays.

## Proposed microbenchmarks

| Id | Question | Method |
|---|---|---|
| B1 | Per-call cost of prepared versus one-shot, from C# | Point lookup on a 1M-row table through the real C interface: one-shot JSON, prepared plus bind; bench power plan, Windows and Linux |
| B2 | Decode plus validation cost with the final library | Corpus: point lookup, five-predicate query, 50-node plan; `"op"` versus outer tags; borrowed versus owned names |
| B3 | Client serialization in C# | `Utf8JsonWriter` versus a hand-written binary writer, same trees, BenchmarkDotNet |
| B4 | Float exactness | Every finite `f32` (2^32 patterns) and 10^8 random `f64`: shortest decimal from Rust and .NET, parsed by `serde_json` with and without `float_roundtrip`, bit equality |
| B5 | Vector parameters | 768 and 4,096 dimensions: number array, Base64, bound pointer, end to end through the C interface |
| B6 | Planning against decoding | Once the planner exists: plan time of the B2 corpus, cache hit and miss |

## Sources

Repository (paths under `C:\projects\Krummi`): `docs/design/tables/README.md`, `04-query-tree-and-planner.md`,
`06-c-interface-and-csharp.md`, `08-execution-plans.md` (read whole); `01-row-format-and-types.md`,
`03-keys-and-indexes.md`, `05-vectors.md`, `verification.md` (skimmed); `docs/prd/krummi/tables.prd.md` (N4, annotations to
other PRDs); `docs/prd/krummi/krummi.prd.md` (D11, D12, out of scope); `docs/benchmark/2026-11-09-windows-margin.md`
(`lecturas-al-azar`); the workspace `Cargo.toml` files (no `serde` today).

Measurement (scratch, outside the repository, kept): `C:\Users\jesus.lopez\AppData\Local\Temp\tmp.1C1GeWR9zE\qbench`
(`src/main.rs`; `run2.txt` the pinned run with `"op"` tags; `run-ext.txt` with outer tags; `serde_json` 1.0.154,
`serde` 1.0.229, `base64` 0.22.1).

External, fetched on 2026-11-10:

- JSON: RFC 8259, §4 (duplicate names "unpredictable"), §6 (interoperable integers, no NaN or Infinity), §9 (parser
  limits): https://www.rfc-editor.org/rfc/rfc8259 ; I-JSON, RFC 7493 (UTF-8, no duplicate names, large numbers as
  strings, base64url for binary): https://www.rfc-editor.org/rfc/rfc7493 ; JCS, RFC 8785 (Informational; ECMAScript
  numbers; keys sorted by UTF-16 code units): https://www.rfc-editor.org/rfc/rfc8785
- CBOR, RFC 8949 (major types, tags 1-4, preferred serialization and float sizes, deterministic encoding §4.2.1,
  security §10 on nesting): https://www.rfc-editor.org/rfc/rfc8949 . Not verified here: the UUID tag (37) and the
  typed-array tags of RFC 8746.
- MessagePack specification (types, `ext` 0-127, timestamp -1, no canonical form, "smallest number of bytes" is a
  SHOULD): https://github.com/msgpack/msgpack/blob/master/spec.md
- Protocol Buffers, "Proto Serialization Is Not Canonical": https://protobuf.dev/programming-guides/serialization-not-canonical/ ;
  `prost-build` 0.14.4 needs `protoc`: https://docs.rs/prost-build/latest/prost_build/
- `serde_json` 1.0.154 manifest (features `float_roundtrip`, `arbitrary_precision`, `unbounded_depth`):
  https://raw.githubusercontent.com/serde-rs/json/master/Cargo.toml ; `unsafe` in
  https://raw.githubusercontent.com/serde-rs/json/master/src/read.rs ; throughput on large files (2021, i7-6600U,
  context only): https://github.com/serde-rs/json-benchmark
- `ciborium` 0.2.2: https://docs.rs/ciborium/latest/ciborium/
- .NET: `JsonSerializerOptions.MaxDepth` (default 64):
  https://learn.microsoft.com/en-us/dotnet/api/system.text.json.jsonserializeroptions.maxdepth ; `JsonNumberHandling`
  (named float literals and quoted numbers are opt-in and "not defined by the JSON specification"):
  https://learn.microsoft.com/en-us/dotnet/api/system.text.json.serialization.jsonnumberhandling ;
  https://learn.microsoft.com/en-us/dotnet/standard/serialization/system-text-json/invalid-json ;
  `System.Formats.Cbor` `CborConformanceMode` (Lax, Strict, Canonical, Ctap2Canonical):
  https://learn.microsoft.com/en-us/dotnet/api/system.formats.cbor.cborconformancemode
- JSON Schema 2020-12 validation (`format` an annotation by default; `uuid`, `date`, `date-time`, `duration`; no
  numeric precision bounds): https://json-schema.org/draft/2020-12/json-schema-validation
- Model Context Protocol, tools (`inputSchema` and `outputSchema` are JSON Schema; JSON-RPC):
  https://modelcontextprotocol.io/specification/2025-06-18/server/tools

From knowledge, not fetched here: SQLite's `prepare`/`bind` API; `System.Text.Json` in the shared framework and its
source generator; `serde_json`'s 128-level recursion limit and its rejection of lone surrogates; Rust's correctly
rounded `str::parse` for floats; MessagePack-CSharp and `rmp-serde`; whether `serde`, `rmp-serde` or `ciborium` contain
`unsafe`; protobuf's JSON mapping of 64-bit integers to strings.
