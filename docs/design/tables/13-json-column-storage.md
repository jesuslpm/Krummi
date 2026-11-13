# 13 - JSON column storage

Technical design, document 13 of the [tables design set](README.md). Written 2026-11-11, after the user asked how
Krummi should store JSON documents in table columns: "investigues, compares y decidas **cómo debería almacenar Krummi
documentos JSON en columnas de sus tablas**", keeping "corrección, simplicidad, rendimiento medido, API ergonómica y
extensibilidad sin complejidad prematura", with the priorities "1. Corrección y durabilidad. 2. Simplicidad de
implementación. 3. Rendimiento. 4. Ergonomía. 5. Extensibilidad." and the warning "No quiero una solución sofisticada
por el mero hecho de ser sofisticada."

Status: every product question was decided with the user on 2026-11-11 (*Product questions*): normalized text,
duplicate names rejected, member order kept, numbers kept as written, depth 128, the navigation index left to module
17's measurement and recorded there as pending, an own reader with layered tests, no update by path in 7.5; indexes
over JSON go to [document 14](14-json-indexes.md). Nothing here is implemented or built. The microbenchmarks of
*Experiments* were run in a scratch project outside the repository and say nothing about the engine's performance.
The PRDs and the other design documents are not changed; PRD 7.5 takes these decisions and annotates the 7.5 entry of
the tables PRD. *Updated 2026-11-12:* the tables PRD's annotation of 7.5 of that day records these decisions; the
rows of *Semantics first* that no product question covered (root scalars, empty input, the grammar, surrogates,
U+0000 and noncharacters, column `NULL` against JSON `null`, defaults) were proposed to the user then and adopted:
"Sí adóptalas."

Evidence labels: **[V]** verified in a primary source (fetched 2026-11-11, or read in a local source tree, with the
path); **[K]** Krummi's own documents or code; **[M]** measured here; **[I]** inference or estimate of this document;
**[L]** from knowledge, not fetched.

## Summary

1. **Decision: store a JSON column value as JSON text in one normalized form, validated at write, behind a one-byte
   header; reserve in that header an optional navigation index, and build the index only when module 17's JSON path
   filters exist and a measurement shows it pays.** The normalized form drops whitespace, writes every string with the
   minimal escapes, keeps the order of object members and the exact text of every number, and rejects duplicate member
   names. The document is the text; the index, when present, is derived data that can always be rebuilt from it.
2. **Why text.** Reading the whole document (the access an application makes most) is a zero-copy `&str` that every
   JSON library reads directly: `serde_json` in Rust, `Utf8JsonReader` in C# without a UTF-16 conversion, the CLI and
   AI agents. Numbers are exact by construction, because their text is kept. The stored bytes are standard JSON that
   the verification and rescue tool (module 12) can show without Krummi's code. Text is what the write must parse and
   validate anyway, so the write path adds a normalized writer and nothing else.
3. **Why not a binary format now.** A binary (PostgreSQL `jsonb`, MySQL's binary JSON, RavenDB's blittable, Parquet
   Variant) makes property access O(log n) per level and scalars free to read, which matters for filters over many
   rows. Its cost is a second representation that is authoritative: an encoder bug loses data, every whole-document
   read becomes a serialization (a copy), numbers need a model of their own, and the format must be versioned and
   migrated forever. Within 7.5 nothing filters rows, so none of its benefit is used before module 17. The reserved
   index gives the same access (binary search over sorted member offsets) on top of the text, when it is needed,
   without changing the stored document.
4. **What the measurements say** (microbenchmarks outside the engine, *Experiments*): a PostgreSQL-style binary was 1.27
   times the text; turning it back into text cost 12 µs for a 4 KB document; converting text into it cost 1.4 to 2.8
   times what the prototype's normalizer takes. On text, a member behind 64 KB of other values took 95 to 160 µs to
   reach by scanning (2.5 ms behind 1 MB), and 0.11 to 0.19 µs through a prototype of the index, against 0.84 µs in the
   binary; indexing only containers over 256 bytes made the index 2.5 to 4.8 % of the text. A safe normalizer of
   Krummi's own did the write-side work about 1.5 times faster than `serde_json` with `serde-transcode` (as fast on the
   smallest document), which does not even check duplicates.
5. **Second-best: a binary of Krummi's own** shaped like Parquet Variant and blittable (sorted member ids, offsets of
   1, 2 or 4 bytes, typed scalars), stored instead of the text. It wins if measurements in module 17 show that, with
   the index, decoding scalars from text still dominates path filters, or that the binary is substantially smaller.
   *When the recommendation would change* gives the thresholds.
6. **Existing binary formats** (CBOR, MessagePack, BSON) are sequential: no offsets, so finding a member is a linear
   walk, and none gives exact decimal numbers without extensions. Used as storage they need an index of their own,
   which makes them a format of Krummi's own in practice. Rejected as storage; nothing prevents them as interchange.
7. **A decomposed document** (one tree entry per node or path) turns one document into hundreds of tree entries,
   multiplies keys, page changes and journal bytes on insert, and needs a range scan and reassembly to read it.
   Its one gain, updating one small property of a large document cheaply, does not justify that. Rejected; shredding
   hot paths into typed columns stays possible later as expression indexes or computed columns.
8. **Semantics proposed** (product decisions, *Product questions*): a JSON value is any RFC 8259 value, scalars at the
   root included; column `NULL` and JSON `null` are different; duplicate member names are rejected; member order is
   kept; whitespace and escape spelling are not; numbers are kept as written and compared by exact value; at most 128
   levels of nesting; the size is the row's (256 MiB); JSON is not orderable, never a primary key or index key column.
9. **Row format: unchanged.** A JSON column is a variable column like `string`, with a new type code in the schema. No
   new row layout, no flag in the row header. The one-byte header belongs to the value, so the stored form of each
   value can evolve without a schema version or a rewrite.
10. **Paths** are typed lists of steps (member name or array index), the only form of a path in query trees, plans and
   index definitions; JSON Pointer (RFC 6901) navigates a concrete document and names locations in errors, and is
   never turned into a query path. No JSONPath (RFC 9535): it is a text query language.

## What is already decided, and what this document may change

| Source | What it fixes | Effect here |
|---|---|---|
| Tables PRD, annotation of 2026-11-10 (`tables.prd.md:107-118`) [K] | Child PRD 7.5, *Columnas JSON*, after 7.3 and before module 17, "para que las consultas puedan filtrar por rutas del JSON desde el principio"; "un tipo de columna JSON guardado en un formato binario que se lee sin copiar ni analizar el texto, al estilo del blittable de RavenDB o del `jsonb` de PostgreSQL"; 7.5 fixes the format, numbers, duplicate keys, member order, equality, limits, error classes and the parser dependency; JSON is never a primary key or index column; indexes over JSON paths come after 7.2 and module 17 | The recommendation keeps the product intent (properties read without parsing the whole document, zero-copy) but not the letter (a binary format). Changing it is product question 1 |
| Tables PRD, *Salida* and root D11 annotation (`tables.prd.md:166`, `krummi.prd.md:116-118`) [K] | JSON values are returned "sin copiarlos" | The normalized text is returned as a borrowed `&str` |
| Root D12; doc 11 P11 [K] | No `unsafe` outside the pager; `serde_json` admitted in the query crate only | The table crate needs its own JSON reader or an annotated dependency (product question 7) |
| Doc 1, row format v1 [K] | Variable columns in an end-offset table, null bitmap, compression hook in the row header | Reused as it is (*Integration with the row format*) |
| Doc 2 [K] | A row is one tree value; over 4,074 bytes minus its key it lives off its leaf in a contiguous run, rewritten whole on update | A large JSON document makes the whole row off-leaf; true of every representation |
| PRD 7.1, E10 [K] | Update changes only the given columns; an update is identical when the row it would give reads the same | A JSON column is replaced whole; "reads the same" is byte equality of the normalized form |
| Doc 4 [K] | V1 queries have no functions over columns | Module 17 needs one family of them: JSON path extraction (*Consequences for 7.1, 7.2, 7.3 and 17*) |
| Doc 9 [K] | JSON as the interchange format of query trees and plans | A different problem: that JSON is parsed once into typed structures and never stored; nothing is shared but the parser policy |

## Requirements

- J1. **Correctness and durability.** What is stored is what the application meant: no silent loss of a member, a
  digit or a character; a value written is read back the same, across restarts and format versions.
- J2. **Zero-copy reads** (D11): reading a JSON value returns a reference into engine memory, valid while the
  transaction is open.
- J3. **Property access without parsing the whole document** (7.5 annotation), at least once module 17 filters rows by
  JSON paths.
- J4. **Safe Rust** (D12) and bounded resources: no stack overflow, quadratic time or unbounded allocation on hostile
  input (huge, deep, many members, long strings).
- J5. **No new row format** unless necessary (the user's rule: "No propongas cambios del formato persistente sin
  explicar por qué son necesarios").
- J6. **Extensible without migrations**: an index, a binary or compression added later must not force rewriting
  existing rows.
- J7. **Ergonomic in Rust, C and C#**, and readable by CLI tools and AI agents.

## Semantics first

The physical choice depends on these guarantees, so they are fixed first. Each row says what PostgreSQL `jsonb`
(PG), SQLite (SL), MySQL (MY), RavenDB blittable (RB) and DuckDB (DK) do, and what this document proposes.

| Question | Others [V] | Proposal | Why |
|---|---|---|---|
| Column `NULL` against JSON `null` | Distinct in PG, SL, MY, Snowflake ("SQL NULL is distinct from VARIANT null") | Distinct. `NULL` is no value (one bitmap bit, 0 bytes); `null` is a stored document of 4 text bytes | Two different facts: "unknown" and "the document says null" |
| Root scalars | Allowed by RFC 8259 ("A JSON text is a serialized value") and I-JSON | Allowed: `42`, `"x"`, `true`, `null` are documents | The standard; nothing gains from refusing them |
| Empty input | Not JSON | Invalid value | Not a JSON text |
| Duplicate member names | RFC 8259: "SHOULD be unique", else behaviour "unpredictable"; I-JSON: MUST be unique; PG `jsonb`, MY, RB keep the last; PG `json`, SL, DK keep all; MongoDB forbids them | **Rejected** with invalid value, naming the JSON Pointer of the object and the name | Keeping the last loses data silently (J1). Strictness is reversible: accepting duplicates later is a compatible change; refusing them after storing some is not |
| Member order | PG `jsonb` and MY sort (MY: "subject to change"); SL, DK and PG `json` keep it; RB keeps it ("GetPropertiesByInsertionOrder") | **Kept** as written | Least surprise for round trips, diffs, tests and AI tools; costs nothing in text, and the index sorts its own entries |
| Whitespace | PG `json` and DK keep it; the rest drop it | **Dropped** | Space; a single stored form. An application that needs the exact bytes stores them in a `string` column |
| Escapes | — | **Normalized**: raw UTF-8 except `"`, `\` and U+0000-U+001F, which use `\"`, `\\`, `\b \f \n \r \t` or `\u00xx` | One spelling per string, so equality of representation is byte equality and most strings are zero-copy `&str` |
| Unicode | I-JSON: UTF-8, no surrogate or noncharacter code points | Valid UTF-8; an escaped surrogate pair becomes its character; a lone surrogate is invalid; noncharacters and U+0000 are accepted | The domain of Krummi's `string` (PRD 7.1, E7: "cualquier texto UTF-8 válido, incluido el carácter U+0000"); a lone surrogate has no UTF-8 form |
| Numbers | PG: `numeric` (prints `1.230e-5` as `0.00001230`), no NaN or infinities; MY, RB: integers as integers, others as double (RB keeps the double's text, `LazyNumber`); SL JSONB keeps the text (`INT5`, `FLOAT5`); I-JSON: integers outside ±(2^53−1) are not interoperable | **Kept as written** (validated RFC 8259 grammar); read on request as `i64` or `u64`, exact or refused, as `decimal(p, s)`, rounded to its scale as E8 rounds and refused only beyond its precision, or as `f64`, correctly rounded and refused only outside its finite range; compared by exact mathematical value | No loss ever (J1); 64-bit identifiers and money survive; the lexeme is the cheapest exact model |
| Equality | PG: structural, numbers by value; DK: textual (`{"a":1,"b":2}` and `{"b":2,"a":1}` differ) | Two notions. **Representational**: the normalized bytes are equal (used by E10's identical update). **Structural**: same kinds, objects with the same names and equal values in any order, arrays element by element, numbers by exact value, strings by bytes (offered when module 17 compares documents) | The first is free and exact; the second is what a query means by `=` |
| Ordering | PG defines a cross-type order (`Object > Array > Boolean > Number > String > null`) | **None.** JSON is not a primary key or index column (7.5 annotation); ordering applies to typed values extracted from paths | No consumer needs a total order of documents; extracted scalars use Krummi's typed order (N5) |
| Depth | PG: stack only; SL: 1,000; MongoDB: 100; .NET `JsonDocument`: 64 by default; `serde_json`: 128 | **128** levels of containers, a ceiling | Matches the query documents' ceiling (doc 11, §2.5); recursion stays bounded. The C# binding must raise .NET's default of 64 (product question 5) |
| Size | PG: 1 GB per value; MY: `max_allowed_packet`; MongoDB: 16 MiB; Snowflake: 128 MB | The row's maximum, 256 MiB (B6, E18), minus the other columns | One rule, already enforced |
| Version of the representation | — | The first byte of every stored value | J6, below |

Two consequences matter for the rest. With numbers kept as written and duplicates refused, **the normalized text loses
nothing that a JSON value means**: whitespace and escape spelling are its only differences from the input. And with
member order kept, normalized text is a function of the document as written (its members in order, its number texts),
so byte equality is exact equality of representation.

**Normalized, not canonical.** A canonical form would give the same bytes to every document with the same meaning.
This one does not: `{"a":1,"b":2}` and `{"b":2,"a":1}`, or `{"n":1.0}` and `{"n":1}`, are stored differently. Making
it canonical would mean sorting members (losing their order) and rewriting numbers exactly (`1.50` as `1.5`, keeping
exponents so that `1e1000000` does not become a million digits), a number design of its own. RFC 8785 (JCS) [V] is
canonical at a higher price: it sorts members by UTF-16 code units and prints numbers through an IEEE double, which
loses digits. Equality by meaning (*Equality*, structural) is therefore computed, never a byte comparison; the two
places that need bytes do not need a canonical form: E10's identical update compares rows as E11 reads them, and the
terms of a future inverted index are computed per path and value, with numbers normalized when the term is built.

**Normalized, not as received.** Storing the input bytes would keep pretty-printing (55 % more for M, X1) and the
escapes of the writer: `System.Text.Json` "escapes all non-ASCII characters" by default (`жарко` becomes
`жарко`), and Python's `json.dumps` escapes them with `ensure_ascii=True`, its default [V].
Such a string is three times larger for Cyrillic and needs decoding, a copy, on every read; and a name written
`"a"` must match `"a"`, so lookups would decode names. Normalized text makes escape-free strings zero-copy `&str`
and name comparison a byte comparison, whatever the application's serializer does. What it gives up, the exact input
bytes (needed, for example, to check a signature over a webhook payload), belongs in a `string` or `bytes` column.

## Options compared

### A. JSON text, UTF-8

- **A1, exact text.** The input bytes, validated. Whole-document reads are zero-copy; nothing else is. Whitespace of
  pretty-printed input is stored and paid in every page and journal write [M: *Experiments*]; escapes vary, so a
  string read may need decoding; duplicate members must still be checked. No advantage over A2 except byte fidelity,
  which a `string` column gives better.
- **A2, normalized text.** The form of *Semantics first*. Same zero-copy whole read, smaller, one spelling per value.
  Property access is a scan of the text up to the member: no validation and no allocation (the text is known valid),
  but O(bytes before the member), plus skipping nested values.

### B. A binary of Krummi's own

The family of PG `jsonb`, MY binary JSON, RB blittable and Parquet Variant [V]:

| System | Object layout | Numbers | Notes |
|---|---|---|---|
| PG `jsonb` (`jsonb.h`) | 4-byte header (count and flags), one 4-byte `JEntry` per key and value (3 type bits, 28 bits of length or offset); "the keys all appear first, in key sort order; then the values" | `numeric` | Stores an offset every `JB_OFFSET_STRIDE` (32) entries and lengths elsewhere, because "an array of offsets ... is basically uncompressible" by TOAST's LZ (commit of 2014-09-29, 9.4 beta; stored betas could not be upgraded) |
| MY (`json_binary.h`) | `element-count size key-entry* value-entry* key* value*`, 16-bit or 32-bit offsets (small and large format), small scalars inlined in the entry | int, uint, double | Partial in-place update when the new value is not larger and adds no member |
| RB blittable (`C:\projects\ravendb\src\Sparrow\Json`) | Values in document order; per object, metadata sorted by property name with a backward offset, a property id and a type byte, offsets of 1, 2 or 4 bytes per object (`BlittableWriter.cs:200-223`); property names written once at the end of the document (`:290-343`); binary search by name (`BlittableJsonReaderObject.cs:785-830`) | Integers as varints; doubles as their text, read lazily (`BlittableWriter.cs:127-131`, `LazyNumber`) | Duplicates: the last one is kept (`CachedProperties.cs:353-375`); strings over 128 bytes optionally LZ4-compressed (`BlittableWriter.cs:762-765`) |
| Parquet Variant | A metadata blob (dictionary of names, sorted flag) and a value blob; objects list field ids and `num_elements + 1` offsets "in lexicographical order of the corresponding field names"; values in any order | int8-64, float, double, decimal4/8/16 (precision ≤ 38), date, timestamp, uuid | "Field names are required to be unique"; shredding moves hot fields to typed columns |
| SL JSONB | A header byte per element (type in 4 bits, size of the size in 4), payload; **no offset table** | Text kept (`INT5`, `FLOAT5`) | "5% and 10%" smaller and "less than half the number of CPU cycles" than text, by SQLite's account; "not intended as an external format" |

What B gives: member lookup by binary search, array element by offset, scalars without lexing, strings stored
unescaped (always zero-copy), and with a name dictionary (RB, Variant) repeated names stored once per document.

What B costs:

- **An authoritative second format.** Encoder, reader, a text writer for every whole-document read, a validator of
  stored bytes (a damaged page must give a damage error, never a panic), and their tests and fuzzing. Each change of
  the format is a format version (B12) with old values kept readable or migrated. PG had to refuse upgrading `jsonb`
  stored by its own betas when it changed the layout [V].
- **Whole-document reads stop being zero-copy text.** The application gets a view, or pays a serialization into its
  memory. In C#, `System.Text.Json` reads UTF-8 text; a binary needs either a C call that writes text into a buffer
  or a reader of Krummi's format in the binding [I].
- **A number model.** Variant and MY map numbers to binary types and lose the lexeme; RB and SL keep the text for
  non-integers. Keeping exactness means storing text inside the binary anyway.
- **Compression.** Offset tables compress badly (the PG lesson); doc 7's future whole-row LZ4 would compress the text
  of A2 better than the offsets of B [I, to measure when 7.4 is built].

### C. Existing binary formats

| Format | Random access | Numbers | Keys | Verdict |
|---|---|---|---|---|
| CBOR (RFC 8949) [V] | None: sequential items; definite lengths let a reader skip a child, indefinite ones need a scan | Integers to 64 bits, floats, bignums (tags 2, 3), decimal fractions (tag 4) | Any type; duplicates "well-formed, but ... not valid", decoder behaviour open | Needs a profile (deterministic, text keys, no indefinite lengths, no duplicates) and an index: a Krummi format |
| MessagePack [V] | None | Integers to 64 bits, double; no decimal | Any type | Same, and no exact decimals |
| BSON [V] | Linear, each sub-document skippable by its `int32` length | int32, int64, double, decimal128 | C strings: "MUST NOT contain" NUL, so not every JSON name; arrays as documents with keys `"0"`, `"1"` | Cannot hold every JSON name; arrays are objects; 16 MiB cap in MongoDB |
| Ion binary [V] | Sequential; names as symbol ids | Integers, decimals, floats | Symbol tables | A data model larger than JSON |

None meets J3 as published. Adding the missing index and the missing guarantees (exact numbers, unique names, one
spelling) is building option B on a borrowed byte grammar, with the obligation of staying conformant to it. Rejected
as storage.

### D. Hybrid

- **D1, normalized text plus a navigation index.** A2's text, followed in the same value by, for each container
  spanning more than a threshold, its members' offsets into the text (sorted by name for objects, in order for
  arrays) and the offset of each nested container's own entry. Property access is a binary search per level, then a
  short lex of the scalar; the whole document is still the text. The index is derived: the verifier rebuilds it from
  the text and compares; a value with a damaged index loses nothing. The idea is the "semi-index" of Ottaviano and
  Grossi (CIKM 2011) [L] and close to simdjson's tape [L], kept in the value instead of rebuilt per read.
- **D2, text plus a full binary copy.** Both A2 and B in the value: roughly twice the bytes, two encoders, no benefit
  over D1 unless scalar decoding dominates, which is B's case. Rejected.

### E. A decomposed document

One tree entry per node, keyed by `primary key ‖ path` (or auxiliary tables per path). Against MVCC and copy-on-write
[I, from doc 2's write model]:

- An insert of a document with n nodes is n tree inserts: n keys of 20 to 40 bytes, more leaves changed, more journal
  bytes, against one value written once.
- A whole-document read is a range scan plus reassembly into a buffer: no zero-copy text (J2 fails).
- Its gain: updating one small property of a large document changes one entry, where every other option rewrites the
  whole row (a large row is a run "rewritten whole on update", doc 2). Snapshot isolation itself is not a problem: the
  entries are tree entries under the same transaction.
- Systems that decompose do it for columnar analytics (ClickHouse sub-columns, with `max_dynamic_paths` 1,024 by
  default and slower whole-object reads by its own documentation; Parquet Variant shredding), not as the primary row
  store [V].

Rejected. The case it serves, frequent small updates of very large documents, has a cheaper remedy that doc 1 already
permits: a companion tree for large columns.

### F. Minimal support first

Validated UTF-8 JSON, a binary later when queries and measurements justify it. F is A1 or A2 without a plan for J3.
Taken alone it leaves two risks: values written as plain text before the binary exists must be read forever or
migrated, and every choice of *Semantics first* that F leaves open (duplicates, numbers, order) becomes a breaking
change when the binary arrives. The recommendation is F made safe: A2's semantics decided now, and the
header that lets D1 (or B) arrive without migrating.

### Matrix

Scores are relative within the row: ++ best, + good, 0 neutral, − weak, −− worst. A cell that cites X1 to X4 rests
on those measurements (*Experiments*); the others are judgement.

| Criterion | A1 exact | A2 normalized | B own binary | C CBOR/MsgPack/BSON | D1 text + index | E decomposed | F minimal |
|---|---|---|---|---|---|---|---|
| Implementation and maintenance | ++ | + (normalized writer) | −− (encoder, reader, text writer, stored-bytes validator, number model) | − (profile plus index) | 0 (A2, plus an index later) | −− | ++ |
| Space and metadata | − (whitespace: +55 % on M, X1) | + | − (PG-style `jsonb` 1.27 × text, X1; a name dictionary would lower it, not measured) | − CBOR 1.50 ×, BSON 1.87 ×; + MessagePack 0.84 × (X1) | 0 (text plus the index, X4) | −− | − or + |
| Insert and update CPU | + (validate) | + (about twice a bare validation, X4) | 0 (text → `jsonb`: 1.4 to 2.8 × the prototype normalizer, X2 and X4) | 0 (2 to 3 times more, X2) | 0 (the index adds 80 to 100 % of normalizing, X4) | −− | + |
| Whole-document read | ++ zero-copy | ++ zero-copy | − (serialize: 12 µs for M, X3) | − | ++ | −− | ++ |
| One property, nested path | −− (parse) | − (0.04 to 0.4 µs near the start; 95 to 160 µs behind 64 KB, 2.5 ms behind 1 MB; X4) | ++ (0.12 to 0.84 µs with a copy, X3) | − (linear walk) | ++ (0.11 to 0.19 µs, X4) | + (one lookup per path, but a descent each) | −− |
| Arrays and objects iteration | − | 0 | ++ | 0 | + | − | − |
| Parse and materialization | − | 0 | ++ | 0 | + | −− | − |
| 8 KiB pages and runs | − | 0 | 0 | 0 | 0 (index adds bytes) | −− (many entries) | 0 |
| Write amplification under COW | − | 0 | 0 | 0 | 0 | −− on insert, ++ on small updates | 0 |
| Zero-copy | whole text only | whole text, most strings | scalars and strings, not the text | strings | whole text, most strings | no | whole text |
| Compression (doc 7, LZ4) | + | + | − (offsets, PG lesson) | 0 | 0 (text compresses, index less) | n/a | + |
| Secondary indexes on paths | needs extraction per row at write (equal for all) | same | same | same | same | natural | same |
| Schema versions | ++ | ++ | + | + | ++ | − (path tables) | ++ |
| Evolution of the stored form | + | ++ (header) | − (authoritative format) | − | ++ (index derived) | −− | − (no header) |
| Huge or deep documents | + | + | + | + | + | − | + |
| Rust, C, C# | ++ text everywhere | ++ | − (binding reader or copies) | 0 | ++ | −− | ++ |
| AI tools | ++ | ++ | 0 (convert to text) | 0 | ++ | − | ++ |

The extremes are quickly settled: C and E lose on simplicity and on whole reads with no compensating gain, A1 and F
are dominated by A2 with a header. The real choice is **A2 → D1 against B**: whether the document's authoritative
form is text with a derived index, or a binary.

## Access to properties and future operations

| Operation | A2 (7.5) | D1 (later) | B | What to decide now so it stays easy |
|---|---|---|---|---|
| Get a member by name | Scan the object's members, skipping values | Binary search the object's entry | Binary search | Names compared as normalized bytes (one spelling) |
| Nested path | Repeat per step | One search per step | Same | Paths as typed steps |
| Array element | Skip i elements | Offset i | Offset i | Indexes as `u32` steps; no negative indexes in V1 |
| Member exists | As get | As get | As get | Missing is distinct from `null` |
| Filter by a value in the document | Extract, then compare typed | Same, faster | Same | Extraction rules: missing, `null`, wrong kind |
| Project members | Return borrowed sub-slices of the text (each a valid JSON text) | Same | Copy or view | A member of a normalized document is itself normalized text |
| Update or remove a member | Rebuild: prefix ‖ new value ‖ suffix of the text, with the duplicate check only for a new name; no re-parse of the rest | Same, then rebuild the index | Rebuild or splice | The row is rewritten whole under COW in every option (doc 2); only CPU differs |
| Index on a path or expression | Extract at write (7.2's machinery, after module 17) | Same | Same | Typed extraction into Krummi types, so entries use doc 3's tuple encoding |

**Paths.** A path is a list of typed steps, each a member name (UTF-8 string) or an array index (`u32`). It is the
**only** form of a path in query trees, plans and index definitions (docs 4 and 8): there it is that list, never a
string to parse, and in their JSON form (document 9) an array of steps where a string is a member name and an integer an
index (`["items", 0, "sku"]`). JSON Pointer (RFC 6901) [V] is used only where a document exists: to navigate one
(`doc.pointer("/items/0/sku")` in Rust and C#, resolved token by token against the containers it meets, as RFC 6901
says), and to name a location in one, in errors ("duplicate member at `/customer`") and in tool output. It is never
turned into a query path, because without a document its tokens are ambiguous: `/items/0` names member `"0"` of an
object or element 0 of an array. Corrected 2026-11-11 (review RJ5): the first version also offered Pointers in the CLI
and said a Pointer "resolves against the document", which does not exist when a query is built. The `-` token of RFC
6901 (after the last element) is refused when navigating, since it never names a value. JSONPath (RFC 9535, Proposed
Standard 2024) [V] adds wildcards, filters and descendants: a text query language, which the root PRD keeps out of
scope.

**Extraction rules** (to fix in module 17, recommended now so 7.5's accessors agree): extracting a path gives a JSON
value, *missing*, or a typed scalar on request. Typed extraction (`as i64`, `as f64`, `as string`, `as decimal(p, s)`,
`as bool`) of missing, `null` or the wrong kind gives SQL `NULL`, so a filter is unknown for that row (doc 8's
three-valued logic). An integer type gives `NULL` for a number it cannot hold exactly. A `decimal(p, s)` rounds the
number to its scale as E8 of PRD 7.1 rounds a `decimal` given to Krummi, the tie away from zero (`1.235` reads as
`1.24` in a `decimal(10, 2)`), and gives `NULL` only when the rounded value has more than `p` digits (`99.995` in a
`decimal(4, 2)`). `f64` gives the correctly rounded nearest value, and `NULL` only outside its finite range
(`1e400`). So `0.1` and `0.10000000000000001`, two different texts, both read as the same `f64` (corrected
2026-11-11, Codex review R1, which found the two rules stated as one). The `decimal` rounding was decided the same
day; the first rule refused a number with more digits than the scale. User: "Redondear no es inventar, yo
redondearía.\"
An integer type (`i64`, `u64`) reads a number whose exact value is an integer in its range, in any notation: `100`,
`100.0`, `1e2` and `1.00e2` all read as 100; `100.6` gives `NULL`, never rounded, because a fraction in an integer
field (an identifier, a count) is a wrong value rather than one to round; so does a value out of range. Decided
2026-11-11; the first rule read only integer text (`1`, not `1.0` or `1e0`), as PostgreSQL's `'1.0'::int` refuses
[L]. User, between rounding, any notation of an exact integer and integer text only: "valor entero exacto".

**Dates and instants** (requirement for module 17, decided 2026-11-11). JSON has no date type: a date is a string.
Asked how Krummi would answer "the orders created on a given day" when the date lives only in the document, the
answer showed that the typed extraction above has no temporal type; the user asked to add it ("Las dos cosas").

- `JsonAs(date, ...)` reads a string of exactly the form `YYYY-MM-DD`. `JsonAs(timestamp, ...)` reads an RFC 3339
  date-time with an offset: `YYYY-MM-DDTHH:MM:SS`, an optional fraction of any number of digits, and `Z` or
  `+HH:MM`/`-HH:MM`, upper-case `T` and `Z`. An instant with an offset is unambiguous, so it is converted to UTC; the
  fraction is rounded to the microsecond as E8 of PRD 7.1 rounds instants given to Krummi (to the nearest, the tie
  to the greater), and the domain is checked after rounding, as E8's annotation of 2026-11-11 says.
- Anything else gives `NULL`, as typed extraction does for the wrong kind: a date-time without an offset
  (`2019-07-26T00:00:00`, a local time whose instant is unknown), another layout (`11/11/2026`, a space instead of
  `T`), a value outside the domains of E8 (`date` from 0001-01-01 to 9999-12-31, `timestamp` from
  0001-01-01T00:00:00Z to 9999-12-31T23:59:59.999999Z, also when rounding takes it out), a date that does not exist
  (`2026-02-30`), a leap second, or a value that is not a string (a number of seconds or milliseconds since 1970 has
  no unit in the document; the application reads it with `JsonAs(i64, ...)`).
- Why this form (corrected 2026-11-11 before any review; the first version accepted only `Z` and at most 6 fractional
  digits): `System.Text.Json`, the C# binding's serializer, writes up to seven fractional digits without trailing
  zeros (`DateTime.UtcNow` as `"2026-11-11T10:23:00.1234567Z"`), a `DateTimeOffset` in UTC as `+00:00`, and a
  `DateTime` of unspecified kind without an offset [V]; the first rule turned most dates written from C# into `NULL`.
  User, to accepting offsets and rounding: "Sí". The query parameters of document 11 keep their stricter single form
  (exactly six digits and `Z`): Krummi or its binding writes them, so they need no tolerance.
- The values are Krummi's `date` and `timestamp`, so comparisons, ranges, order and expression indexes over them use
  the typed order of N5. "A given day" is then the range `[day 00:00Z, next day 00:00Z)`; Krummi stores instants in
  UTC without a time zone (PRD 7.1), so the application turns its local day into that range.
- Why not compare the text: a prefix `"2026-11-11"` on `JsonAs(string, ...)` equals "that day in UTC" only while every
  document writes its dates in the same form in UTC; `"2026-11-11T00:30:00+01:00"`, an instant of 10 November in UTC,
  has the prefix of the 11th, and `"11/11/2026"` misses it, both without an error. With `JsonAs(timestamp, ...)`, the
  first is read as the instant it is, and the second gives `NULL`, which the application can find and fix.
- `time` and `duration` are not extracted from JSON in this requirement; they can be added the same way if needed.

**Quantifiers over arrays** (requirement for module 17, decided 2026-11-11). The user: "Una cosa que me gustaría hacer
es poder hacer filtros complejos dento del json en una consulta, no sólo por un path. Por ejemplo imagina que tienes
una columna JSON con un pedido en cada fila, con su cabecera y con sus líneas de pedido y quieres obtener los pedidos
que tenga una cantidad mayor que C del producto P." To recording it as a requirement for module 17's PRD: "sí".

- **The trap.** `lineas[*].producto = P AND lineas[*].cantidad > C` is true for an order whose line `{"producto":"P",
  "cantidad":1}` meets the first condition and whose line `{"producto":"Q","cantidad":100}` meets the second. The
  request needs **one element** that meets both. MongoDB has `$elemMatch` for it, which "matches documents that
  contain an array field with at least one element that matches all the specified query criteria"; PostgreSQL has
  jsonpath filters, `$.lineas[*] ? (@.producto == $p && @.cantidad > $c)`, where `@` "denotes the value being
  considered" [V].
- **The nodes.** `JsonAny { column, steps, element }` is true when some element of the array at `steps` makes the
  predicate `element` true; `JsonAll` when every element does (and, as in logic, for an empty array). Inside
  `element`, paths start at the element (`@`), and every predicate of the query tree is allowed: comparisons, `AND`,
  `OR`, `NOT`, typed extraction, and another `JsonAny` or `JsonAll` for arrays inside elements. Values are
  parameters, as everywhere in the query tree (document 4).

  ```text
  JsonAny { column: pedido, steps: [lineas],
            element: And( Eq( JsonAs(string, @, [producto]), $p ), Gt( JsonAs(i64, @, [cantidad]), $c ) ) }
  ```

- **Three-valued logic.** An element whose predicate is unknown (a line without `cantidad`) does not count for
  `JsonAny`; for `JsonAll`, unknown makes the result unknown unless some element is false, as SQL's `AND` over the
  elements.
- **Strict paths.** A step that expects an array or an object and finds something else gives *missing*; no automatic
  wrapping or unwrapping of arrays, unlike PostgreSQL's default *lax* mode, which "implicitly adapts the queried data
  to the specified path" [V].
- **Execution without an index**: navigate to the array, evaluate the element predicate on each element, stop at the
  first that decides. Cost grows with the array's bytes; document 13's navigation index reaches the array, not its
  elements' members.
- **Indexes**: [document 14](14-json-indexes.md), the multi-entry index over several members of the same element,
  which answers this example exactly.
- **Later**: counting elements that satisfy a predicate (`count(lineas where ...) > 2`) and other aggregates over
  arrays.

## Performance and storage

Where the time goes, per access pattern. Costs come from *Experiments* [M] where measured, and from doc 2's write
model otherwise [I].

| # | Pattern | Dominant cost | Representation matters? |
|---|---|---|---|
| 1 | Insert small documents (~200 B) | Commit: journal, page copies, checksums (more than half of write time in W2 and W21, doc 2) | Little: normalizing S takes 0.8 µs (X4), about one in-order insert of the bench and under 1 % of a durable single write |
| 2 | Insert documents of a few KiB | Commit plus parsing; a 4 KiB row fills half a leaf | Parsing is the CPU part for every option (M: 11.6 µs, X4, against 8.5 µs per random insert of a 100-byte row); B's encoding costs about the same (X2) |
| 3 | Documents larger than a page | The off-leaf run: whole pages, all journaled | No: bytes written are the same order in every option |
| 4 | Read the whole document | Page access (zero-copy) for A2/D1; serialization for B | Yes, against B |
| 5 | One simple property | A2: scan to it; B, D1: a few comparisons | Yes, for large documents and many rows [M] |
| 6 | A deeply nested path | A2: scan per level; B, D1: one search per level | Yes |
| 7 | Change one small property of a large document | The row rewrite under COW (a new run for a large row) dominates; CPU differences are secondary | No: COW rewrites everything; only E or a companion tree avoids it |
| 8 | Find rows by an indexed property | The index (7.2 after module 17): descent plus fetch | No: extraction happens at write |
| 9 | Scan many rows without reading their JSON | Rows are wider, so fewer per leaf, and over 4,074 bytes the whole row is off-leaf | No: the size matters, not the format; the remedy is a companion tree for large columns (doc 1) |

The rule that follows: **parsing dominates only when documents are read by path, in memory, over many rows (5, 6
and filters in module 17)**; elsewhere storage, page reads or copy-on-write dominate, and the representation changes
little. That is exactly the case D1 serves, and exactly the case that does not exist before module 17.

## Experiments

Microbenchmarks on one machine (Intel i9-13980HX, Windows 11, rustc 1.99.0, release with `codegen-units = 1` and thin
LTO), single thread, warm cache, median of 41 batches of about 5 ms. **They measure libraries on bytes in memory, not
Krummi**: no pages, no transactions, no journal. Scratch project, outside the repository and kept:
`C:\Users\jesus.lopez\AppData\Local\Temp\claude\abbc23f7a7515b82\632caf70-8da8-4ba3-915c-cf49780988da\scratchpad\jsonbench`
(`run1.txt`, `run2.txt`; documents in `docs\`, generated from a fixed xorshift seed). Reproduce:
`CARGO_TARGET_DIR="$PWD/target" cargo build --release && ./target/release/jsonbench docs`. Crates: `serde_json`
1.0.154, `serde-transcode` 1.1.1, `sonic-rs` 0.5.10, `simd-json` 0.14.3, `ciborium` 0.2.2, `rmp-serde` 1.3.1, `bson`
2.15.0, `jsonb` 0.5.6 (Databend's PostgreSQL-like binary). Two full runs agreed within 10 % except Databend `jsonb`
path lookups (12 to 16 % slower in run 2) and XL's text-to-`jsonb` conversion (30 %); run 1 is shown.

Documents: **S**, a flat object of 172 bytes; **M**, an order of 3,900 bytes (`id`, `status`, `created`, `total`, a
nested `customer.address.geo`, 22 `lines`); **M pretty**, M indented (6,051 bytes); **L**, 65,402 bytes (300 lines);
**XL**, 1,048,526 bytes; **D100**, 100 nested objects. The root members come in the order `id`, `status`, `created`,
`total`, `customer`, `lines`, so every root member but `lines` precedes the large array; X4 adds documents with a
member after it.

### X1. Sizes (bytes) [M]

| Doc | Text | CBOR | MessagePack | BSON | Databend `jsonb` |
|---|---:|---:|---:|---:|---:|
| S | 172 | 232 | 141 | 283 | 195 |
| M | 3,900 | 5,843 | 3,265 | 7,277 | 4,961 |
| L | 65,402 | 97,988 | 54,638 | 122,361 | 83,240 |
| XL | 1,048,526 | 1,569,942 | 875,802 | 1,967,206 | 1,334,201 |

The PostgreSQL-style binary is **1.27 times the text** on M, L and XL; MessagePack, which has no offsets, is 0.84
times; whitespace adds 55 % to M (M pretty). A binary is not smaller by nature: offsets and type bytes cost about what
quotes, commas and colons do. A name dictionary (blittable, Variant) would remove repeated names, which this corpus
has in every `lines` element; that was not measured.

### X2. Write side: validation and conversion (ns per document) [M]

| Doc | memcpy | UTF-8 only | Validate (`IgnoredAny`) | Validate + duplicates (untuned visitor) | Minify (`serde-transcode`) | Text → `jsonb` | Parse to `serde_json::Value` |
|---|---:|---:|---:|---:|---:|---:|---:|
| S | 6 | 21 | 226 | 953 | 1,061 | 1,084 | 2,822 |
| M | 34 | 147 | 5,503 | 25,340 | 24,545 | 24,755 | 69,171 |
| L | 2,402 | 2,689 | 99,617 | 431,618 | 389,409 | 510,986 | 1,445,933 |
| XL | 41,009 | 41,224 | 1,547,067 | 6,748,000 | 7,216,700 | 9,976,200 | 20,163,100 |

**Correction of 2026-11-11 (found during the review of *Review history*).** Databend's `jsonb` crate enables
`serde_json`'s `arbitrary_precision` and `preserve_order` features by default (`cargo tree -e features -i serde_json`
in the scratch project), and Cargo unified them for the whole build: every `serde_json` column above ran with them.
It is a live case of the unification risk of *The parser*. The `serde_json` columns were measured again in a separate
project with default features (`scratchpad\serdeplain`, `serde_json` 1.0.154, `serde-transcode` 1.1.1; two runs,
within 10 %; run 1 shown):

| Doc | Validate (`IgnoredAny`) | Minify (`serde-transcode`) | Parse to `serde_json::Value` |
|---|---:|---:|---:|
| S | 199 | 754 | 1,580 |
| M | 5,236 | 17,689 | 51,522 |
| L | 92,898 | 306,406 | 839,640 |
| XL | 1,530,700 | 5,346,500 | 14,427,200 |

Validation barely changes; minify and `Value` are 25 to 40 % faster without the features. The text → `jsonb` column
keeps its features, since that crate needs them. Throughput with default features: validation 690 to 860 MB/s; minify
about 200 MB/s; a full `Value` 73 to 110 MB/s. Converting text into Databend's binary costs 1.4 (S), 2.1 (M) to 2.8 (XL)
times the prototype normalizer of X4; a binary encoder of Krummi's own was not measured, so the write side separates A2
from B as measured, not as it could be built. For scale, the bench on the same machine
([2026-11-09-windows-margin.md](../../benchmark/2026-11-09-windows-margin.md), values of about 100 bytes) gives Krummi
990,853 inserts per second in key order (about 1.0 µs each), 117,244 at random (8.5 µs), 8,531 durable single writes
(117 µs) and 1,045,630 random reads (0.96 µs) [K]. Validating M (5.5 µs) is then several in-order inserts of small
values, less than one random insert, and 5 % of a durable single write; a 4 KiB row also fills half a leaf, so its page
copies and journal bytes grow with it whatever the format [I].

### X3. Read side (ns per document) [M]

| Doc | Whole text (memcpy) | `jsonb` → text | `id` (text, lazy `sonic-rs` `get`) | `id` (`jsonb` path) | Nested path (text, lazy) | Nested path (`jsonb`) | Full `serde_json::Value` |
|---|---:|---:|---:|---:|---:|---:|---:|
| S | 6 | 435 | 114 | 124 | 137 | 126 | 2,822 |
| M | 34 | 12,379 | 112 | 124 | 376 (`customer/address/city`) | 151 | 69,171 |
| L | 2,402 | 208,426 | 114 | 122 | 73,500 (`lines/250/sku`) | 835 | 1,445,933 |
| XL | 41,009 | 3,774,500 | 120 | 122 | 73,184 (`lines/250/sku`) | 840 | 20,163,100 |
| D100 | 9 | 7,566 | — | — | 2,333 (100 levels) | 1,607 | 32,314 |

Readings:

- **Reading the whole document as text** costs a copy at most with text storage, and a serialization with a binary:
  12 µs for M, 3.8 ms for XL. This is the price B pays on the most common access.
- **A member near the start** costs the same in text and binary (about 0.1 µs): the lazy scan stops early.
- **A member behind a large value** is where text loses: 73 µs to reach `lines[250]` in L, against 0.84 µs in the
  binary, 87 times, and 76 random reads of a row by key in the bench (0.96 µs each). The scan's cost is the bytes it
  must skip, not the document's size.
- `sonic-rs` scans with SIMD and `unsafe`; a safe scanner in Krummi would be slower [I], which X4 measures.
- Databend's path lookup returns a copy of the sub-value (`OwnedJsonb`), so its numbers include an allocation.

### X4. Prototype of the normalized text and of the index (D1) [M]

A prototype of this document's design, in the same scratch project (`src/bin/proto.rs`, `proto_run1.txt`,
`proto_run2.txt`; reproduce with `cargo build --release --bin proto && ./target/release/proto`), in safe Rust with
`#![forbid(unsafe_code)]` and only the standard library: a one-pass normalizer with the rules of *Stored form v1*
(about 315 lines), a scanner over normalized text (about 110 lines), and the index of *Index (D1)* with its lookup
(about 200 lines). Before timing, it checks that every normalized document parses (with `serde_json`) to the same value
as its input, that normalizing is idempotent, that M pretty gives M's exact 3,900 bytes, about 65 hand cases
(duplicates also after decoding, lone and paired surrogates, `\/`, `-0`, `1.50e+3`, `01`, `1.`, raw tabs, depth 129),
and that scan and index lookups give the same byte range as `serde_json`'s `pointer`, for present and missing paths.

**Normalizing** (validation, duplicate check and normalized writing, including the output allocation):

| Doc | Bytes | ns | MB/s | Against `IgnoredAny` validation (X2, default features) | Against `serde-transcode` minify (X2, default features) |
|---|---:|---:|---:|---:|---:|
| S | 172 | 786 | 219 | 3.9 × | 1.04 × |
| M | 3,900 | 11,638 | 335 | 2.2 × | 0.66 × |
| M pretty | 6,051 | 14,052 | 431 | — | — |
| L | 65,402 | 198,412 | 330 | 2.1 × | 0.65 × |
| XL | 1,048,526 | 3,609,100 | 291 | 2.4 × | 0.68 × |
| D100 | 602 | 3,339 | 180 | — | — |

An untuned reader of Krummi's own does the whole write-side job about 1.5 times faster than `serde_json` plus
`serde-transcode` with default features (as fast on S), although `serde-transcode` does not even check duplicates,
and in about twice a bare validation. (The first version of this table compared against the feature-unified figures
and said "half the time"; corrected with the default-feature measurement.)

**Index size** (percent of the normalized text) by the span a container needs to get an entry:

| Doc | All containers | Span > 64 bytes | Span > 256 bytes |
|---|---:|---:|---:|
| S | 34 % | 34 % | 0.6 % (no entry; scans) |
| M | 40 % | 31 % | 3.4 % |
| L | 40 % | 30 % | 2.5 % |
| XL (4-byte offsets) | 75 % | 58 % | 4.8 % |
| D100 | 166 % | 150 % | 97 % |

**Lookups** (ns; index with the 64-byte threshold):

| Doc | Path | Scan | Index |
|---|---|---:|---:|
| S | `id` | 40 | 77 |
| M | `id` | 40 | 125 |
| M | `customer/address/city` | 358 | 236 |
| M | `lines/10/sku` | 4,243 | 173 |
| L | `customer/address/city` | 347 | 249 |
| L | `lines/250/sku` | 94,692 | 173 |
| XL | `lines/250/sku` | 97,007 | 192 |
| D100 | 100 × `a` | 1,411 | 2,908 |

Readings:

- **The index reaches the binary's lookup cost**: 173 to 192 ns for `lines/250/sku`, against 835 to 840 ns for
  Databend's `jsonb` (which also copies the result, X3) and 95 µs for the safe scan, 500 times more.
- **The index costs where the scan is already cheap**: a first member is found in 40 ns by scanning and 125 ns through
  the index, and deep nests of small objects (D100) are twice as slow, because each binary-search step must find the
  end of a name. Both are small in absolute terms; a reader may scan a container whose span is small.
- **The threshold decides the space.** Indexing every container costs 30 to 75 % of the text, which would erase the
  space advantage over the binary (X1: 27 %). Indexing only containers that span more than 256 bytes costs 2.5 to 4.8
  %, and those are the containers where scanning is expensive.
- **The safe scanner is slow on containers** (about 0.6 GB/s, a byte-by-byte loop); a table-driven classifier would
  help. It matters only without an index.
- Design corrections the prototype found, applied in *Index (D1)*: the root's entry must be addressed by an offset
  (writing children before their parent then needs no fix-up pass), and the offset width must cover the index as well
  as the text, since child offsets point into the index, which can exceed 64 KiB for a text that does not.

**Members after the large array, and the 256-byte threshold** (`f1.txt`, `f2.txt`). L tail and XL tail are L and XL
with `"zz":1` as their last root member. ns per lookup:

| Doc | Path | Scan | Index, > 64 bytes | Index, > 256 bytes |
|---|---|---:|---:|---:|
| S | `tag` | 351 | 145 | 369 (S is under 256 bytes: it scans) |
| M | `customer/address/city` | 345 | 246 | 277 |
| M | `lines/10/sku` | 4,237 | 170 | 135 |
| L | `lines/250/sku` | 101,031 | 179 | 127 |
| XL | `lines/250/sku` | 107,250 | 193 | 142 |
| L tail | `zz` | 159,746 | 107 | 107 |
| L tail | `lines/393/sku` (last) | 167,966 | 168 | 140 |
| XL tail | `zz` | 2,516,400 | 120 | 119 |
| XL tail | `lines/6335/sku` (last) | 2,410,150 | 224 | 152 |

- **Without an index, a member at the end costs a full pass of the document**: 160 µs for 64 KB and 2.5 ms for 1 MB,
  about 1.5 times one validation of the same text (X2). That is the case D1 exists for.
- **With the index at 256 bytes**, every measured path into or past the large array costs 107 to 152 ns, whatever the
  document's size, for an index of 2.5 % (L) and 4.8 % (XL) of the text. Shallow paths into small objects are about
  15 % slower than at 64 bytes, because they are scanned. 256 bytes is therefore the first value proposed; module 17's
  measurement fixes it.
- The 2-byte width was never exceeded in the corpus; the prototype rebuilds with 4 bytes if an offset does not fit,
  and the width byte records it.

The run-to-run differences above 10 % were all in `memcpy`, in D100's index build and in cells under 200 ns (M's
`id`, `total` and `lines/10/sku` moved 10 to 20 %); every scan-against-index gap above is two to four orders of
magnitude larger.

### Depth

| Library | 100 levels | 1,000 levels |
|---|---|---|
| `serde_json` into `Value`, a visitor, `serde-transcode` | ok | Error: recursion limit (128) |
| `serde_json` `IgnoredAny` | ok | ok (no depth counter) |
| `ciborium` decode | ok | Error: recursion limit (about 256) |
| `sonic-rs`, `simd-json`, `rmp-serde`, `bson`, Databend `jsonb` | ok | ok |

A limit must be Krummi's rule, not an accident of the library: libraries differ, and a document accepted at write
must be readable by every Krummi reader and by the C# binding.

### Proposed microbenchmarks (with the engine, for PRD 7.5 and module 17)

| Id | Question | Method |
|---|---|---|
| JB1 | Write cost of a JSON column in the engine | Insert 1,000,000 rows with S, M and L documents in writes of 1,000, like W23; time, pages written, journal bytes; against the same rows with the text in a `string` column (the floor) |
| JB2 | Read cost | Read by primary key and return the text; read and extract `id`; read and extract a late member; S, M, L |
| JB3 | Path filter (decides D1) | Full scan of 1,000,000 M rows filtering on an early and a late member, A2 against D1, against a scan that reads a `string` column of the same size |
| JB4 | Update one member | Change `status` in M and L documents; time and journal bytes; shows that COW dominates |
| JB5 | Scan without JSON | Scan a small column of tables whose rows carry S, M and L documents; rows per leaf and off-leaf runs read |
| JB6 | Compression (7.4) | LZ4 with and without a dictionary on normalized text, with and without the index, and on a binary |
| JB7 | Voron comparison | JB1 and JB2 against Voron's tables with blittable values (N11) |

## Integration with the row format

- **Column class.** JSON is a variable column, in doc 1's variable section, located by the end-offset table like
  `string` and `bytes`. Access to the value stays O(1); a null costs one bitmap bit and 0 bytes. No change to row
  format v1.
- **Type code.** The schema's column type gains `json`. Rows never store types (doc 1), so nothing else changes.
  Because 7.1's catalog treats an unknown type as damage, a store with a JSON column cannot be opened by a build
  without 7.5: 7.5 is an incompatible change for B12 (the next format version), unless 7.5 ships in the same format
  version as 7.1.
- **Value header** (the only new persistent detail, and why it is needed): one byte at the start of every non-null JSON
  value. Bits 0-3: the stored form, `1` = normalized text v1; bit 4: an index follows the text; bits 5-7: zero
  (reserved). Without an index the text is `value[1..]`; with one, a `u32` text length follows the header, then the
  text, then the index. It is needed for J6: an index, a binary form or a later text revision then applies per value,
  old values stay readable as they are, and no schema version or rewrite is involved. The alternative, the form in the
  schema, would tie a storage optimization to the product-visible schema version (PRD 7.1, E2 and E4) and force all
  values of a column to change together. Cost: 1 byte per value.
- **Defaults.** A JSON column may have a constant default like any column (PRD 7.1, E4), stored in its normalized
  form; E5's "esquema demasiado grande" rule covers large defaults.
- **Schema evolution.** Adding a nullable JSON column, or one with a default, is free as for every type (N3). Changing
  a `string` column to `json` is a type change, refused by N3.
- **Public conversion.** In: text (`&str` or `&[u8]`), validated and normalized before anything changes, failing
  with invalid value (N10) with the JSON Pointer and byte offset of the first fault. Out: a borrowed view (below).
- **Compression.** Doc 7's whole-row LZ4 compresses the row, JSON text included; nothing in this design depends on it,
  and the header is inside the compressed body like every column.

Rust surface, a sketch for PRD 7.5 (names are 7.5's to fix):

```rust
let doc: Json<'txn> = row.json("payload")?;      // None if the column is NULL
let text: &'txn str = doc.as_str();              // normalized text, zero-copy
let city = doc.pointer("/customer/address/city"); // Option<Json<'txn>>
match city.map(|c| c.kind()) { Some(JsonKind::String) => { /* c.as_text_value(): Cow<'txn, str> */ } _ => {} }
let qty: Option<i64> = doc.get("qty").and_then(|v| v.as_i64());
for (name, value) in doc.members() { /* in stored order */ }
```

C (module 18): the value as a pointer and length of UTF-8 text, valid until the transaction ends, like `string`.
C#: a `ReadOnlySpan<byte>` that `Utf8JsonReader` and `JsonDocument.Parse` read without converting to UTF-16 [L],
plus the copying twin of doc 6. Navigation through C functions is not needed in V1: C# libraries navigate the text.

## AI-assisted development

What an agent needs is **readable JSON at the interface**: the CLI and an MCP server (doc 10) print documents, accept
them, and report errors by JSON Pointer. That holds for any stored representation, since a tool can always convert;
it is not a reason to store text, and this document does not count it as one (the reasons are J1, J2 and simplicity).
What the stored form does change is the work of the tools: with text, `krummi` prints the value it reads, and the
verification and rescue of module 12 can show a document from a damaged store without decoding a format.

Capabilities worth adding to doc 10's layer, none of which depends on the format:

- **Define**: a `json` column type in table definitions.
- **Insert and import**: JSON lines (module 19) map one document per line; errors give the line, the byte offset and
  the Pointer.
- **Inspect**: show a document; `profile`: sample rows (as statistics do) and report each path with its kinds, fill
  rate and value examples, the input an agent needs to write queries and propose indexes.
- **Recommend indexes**: from the profile and a query's explain, suggest an expression index over a path with its
  typed extraction (after module 17).
- **Validate**: check a file of documents against the column's rules before loading, with the same errors as the
  engine.

## Product and design decisions, separated

**Product** (the user decides; PRD 7.5 records):

- What a JSON column is: any JSON value, its semantics (*Semantics first*), its limits (128 levels, the row's size).
- Public operations in 7.5: write from text; read the whole text; navigate (members, elements, Pointer); read typed
  scalars; build a document without text (a builder) if wanted. Updating a member by path inside the engine:
  postponed (the application reads, changes and writes; COW rewrites the row anyway).
- In module 17: path extraction, existence and typed comparisons in filters and projections. After 7.2 and 17:
  expression indexes over paths.
- Error classes: invalid value for invalid text or rules; size not valid for a document over the row maximum.

**Design** (this document; PRD 7.5 may record it as starting state):

- Stored form: normalized text v1 with a value header; the index reserved and built when measured.
- Validation and normalization in one pass, iterative or with recursion bounded by the depth limit.
- The parser: an own strict reader in safe Rust, or `serde_json` (question 7).
- Navigation by scanning the normalized text (7.5), by the index (later).
- Thresholds: when a container gets index entries, when the index is worth storing.

## Recommended design

### Stored form v1

```text
value  = header text                         header bit 4 = 0 (7.5)
       | header text_len:u32le text index    header bit 4 = 1 (reserved; built later)
header = form:4 (1 = normalized text v1) | has_index:1 | reserved:3 (0)
text   = normalized JSON text (below)
```

Normalized text:

1. One RFC 8259 value, UTF-8, no byte order mark, no whitespace outside strings.
2. `true`, `false`, `null` as such.
3. Numbers: the input's characters, which must match RFC 8259's grammar (no `+`, no leading zeros, no `NaN` or
   `Infinity`, no hexadecimal, no trailing `.`); unbounded in digits and exponent within the size limit.
4. Strings and member names: escapes decoded, then written with raw UTF-8 except `"` → `\"`, `\` → `\\`, U+0008
   `\b`, U+000C `\f`, U+000A `\n`, U+000D `\r`, U+0009 `\t`, other U+0000-U+001F as `\u00xx` (lower-case hex). An
   escaped surrogate pair is written as its character; a lone surrogate is an error; `\/` becomes `/`.
5. Objects: members in input order; two members whose names are equal after decoding are an error.
6. At most 128 nested containers.

### Index (D1), designed now, built when measured

Sketched to show the header reserves enough; sizes and thresholds are measured before building it.

```text
index      = width:u8 (2 or 4) root:width (offset of the root's entry, 0 if none) entries
entry      = for an object: count:u32, then count triples (name_offset, value_offset, child), sorted by the name's normalized bytes
             for an array : count:u32, then count pairs (value_offset, child), in order
name_offset, value_offset = positions in the text; child = position of the nested container's entry in the index, or 0
width      = 2 when both the text and the index are under 64 KiB, else 4
```

A container gets an entry when its text spans more than a threshold; containers below it are scanned. X4 measured 64
and 256 bytes: at 256 the index is 2.5 to 4.8 % of the text and keeps the lookups that matter fast; the value is
fixed with module 17's measurement. Children are written before their parent, so the root's position is stored
rather than implied. Lookup: binary search the entry, then lex the value at `value_offset` (a number's text, a
string's escapes if any). The verifier (module 12) rebuilds the index from the text and compares: a mismatch is
damage of the index, and the text is still the document.

### Invariants

- I1. Every stored JSON value starts with a known header; reserved bits are zero; an unknown form is damage.
- I2. The text of every stored value is normalized: `normalize(text) == text` (an idempotence the tests check on
  every stored value they write).
- I3. Reading never changes a value; `parse(as_str())` gives the value written, with member order and number texts
  kept.
- I4. A failed write changes nothing (B7): validation completes before the row is built.
- I5. Representational equality is byte equality of the text; structural equality is invariant under member order and
  number spelling.
- I6. Depth ≤ 128 in every stored value; the reader enforces it again and reports damage, never overflowing its stack.
- I7. Validation is O(n log n) at worst in the input size: the duplicate check per object is a sort or a hash with a
  keyed hasher; pairwise comparison only below a small constant number of members (8 in the prototype).
- I8. When an index exists, it is a function of the text: rebuilding it gives the same bytes.

### Limits and security

| What | Limit | Kind |
|---|---|---|
| Document bytes | The row's 256 MiB, minus the rest of the row | Ceiling (B6, E18) |
| Nesting | 128 containers | Ceiling |
| Members per object, elements per array | Bounded by the size; duplicate check O(n log n) | — |
| Number text | Bounded by the size; conversions refuse what does not fit | — |
| Validation memory | O(depth) for the stack plus O(members of the largest object) for the duplicate check | Follows from the size |

Rust's `str::parse::<f64>` is correctly rounded [L], so `as_f64` needs no dependency; a number text such as `1e400`
gives no `f64` (out of range) and is still stored exactly.

### The parser

A strict validating normalizer is small: RFC 8259's grammar, UTF-8 checks, escape decoding, the duplicate check
and a writer. Two ways:

- **An own reader in safe Rust in the table crate** (recommended): no dependency and no D12 annotation; one pass that
  validates and writes normalized text; differential tests and fuzzing against `serde_json` as a development dependency.
  The prototype (X4) did it in about 315 lines, about 1.5 times faster than `serde_json` with `serde-transcode`
  (default features), which does less.
- **`serde_json`**, annotated under D12 as P11 did for the query crate. Keeping number texts needs either its
  `arbitrary_precision` feature, which Cargo unifies across the build, so it would also change how the query crate
  parses its documents (a reported case: an upstream crate enabling it broke deserialization elsewhere, serde-rs/json
  issue #845 [V]), or its `raw_value` feature plus a custom visitor. Its duplicate check needs a visitor too.

## Consequences for 7.1, 7.2, 7.3 and 17

- **7.1**: nothing to change. Row format v1 already holds a variable column; the type enumeration of the catalog
  gains a value in 7.5. 7.5 is the format change, not 7.1.
- **7.2**: JSON is never an index key column (7.5 annotation). As an included (covering) column it is bytes in the
  index value like `string`, with doc 3's off-leaf rule; whether to allow it is 7.2's question, as it is for vectors.
- **7.3**: statistics of a JSON column hold its null fraction only; statistics of paths come with expression indexes.
- **7.4**: text is the friendly case for LZ4 with a dictionary (repeated member names across rows).
- **7.5**: the PRD settles the product questions below; its *Fija* list is answered by *Semantics first*.
- **17**: a family of expression nodes, the only functions over columns of V1: `JsonPath(column, steps)`,
  `JsonExists(column, steps)` and typed extraction `JsonAs(type, column, steps)`, usable in filters, projections and
  sorts with the rules of *Access to properties*. The planner gives JSON predicates a default selectivity and
  evaluates them as residual filters until expression indexes exist. **Pending by decision of 2026-11-11 (product
  question 6):** its PRD adds the path filter load JB3 and decides from it whether to build D1, the navigation index
  that 7.5 leaves reserved. **Requirement of 2026-11-11:** its PRD includes the quantifiers `JsonAny` and `JsonAll` of
  *Quantifiers over arrays*, with the multi-entry index over several members of one element from document 14, and
  the temporal extraction `JsonAs(date, ...)` and `JsonAs(timestamp, ...)` of *Dates and instants*.
- **18**: text span in C and C#; the C# binding sets `MaxDepth` to 128 when it parses Krummi's documents.
- **19**: JSON lines import writes the normalized text of each line.

## When the recommendation would change

Measured in module 17's bench on Windows and Linux, on a table of 1,000,000 rows with M-like documents (4 KiB):

| Observation | Then |
|---|---|
| A path filter with A2 costs more than twice a full scan that reads a `string` column of the same size | Build D1 (set bit 4 for new and rewritten values; a background or explicit rebuild for old ones) |
| With D1, lexing scalars and decoding escapes is more than half of the filter's CPU | Consider B as form `2` of the header, values converted on rewrite |
| B's stored size is 20 % or more below A2's on the bench corpus | Consider B for space |
| Applications or C# mostly navigate and rarely read whole documents | Weighs towards B |
| Frequent small updates of documents over 64 KiB dominate a real workload | A companion tree or chunking for large columns (doc 1), not a new JSON format |

## Risks

| Risk | Effect | Mitigation |
|---|---|---|
| Path filters before D1 are slow on large documents | Module 17's first JSON filters scan text | D1 is designed and reserved; its trigger is a measurement, not a redesign |
| The normalized writer differs from the validator | Stored text not normalized, equality wrong | I2 checked on every stored value in tests; fuzzing with round trips |
| Lone surrogates, overlong UTF-8, `\u0000` handled wrongly | Invalid or lossy text | Test vectors per rule; differential tests against `serde_json` |
| Quadratic duplicate check | Hostile objects freeze a writer | I7, a test with 10^6 members |
| C# default depth 64 below Krummi's 128 | The binding fails on valid documents | The binding sets `MaxDepth`; a test with depth 128 |
| Rejecting duplicates surprises users of PG or MySQL | Imports fail | The error names the Pointer; product question 2 |
| Large documents make rows off-leaf | Scans of other columns read runs | Doc 1's companion tree remedy, shared with vectors |

## Tests and acceptance criteria for PRD 7.5

- Round trip: for a corpus with every value kind, unicode, escapes, numbers of every form (`-0`, `1E400`,
  `123456789012345678901234567890`, `0.1`), deep and wide documents, `parse(stored) == input` as values, member order
  and number texts kept, and `stored == normalize(stored)`.
- Rejections, each with invalid value, the Pointer and the offset, and the row and transaction unchanged (B7): empty
  input, trailing data, every grammar fault, invalid UTF-8, a lone surrogate escape, a duplicate member (also equal
  only after decoding: `"a"` and `"\u0061"`), depth 129, a document over the row maximum (size not valid).
- Column `NULL` against JSON `null`: written and read as different things; a missing member against a `null` member.
- Accessors: `as_i64`, `as_u64`, `as_f64`, `as_decimal` at their limits and one past them; `i64::MIN`, `u64::MAX`.
- Zero-copy: `as_str` and escape-free strings point into the page (a test hook as in 7.1).
- **The reader, "muy bien probado"** (product question 7), with these layers, every one in CI on Windows and Linux
  unless it says otherwise:
  - *Conformance corpus*: Nicolas Seriot's JSONTestSuite (`y_` files must be accepted, `n_` rejected; each `i_`
    file, implementation-defined, gets a decided outcome pinned by a test), plus a corpus of Krummi's own rules. A
    `y_` file that breaks a stricter rule of Krummi is a pinned exception, listed by name with the rule and the
    expected error: today `y_object_duplicated_key.json` (`{"a":"b","a":"c"}`) and
    `y_object_duplicated_key_and_value.json` (`{"a":"b","a":"b"}`), both refused as duplicate members; an unlisted
    difference fails the test (corrected 2026-11-11, Codex review R2: the first version required accepting every
    `y_` file).
  - *Reference model* (the oracle for values; corrected 2026-11-11, review RJ3): a test-only tree `Null | Bool |
    Number(text) | String | Array | Object(members in order)` whose numbers are their exact text, built directly by
    the property-test generator, so it never goes through a JSON parser. What Krummi's navigator reads back from a
    stored document must equal the tree the generator wrote; numbers compare by their text, never through `f64`.
  - *Differential*: every corpus and generated document through the reader and through `serde_json` with its
    **default features** (a development dependency; `arbitrary_precision` or `preserve_order` would be unified by Cargo
    into the query crate's `serde_json`, as X2's correction shows happening), comparing accept or reject; values are
    compared through `serde_json` only for documents whose numbers it represents exactly (integers within `i64` and
    `u64`, and decimals the generator writes as the shortest text of an `f64`), and through the reference model
    otherwise. The only allowed differences are listed in the test (duplicates, depth and numbers `serde_json` cannot
    hold), so a new difference fails. The first version compared values through `serde_json::Value`, which with
    default features rejects `1e400` ("number out of range"), reads `123456789012345678901234567890` as
    `1.2345678901234568e29`, and finds `0.1` equal to `0.10000000000000000000001`, two documents Krummi keeps apart
    (measured, `scratchpad\serdeplain`, `src/bin/oracle.rs`).
  - *Numbers beyond `f64` and `u64`*: `1e400`, `-1e-400`, `123456789012345678901234567890`, `18446744073709551616`,
    `0.10000000000000000000001` next to `0.1`, `1.0` next to `1`: stored and read back with their exact text, distinct
    from each other, and refused or `NULL` by the typed accessors that cannot hold them.
  - *Exhaustive small inputs*: every string up to a fixed length over the number alphabet (`0-9 . e E + -`) against
    the RFC 8259 number grammar, and every `\uXXXX` escape alone and in pairs (surrogates paired, lone, reversed).
  - *Property tests* with the in-house generator the table tests already use (`crates/krummi/tests/table_encoding.rs`):
    random documents of every kind, re-serialized with random whitespace and random legal escapes, must give the same
    normalized text; `normalize` is idempotent; a random byte flipped in a valid document gives an error or another
    valid document, never a panic.
  - *Limits*: depth 128 accepted and 129 refused at every nesting kind; an object with 10^6 members (I7's bound on
    time); the largest document a row admits.
  - *Mutants*: hand-made mutants of the escape table, the duplicate check, the depth counter and the number grammar,
    each killed by some test, as document 3 did for the key encoding.
  - *Fuzzing* of the reader and the navigator over arbitrary bytes (`cargo fuzz` on Linux, a development tool outside
    the library's build): no panic, no stack overflow, only errors.
  - UTF-8 is validated with the standard library (`std::str::from_utf8`), not by Krummi's code.
- E10: an update with a document that differs only in whitespace or escapes is identical.
- Bench (N11): inserting, reading and extracting from S, M and L documents against Voron's tables with blittable
  values, so the comparison includes RavenDB's format.

## Product questions

1. **The stored form.** The 7.5 annotation says "un formato binario ... al estilo del blittable de RavenDB o del
   `jsonb` de PostgreSQL". Recommended: normalized text with a value header, the navigation index reserved and built
   when module 17's measurement asks for it; the binary (B) stays the alternative if the thresholds above are met.
   Alternative: B in 7.5, as annotated. *Decided 2026-11-11: normalized text.* User: "Sí, pero ¿el texto canónico no
   exigiría que dos json con el mismo significado fueran iguales en texto?"; after the answer (the form is normalized,
   not canonical; *Semantics first*) and the comparison with storing the text as received: "Sí seguimos con el texto
   normalizado." The 7.5 annotation's "formato binario" is to be annotated by PRD 7.5.
2. **Duplicate member names.** Recommended: rejected. Alternative: the last one kept, as PG `jsonb`, MySQL and RavenDB.
   *Decided 2026-11-11: rejected* with invalid value, naming the object's JSON Pointer and the repeated name. User: "A
   mi entender claves duplicadas en un JSON implica JSON no válido, lo más estricto sería rechazarlo, pero si lo que
   quieres es mantener la información todo lo posible lo suyo sería { "a": [1, 2] }". Answer: RFC 8259 allows them
   ("SHOULD be unique") and I-JSON forbids them; merging into an array is ambiguous (indistinguishable from a real
   array), makes a member's kind depend on how often it appears, has no clear form when the values are arrays, and
   changes what is read back. To rejecting them: "Sí".
3. **Member order.** Recommended: kept. Alternative: sorted, as PG `jsonb` and MySQL. *Decided 2026-11-11 with question
   1: kept* (the question accepted was "texto normalizado, conservando el orden de las propiedades y los números tal
   como vienen").
4. **Numbers.** Recommended: kept as written, read exactly or refused, compared by value. Alternative: integers as
   `i64` and the rest as `f64` (smaller model, loses digits). *Decided 2026-11-11 with question 1: kept as written.*
5. **Depth.** Recommended: 128. Alternatives: 64 (.NET's default) or 100 (MongoDB). *Decided 2026-11-11: 128.* User:
   "128". The C# binding raises .NET's `MaxDepth` to 128 when it reads Krummi's documents.
6. **The index in 7.5 or later.** Recommended: later, triggered by module 17's measurement. Alternative: build D1 in
   7.5, so that 7.5 already reads properties without scanning. Measured in X4 (separate passes; one combined pass not
   measured): at the 256-byte threshold the index build costs about as much again as normalizing M (11.4 µs
   against 11.6 µs) and 80 % more for L (159 µs against 198 µs); without it, reading the last member of a 1 MB
   document costs 2.5 ms instead of 0.12 µs. *Decided 2026-11-11: later, and recorded as pending work.* User:
   "Después, pero que quede anotado". So 7.5 stores values with header bit 4 clear and reads them by scanning; module
   17's PRD must include the path filter load JB3 and decide D1 from it with the thresholds of *When the
   recommendation would change*; PRD 7.5 records the reserved bit and this pending decision.
7. **The parser.** Recommended: an own strict reader in safe Rust in the table crate. Alternative: `serde_json` in the
   table crate, annotated under D12, with the `arbitrary_precision` risk above. *Decided 2026-11-11: its own reader.*
   User: "El propio, pero muy bien probado." The test layers are in *Tests and acceptance criteria for PRD 7.5*.
8. **Updating a member by path inside the engine.** Recommended: not in 7.5. Alternative: `set` and `remove` by path
   in 7.5, rewriting the row as any update does. *Decided 2026-11-11: not in 7.5.* User: "no en 7.5". The application
   reads, changes and writes the document; adding `set` and `remove` later is a compatible change.
9. **Where indexes over JSON are designed.** Raised in the review of this document (GIN-style indexes, Krummi's
   posting lists, primary keys in them). *Decided 2026-11-11: a document of their own,*
   [14-json-indexes.md](14-json-indexes.md). User: "documento propio".

## Review history

**2026-11-11, external review (ChatGPT, passed on by the user) of this document and document 14, findings J1 to J5,
written RJ1 to RJ5 here because J1 to J7 are this document's requirements.** The reviewer approved the decisions
(normalized text, the deferred navigation index, posting lists of primary keys) and asked to correct five points, to
check each against the documents first, and to close the review. RJ1, RJ2 and RJ4 belong to document 14 and are
recorded in its *Review history*. Here:

| Finding | Verdict | Kind | Fix |
|---|---|---|---|
| RJ3, the test oracle for numbers (medium) | Valid: with default features `serde_json` rejects `1e400`, rounds `123456789012345678901234567890` and equates `0.1` with `0.10000000000000000000001` (measured) | Test defect | *Tests*: a reference model with numbers as text; `serde_json` with default features for accept or reject and for values it represents exactly; cases beyond `f64` and `u64` |
| RJ5, typed paths against JSON Pointer (medium) | Valid: `/items/0` is ambiguous without a document, and the first version offered Pointers where none exists | Precision | *Paths* and the summary: typed steps are the only query path; Pointers navigate documents and name locations |

Found while checking RJ3, and material: Databend's `jsonb` crate turns on `serde_json`'s `arbitrary_precision` and
`preserve_order` by default, Cargo unified them for the whole scratch build, and the `serde_json` figures of X2 had
been measured with them; reproducible with `cargo tree -e features -i serde_json` in the scratch project. Measured
again with default features, the prototype normalizer is about 1.5 times faster than `serde_json` with
`serde-transcode`, not twice, and converting text into Databend's binary costs 1.4 to 2.8 times the normalizer, not
the same. Corrected in the summary, the matrix, X2, X4 and *The parser*; no decision depended on the factor of two
(question 7 was decided for the dependency and the number texts, which hold), and none changes.

## Sources

Repository (paths under `C:\projects\Krummi`): `docs/prd/krummi/tables/tables.prd.md` (N1-N11, the 7.5 annotation at lines
107-118, *Salida*), `docs/prd/krummi/tables/tables-schema.prd.md` (E4, E5 and its annotation of 2026-11-11, E7, E10, E18,
E20), `docs/prd/krummi/krummi.prd.md` (D11, D12), `docs/design/tables/README.md`, documents 1, 2, 4, 7, 9, 10 and 11
(read in the parts cited), document 3 (included columns), `docs/benchmark/2026-11-09-windows-margin.md` (operations
per second on the machine of *Experiments*).

RavenDB (local source, read 2026-11-11): `C:\projects\ravendb\src\Sparrow\Json\BlittableJsonToken.cs` (token types
and offset widths), `BlittableWriter.cs` (lines 127-131 doubles as text, 200-223 object metadata, 290-343 property
names, 762-765 string compression over 128 bytes), `CachedProperties.cs` (77-84 and 353-375, the last duplicate kept),
`BlittableJsonReaderObject.cs` (785-830 binary search, 885 insertion order).

External, fetched 2026-11-11:

- PostgreSQL: JSON types (whitespace, order, duplicates, `numeric`, ordering, GIN classes):
  https://www.postgresql.org/docs/current/datatype-json.html ; `jsonb.h` (`JEntry`, `JB_OFFSET_STRIDE` 32, key order):
  https://raw.githubusercontent.com/postgres/postgres/master/src/include/utils/jsonb.h ; `jsonb_util.c` (last
  duplicate wins, `check_stack_depth`): https://raw.githubusercontent.com/postgres/postgres/master/src/backend/utils/adt/jsonb_util.c ;
  the 2014 compressibility commit: https://postgresql.org/message-id/E1XYdpK-0003Bk-PU%40gemulon.postgresql.org ;
  TOAST (2 kB threshold, 1 GB limit): https://www.postgresql.org/docs/current/storage-toast.html
- SQLite: https://sqlite.org/json1.html (text storage, 1,000 levels, JSON5 input); https://sqlite.org/jsonb.html
  (element header, no index, size and speed claims)
- MySQL: https://dev.mysql.com/doc/refman/8.0/en/json.html (binary storage, last duplicate, sorted keys, partial
  update conditions, generated-column and multi-valued indexes); `json_binary.h`:
  https://raw.githubusercontent.com/mysql/mysql-server/8.0/sql-common/json_binary.h
- CBOR, RFC 8949: https://www.rfc-editor.org/rfc/rfc8949 ; MessagePack: https://github.com/msgpack/msgpack/blob/master/spec.md ;
  BSON: https://bsonspec.org/spec.html ; MongoDB limits (16 MiB, 100 levels, unique names):
  https://www.mongodb.com/docs/manual/reference/limits/ ; Ion binary: https://amazon-ion.github.io/ion-docs/docs/binary.html
- Parquet Variant: https://raw.githubusercontent.com/apache/parquet-format/master/VariantEncoding.md and
  https://raw.githubusercontent.com/apache/parquet-format/master/VariantShredding.md
- Quantifiers over arrays: MongoDB `$elemMatch`:
  https://www.mongodb.com/docs/manual/reference/operator/query/elemMatch/ ; PostgreSQL SQL/JSON path filters (`?`,
  `@`, lax and strict modes): https://www.postgresql.org/docs/current/functions-json.html
- ClickHouse JSON: https://clickhouse.com/docs/sql-reference/data-types/newjson ; Snowflake VARIANT:
  https://docs.snowflake.com/en/sql-reference/data-types-semistructured ; DuckDB JSON (stored as `VARCHAR`, order and
  duplicates kept): https://duckdb.org/docs/2.0/data/json/json_type.html
- RavenDB blittable design (secondary, 2016): https://ayende.com/blog/172707/the-importance-of-a-data-format-part-iii-the-solution
- RFC 8259: https://www.rfc-editor.org/rfc/rfc8259 ; I-JSON, RFC 7493: https://www.rfc-editor.org/rfc/rfc7493 ; JSON
  Pointer, RFC 6901: https://www.rfc-editor.org/rfc/rfc6901 ; JSONPath, RFC 9535: https://www.rfc-editor.org/rfc/rfc9535 ;
  JCS, RFC 8785: https://www.rfc-editor.org/rfc/rfc8785
- Rust: `serde_json` features: https://raw.githubusercontent.com/serde-rs/json/master/Cargo.toml ; its map's sorted
  default: https://docs.rs/serde_json/latest/serde_json/map/struct.Map.html ; `arbitrary_precision` breaking another
  crate: https://github.com/serde-rs/json/issues/845 ; `sonic-rs` (lazy `get`):
  https://raw.githubusercontent.com/cloudwego/sonic-rs/main/README.md ; Databend `jsonb`:
  https://raw.githubusercontent.com/datafuselabs/jsonb/main/README.md ; simdjson README:
  https://raw.githubusercontent.com/simdjson/simdjson/master/README.md
- .NET `JsonDocumentOptions` (`MaxDepth` 64 by default; `AllowDuplicateProperties`, .NET 10):
  https://learn.microsoft.com/en-us/dotnet/api/system.text.json.jsondocumentoptions; `System.Text.Json` escaping
  ("By default, the serializer escapes all non-ASCII characters"; `UnsafeRelaxedJsonEscaping`):
  https://learn.microsoft.com/en-us/dotnet/standard/serialization/system-text-json/character-encoding ; `System.Text.Json`
  dates (up to seven fractional digits, offsets): https://learn.microsoft.com/en-us/dotnet/standard/datetime/system-text-json-support ; Python
  `json` (`ensure_ascii`, default `True`): https://docs.python.org/3/library/json.html

From knowledge, not fetched: Ottaviano and Grossi, "Semi-indexing semi-structured data in tiny space" (CIKM 2011);
simdjson's tape; Rust's correctly rounded float parsing; `Utf8JsonReader` reading UTF-8 without conversion;
PostgreSQL refusing `'1.0'::int`; MySQL's 100-level depth limit (not found in the pages fetched).
