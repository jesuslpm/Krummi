# 14 - Indexes over JSON columns

Technical design, document 14 of the [tables design set](README.md). Written 2026-11-11 from a discussion with the
user during the review of [document 13](13-json-column-storage.md), which stores JSON columns as normalized text.
It began with "¿Podemos tener índices de JSON al estilo de PostgreSQL (creo que se llaman GIN=" and continued with
"Krumi tiene además dentro del alcance otras estructuras como posting lists, mira krummi.prd.md" and "Las posting
lists prodrían guardar las claves primaria en vez de los identificadores de fila." The user chose a document of its
own for it ("documento propio", document 13, product question 9).

Status: architecture approved by the external review of 2026-11-11 (*Review history*), with its invariants (X1 to
X5, and the two rules of *Queries*) to be kept when the indexes are designed in detail. Nothing here is implemented,
built or measured in the engine; the compression figures come from
small simulations outside the repository. No PRD takes these indexes yet; the tables PRD's annotation of 7.5
(2026-11-12) records where they stand and that the multi-valued index over one element is a requirement of module
17. The product questions at the end stay open until
this work is planned, after module 14, when real documents can measure terms per row (decided 2026-11-11; the user, to
leaving them for then: "Sí").

Evidence labels, as in document 13: **[V]** verified in a primary source (fetched 2026-11-11, or read in a local
source tree, with the path); **[K]** Krummi's own documents; **[S]** simulated here; **[I]** inference or estimate;
**[L]** from knowledge, not fetched.

## Summary

1. **Two kinds of index over JSON, for two kinds of question.**
   - An **expression index** indexes one typed value extracted from a path (`customer.city` as a `string`): a
     secondary index of 7.2 whose column is an expression. It answers equality, ranges and order on that path, and can
     be unique. A **multi-valued** variant indexes every element of an array path (`tags[*]`), one entry per element;
     over several members of the same element (`lineas[*]/producto`, `lineas[*]/cantidad`) it answers document 13's
     quantifiers exactly, such as the orders with a line of product P and a quantity over C.
   - An **inverted index** indexes every path and value of the document, in the style of PostgreSQL's GIN: a
     dictionary of terms, each with the list of rows that contain it. It answers containment and existence over paths
     nobody chose in advance, such as an AI agent exploring documents. Its dictionary is ordered by term, so the
     values of one path are contiguous: prefixes, ranges and order over them are possible, strings as the terms are
     designed and numbers once their terms keep numeric order, an open point of the detailed design (*Prefixes and
     ranges over the dictionary*); until then rule 1 of *Queries* does not use them. Corrected 2026-11-11: the first
     version said it answered "neither ranges nor order"; the user: "Esto está mal sí que se pueden los rangos y los
     ódenes de los valores".
2. **The rows of an inverted index are named by their primary key**, as every secondary index of Krummi does
   (document 2). No row identifiers of the index's own: they were considered and dropped in the discussion, because
   the application's schema solves what they solve (*Row identifiers, considered and dropped*).
3. **A term's rows live inline in its dictionary entry while they are few, and in a posting list (root module 14) when
   they are many**, the two-level shape that GIN and Corax share. Krummi's normal trees hold values up to 256 MiB, so
   the small lists need no container (module 13), unlike Corax.
4. **Compression depends on the primary key's type** [S]: about 1 byte per entry for an integer key from a sequence;
   2 to 3 bytes for `ulid` keys inserted fast; 11 to 13 bytes for `ulid` keys inserted slowly, because each new
   millisecond brings 80 random bits that no compressor reduces (PRD 7.1, E15). Hence a schema guideline, not engine
   machinery: tables with large inverted indexes use an integer primary key from a sequence (`i64` is fine).
5. **Terms are exact**: a value term holds the path and the value, with numbers normalized so that `1`, `1.0` and
   `1e0` give the same term (document 13 keeps numbers as written). Only terms too long for a key are hashed. A query
   whose single term can tell the answer apart is answered by the index alone (*Queries* lists which); any other
   rechecks each candidate row against its document.
6. **Order of construction**: expression indexes need 7.2 and module 17's path expressions; inverted indexes need
   module 14, which comes after modules 17 and 18. So expression indexes come first, and they are the cheaper answer
   for known paths.

## Starting state

| Source | What it fixes | Effect here |
|---|---|---|
| Tables PRD, 7.5 annotation (`tables.prd.md:107-118`) [K] | "Indexar rutas dentro de un JSON queda fuera de 7.5 (índices sobre expresiones, después de 7.2 y del módulo 17)"; a JSON column is never a primary key or index column | Indexes over JSON index values extracted from it, never the column itself |
| Root PRD, D4 and the module split (`krummi.prd.md:44`, `:254-257`) [K] | Scope includes "las estructuras añadidas para Corax (containers, posting lists, compact trees y lookups)": modules 13 *Containers*, 14 *Posting lists* ("Listas ordenadas y comprimidas de identificadores"), 15 *Compact trees*, 16 *Lookups*; modules 17 and 18 are built right after module 7 | The inverted index builds on module 14; it cannot come before it |
| Document 3 [K] | Index entries are `tuple(columns) ‖ tuple(primary key)` (FoundationDB tuple encoding, `memcmp` order); expression indexes planned for case-insensitive lookups; partial indexes reserved (`filter`) | Expression indexes reuse this layout as it is |
| Document 2 [K] | Tables are clustered by primary key; no row identifiers | Posting lists hold primary keys |
| Document 13 [K] | Normalized text; numbers kept as written; duplicates refused; depth 128; typed extraction gives `NULL` for missing, `null` or the wrong kind | Extraction and term rules follow it |
| PRD 7.1, E14 and E15 [K] | Sequences give increasing `i64` values, never reused; a `ulid` adds 1 to its random part within a millisecond and draws new random bits at each new millisecond | The compression of posting lists of primary keys (*Compression*) |

## What others do

- **PostgreSQL GIN** [V]: "A GIN index stores a set of (key, posting list) pairs, where a *posting list* is a set of
  row IDs in which the key occurs"; a posting list too large for one index tuple becomes a "posting tree". For `jsonb`,
  `jsonb_ops` "creates independent index items for each key and value in the data" and supports `?`, `?|`, `?&`, `@>`,
  `@?` and `@@`; `jsonb_path_ops` makes "a hash of the value and the key(s) leading to it", is "usually much smaller"
  and more specific, cannot answer key existence, and "produces no index entries for JSON structures not containing
  any values, such as `{"a": {}}`". For jsonpath operators it uses only clauses "of the form `accessors_chain ==
  constant`"; no range operator is indexable. Writes are the weak point: "inserting or updating one heap row can cause
  many inserts into the index", so *fastupdate* queues entries in a pending list that "searches must scan ... in
  addition to searching the regular index". Its own advice: "targeted expression indexes are likely to be smaller and
  faster to search than a simple index".
- **MySQL** [V]: JSON columns "are not indexed directly"; indexes go on generated columns, and multi-valued indexes
  (8.0.17, InnoDB) index each element of an array.
- **Corax on Voron** (local source, `C:\projects\ravendb\src`) [V]: each field has a term dictionary (a compact tree,
  `Corax/Indexing/IndexWriter.cs:717-748`) whose value is a 64-bit reference tagged in its two low bits
  (`Corax/Indexing/TermIdMask.cs`): `Single` (one entry inline), `SmallPostingList` (an encoded list in a container) or
  `PostingList` (a Voron posting list: a tree whose leaves hold sorted `long` values compressed with FastPFor,
  `Voron/Data/PostingLists/PostingListLeafPage.cs:47-112`). The posting list keeps its entry count
  (`PostingListState.NumberOfEntries`), accepts only non-negative values ("Only positive values are allowed",
  `PostingList.cs:101-104`) and reserves bit 0 to mark removals while merging (`PostingList.cs:109-114`, `:715-725`).

## Requirements

- X1. Indexes stay consistent with the table in the same transaction (N7, G1), unless a variant says otherwise.
- X2. An index changes no row and no row format.
- X3. Results are exact: an index may return candidates, but the query returns only rows whose document satisfies the
  condition.
- X4. Bounded work per write, in two parts that are stated separately: *finding* what changed reads the old and the
  new document whole (extracting their terms or values and comparing them), so it is proportional to their sizes;
  *applying* it changes only the entries, terms or list positions that differ. Corrected 2026-11-11 (review RJ4): the
  first version said the whole cost was proportional to the terms that change.
- X5. Reuse what 7.2 and module 14 build; no structure of the index's own beyond them.

## Expression indexes

**Definition.** An index of 7.2 whose key column is a typed extraction, `JsonAs(type, column, steps)` (document 13,
*Access to properties*), possibly next to ordinary columns: `(tenant_id, JsonAs(string, payload, customer/city))`.
The entry is `tuple(value) ‖ tuple(primary key)`, as for any column (document 3); included columns, descending order,
nulls first or last and uniqueness work the same.

**Extraction.** Missing, `null` and the wrong kind give `NULL`, which is indexed like any null (first by default); a
number that an integer type cannot hold exactly gives `NULL` too; `decimal(p, s)` rounds to its scale and `f64` to
the nearest value, as document 13 defines them. A unique expression index therefore lets
many rows lack the path, as nulls are distinct in a unique index (N7). Whether a mismatch should fail the write
instead, as a `CHECK` would, is product question 2.

**Maintenance.** Each write extracts the value from the old and the new document and changes the entry only if it
changed: two extractions per index per write (finding, X4), at most one entry changed (applying). With document 13's
scan, extracting a member near the start of a 4 KB document costs about 0.1 µs, and one at the end of a 1 MB document
2.5 ms [V: document 13, X3 and X4]; the second is the case for document 13's navigation index.

**Too long a value.** An extracted string whose entry passes the 2,048-byte key limit cannot be indexed. 7.2 already
fails such a write with size not valid (N7); the same rule applies (product question 1).

**Multi-valued.** Its definition has the shape of document 13's `JsonAny`: a typed path to an array, then a typed path
relative to each element (empty for the element itself). This document writes it `tags[*]` or `lineas[*]/producto` for
short; `[*]` is not a step of a typed path, which has only member names and array indexes, but the boundary between the
two paths (aligned 2026-11-11 while checking review RJ5). With `JsonAs(string, payload, tags[*])`, the index has one
entry per distinct element: `tuple(element) ‖ tuple(primary key)`. A write adds and removes the difference between the
old and new element sets. It is the right index for arrays of a few values (tags, roles), with ranges per element, and
it is what the discussion found an inverted index would do no better (*Row identifiers*: below about 3 terms per row,
posting lists save nothing).

**Multi-valued over several members of one element** (requirement for module 17, decided 2026-11-11, with document
13's *Quantifiers over arrays*). The index's columns are extractions relative to the elements of one array:
`(JsonAs(string, pedido, lineas[*]/producto), JsonAs(i64, pedido, lineas[*]/cantidad))`, all with the same array
step. It has one entry per element: `tuple(producto, cantidad) ‖ tuple(primary key)`, so the columns of an entry
always come from the same line. The user's example, the orders with a line of product P and a quantity over C, is
then one range: prefix `producto = P`, `cantidad > C`, giving exactly the matching orders, without a recheck; an order
with several qualifying lines appears once per line, so the plan removes repeated primary keys. A write adds and
removes the difference between the old and new sets of element tuples. Rules: every column of such an index uses the
same array step (two different arrays would give their cross product); an element without one of the members
contributes `NULL` for it, as typed extraction does; ordinary columns may lead the index (`tenant_id` first).

## Inverted indexes

### Terms

Every document gives two kinds of term [I, from GIN's `jsonb_ops` and its `jsonb_path_ops` gap]:

| Kind | One per | Bytes | Answers |
|---|---|---|---|
| **Path** | Path of every value present, a member's or an array element's (positions erased), whatever its kind | `P ‖ path` | "Has the path"; candidates for containment of empty structures (`{"a":{}}`), which `jsonb_path_ops` cannot index, with a recheck of the kind |
| **Value** | Scalar at a path | `V ‖ path ‖ value` | "The path holds this value", containment |

- **Path encoding**: the steps from the root, each a tagged step: `01 ‖ length ‖ name bytes` for a member, `02` for an
  array element, without its position, because containment ignores positions (as PostgreSQL's `@>` does, product
  question 4); `00` ends the path. A member named `*` is therefore never confused with an array step, and the path
  never runs into the value. Corrected 2026-11-11 (review RJ2): the first version wrote the array step as `*`.
- **Value encoding**: a kind tag (string, number, `true`, `false`, `null`), then for a string its bytes and for a number
  an exact normalized decimal form (sign, significant digits without leading or trailing zeros, exponent), so `1`,
  `1.0` and `1e0` give the same term and the string `"1"` another. Document 13 keeps the text as written; this
  normalization exists only inside the term.
- **Path terms carry no kind.** Corrected 2026-11-11 (review RJ1): the first version wrote `P ‖ path ‖ kind`, so
  `status` as a string and `status` as a number gave two terms, and "has `status`" needed the union of six lists, not
  the one term the queries claimed. Only containment of an empty object or array needs the kind, and it gets it from
  the recheck.
- **Long terms**: a term over a threshold (to measure; the key limit is 2,048 bytes) is replaced by `H ‖ xxh3-64` of
  its bytes (the pager already depends on `twox-hash`). A hashed term can collide, so it always needs a recheck.
- **Path terms of array elements.** Corrected 2026-11-11 (review RJ7): the first version made path terms for members
  only, so `{"items":[{}]}` had no term for its element and `JsonExists(items/0)` lost it. Every value's position now
  gives a path term, an element's ending in the array step; terms are a set per document, so an array adds one term
  whatever its length, and an empty array adds none, which is right: `{"items":[]}` has no `items/0`.
- **Count**: a document gives about one path term per member, one per non-empty array and one value term per scalar.
  Document 13's M order (4 KB, 22 lines) has about 200 scalars, so about 400 terms; a flat document of 10 members,
  about 20 [I].

### Structure

| Piece | Shape | From |
|---|---|---|
| Term dictionary | A normal tree: key = term bytes, value = a tagged reference | Module 5 |
| Few rows | The sorted, compressed list of primary keys, inline in the dictionary value | — |
| Many rows | A posting list of primary keys; the dictionary value holds its root | Module 14 |

The step from inline to posting list happens past a size threshold (to measure: a few hundred bytes, so that the
dictionary's leaves stay dense and an inline list rewritten on each change stays cheap). Corax adds a third level,
one entry inline in the reference itself; with keys of 16 bytes or more that saves little, and the inline list covers
it. Module 15's compact trees could later hold the dictionary with compressed keys, if its size calls for it.

### Posting lists of primary keys

Each list holds the encoded primary keys of the rows that contain the term, sorted by their bytes: the tuple encoding
sorts with `memcmp` (document 3), so intersecting lists is a merge over bytes, with no decoding. How a list is
compressed depends on the key:

| Primary key | Encoding of a list | Inserts |
|---|---|---|
| One integer column (`i64`, values from a sequence or not) | Deltas, block-packed (FastPFor, as Voron) | Appends when the key comes from a sequence |
| 16 bytes (`ulid`, `uuid`) | Split in milliseconds and the rest, or plain deltas over 128 bits; raw when the deltas are large | Near the end for `ulid` and `uuid` v7; anywhere for `uuid` v4 |
| Variable or composite with `string` or `bytes` | Shared prefixes (front coding) | By key |

Module 14's PRD fixes which of these its first version supports (product question 6). For the integer case, Voron
accepts only non-negative values and reserves bit 0 for its merge; Krummi need not: storing a key with its sign bit
flipped keeps the order and the deltas for every `i64`, and pending removals can live in a list of their own.

### Queries

Through module 17's expressions:

The node names are document 13's (`JsonPath`, `JsonExists`, `JsonAs`, `JsonAny`, `JsonAll`) plus `JsonContains`,
which this document adds. A query path is typed (document 13, *Paths*): member names and array **indexes**; terms keep
member names and replace every array position by the array step. So the index answers exactly only what it can tell
apart (corrected 2026-11-11, reviews RJ1 and RJ2):

Two rules decide how a predicate uses the index (corrected 2026-11-11, reviews RJ6 and RJ8):

1. **Candidates**, computed over the predicate tree, at the top level and inside a `JsonAny` element alike: an
   existence or an equality with a term gives that term's list, except an equality on `JsonAs(f64, ...)` or
   `JsonAs(decimal(p, s), ...)`, which gives none, because many number texts round to one value and a term holds
   one exact value (Codex review R1: `{"n":0.1}` and `{"n":0.10000000000000001}` both equal the `f64` 0.1 and have
   two terms; likewise `{"n":1.235}` and `{"n":1.24}` both read as the `decimal(10, 2)` 1.24); `AND` gives the
   intersection of the lists of the
   children that have one (the others are left to the recheck); `OR` gives the union of its children's lists, and
   only when every child has one, otherwise the `OR` gives none; `NOT`, ranges and `JsonAll` give none. A predicate
   with no list does not use the index. The first version intersected the equalities of a `JsonAny` element even
   under `OR`, so `JsonAny(items, sku = "A" OR sku = "B")` lost a document with only `A` (RJ6).
2. **Exactness**: candidates are the answer, with no recheck, only when the predicate is one term, or an `OR` of such
   predicates, each of which (a) is not hashed, (b) has no array index anywhere in its full path, counting the steps
   that lead to a quantifier's array (RJ8: `JsonAny(groups/1/items, sku = "A")` erases the `1`, so a document with `A`
   only in `groups[0]` has the term), and (c) is proved by its kind of term: a value term proves an equality, a path
   term proves an existence, and a path term never proves a containment, which also needs the value's kind; and (d) does
   not read a number through a rounding extraction (`JsonAs(f64, ...)`, `JsonAs(decimal(p, s), ...)`), which rule 1
   already leaves without a list. `JsonAs(i64, ...)` and `JsonAs(u64, ...)` read the exact value of a number in any
   notation (document 13), as the term's normalization does, so their equality is proved by the term. Everything else is
   rechecked; an `AND` of exact predicates is rechecked too, which is correct, though not always necessary.

| Predicate | Terms used | Result of the index |
|---|---|---|
| `JsonExists(column, steps)`, no array index in the path | One path term | **Exact** |
| `JsonExists` with an array index in its path (`items/0/sku`, `items/0`) | The path term with the array step | Candidates: recheck (`{"items":[{},{"sku":"A"}]}` has the term of `items/0/sku` and lacks that path; `{"items":[{}]}` has the term of `items/0`, RJ7) |
| `Eq(JsonPath(column, steps), value)` on a scalar, no array index in the path | One value term | **Exact**, numbers compared by value as the term normalizes them |
| `Eq(JsonAs(string or bool, column, steps), value)`, no array index in the path | One value term | **Exact** |
| `Eq(JsonAs(i64 or u64, ...), value)`, no array index in the path | One value term | **Exact**: the extraction and the term both take the exact value (`1`, `1.0` and `1e0` are 1 for both); a fraction or an out-of-range number gives `NULL` and has a term of another value |
| `Eq(JsonAs(f64 or decimal, ...), value)`, and ranges on any typed number | None | Not answered by the index: evaluated as a filter, since texts that round to the same value have different terms (Codex review R1); an expression index on the extraction answers it |
| An equality with an array index in its path | The term with the array step | Candidates: recheck (`{"items":[{"sku":"A"},{}]}` against `{"items":[{},{"sku":"A"}]}` for `items/0/sku = "A"`) |
| `JsonAny(column, steps, element)` whose element is one exact equality, no array index in `steps` nor in the element's path | One value term through the array step | **Exact**: the term says that some element holds the value |
| `JsonAny` with an array index in `steps` (`groups/1/items`) | The term with every position erased | Candidates: recheck (RJ8) |
| `JsonAny` with several conditions, ranges, `OR` or `NOT` in the element | Rule 1: `AND` intersects, `OR` unites when every branch has a list | Candidates: recheck with the whole quantifier, unless rule 2 holds (an `OR` of exact equalities) |
| `JsonAll`, and any `NOT` | None | Not answered by the index: evaluated as a filter |
| `JsonContains(column, document)` | The terms of the document, intersected from the shortest list | Exact only when the document gives one value term and nothing else (a single scalar at a path without array indexes); otherwise candidates: recheck (`{"a":[{"x":1},{"y":2}]}` has both terms of `{"a":[{"x":1,"y":2}]}` and does not contain it; `{"a":{}}` gives one path term, which `{"a":1}` also has) |
| Any predicate whose term was hashed | The hashed term | Candidates: recheck |

The invariant behind the rules is X3: the rows a plan returns are exactly those whose document satisfies the
predicate; the candidates never miss a row that satisfies it, and a row they propose is either proved by rule 2 or
rechecked. When a multi-valued index over
the same element exists, the planner prefers it for `JsonAny`: it answers ranges and keeps the correlation.

A recheck reads the row by its primary key (one descent; the bench reads about 1,000,000 values of about 100 bytes
per second at random from a tree [K: `docs/benchmark/2026-11-09-windows-margin.md`]) and evaluates the condition on
the normalized text. The list's length is the exact number of rows holding a term (Voron keeps `NumberOfEntries`),
which gives the planner exact selectivities for single terms at the cost of one lookup [I].

### Maintenance

Synchronous, in the write's transaction (X1), in the two parts of X4. *Finding*: the write extracts the term sets of
the old and the new document, which reads both whole, and compares them (sorted, a merge). *Applying*: it adds or
removes its primary key only in the lists of terms that differ. An insert of M extracts about 400 terms and touches
about 400 lists; an update of one member still extracts both documents' terms (about 800 for M), and touches only the
two or three lists that changed. Since 7.5 has no update by path (document 13, question 8), the engine does not know
which member changed, so the finding part stays proportional to the documents' sizes; reducing it is an optimization
for later, not a guarantee. Within a transaction, changes to the same list are grouped, as Voron's
posting list does with its pending additions and removals [V], so a bulk load touches each page once.

The cost that GIN's *fastupdate* answers, many index inserts per row, is real here too. A deferred variant (the write
records the change and the store updates the index in the background, like 7.6's vector indexes) would cut the
foreground cost, at the price of searches that must also read the pending changes or see stale results. Product
question 5; synchronous is the first choice because it needs no new machinery.

Under copy-on-write, a frequent term (`"status":"shipped"`) has its last leaf changed by almost every insert: one page
per frequent term per write transaction, shared by all the rows of that transaction [I].

### Prefixes and ranges over the dictionary (open)

Recorded 2026-11-11 for the detailed design of inverted indexes, without changing rule 1 now. The user: "los
invertidos tienen una ventaja: no tienes que pensar qué vas a consultar a priori", and, to recording this: "Las dos
cosas".

- The dictionary is a tree ordered by term bytes, and every value term of one path starts with `V ‖ path ‖ kind`. So
  the value terms of one path are contiguous, and a prefix or a range of values of that path is a contiguous scan of
  the dictionary; its result is the union of the lists of the terms scanned, in term order.
- **Strings** are already in byte order inside the term: a prefix or a range of strings of one path can use the
  dictionary as it is designed. Example: the orders of one day, with the date stored as text, are the terms from
  `V ‖ cabecera/creado ‖ string ‖ "2026-11-11"` up to the prefix of the next day. The result is right only when every
  document writes the value in one form (dates in UTC with `Z`); document 13's `JsonAs(date)` and `JsonAs(timestamp)`
  are the typed alternative.
- **Hashed terms are outside the scan.** A term too long for a key is stored as `H ‖ xxh3-64` (*Terms*), without
  its `V ‖ path ‖ kind` prefix, so a scan of that prefix misses it: with `{"a":"A…A"}` (3,000 `A`) and
  `{"a":"Ab"}`, a prefix `A` on `a` would find only the second, and no recheck brings back a row that was never a
  candidate (Codex review of 2026-11-11, report `2026-11-11-code-03.md`, R1). The scans above are therefore complete
  only over unhashed terms, and enabling them needs one of two things, to choose with the detailed design: hashed
  terms that keep their path and the first bytes of their value before the hash (`H ‖ path ‖ kind ‖ value prefix ‖
  hash`), so that a scan also visits them and rechecks them, with any prefix longer than the kept bytes rechecked;
  or no list, and a full scan, for any path that may hold hashed values. A path whose own encoding is too long is
  always hashed and always falls back.
- **Numbers** are not: the normalized decimal form of *Terms* (sign, digits, exponent) does not sort by value. Numeric
  ranges over the dictionary need an order-preserving decimal encoding in the term (sign, then exponent, then
  digits, as order-preserving key encodings of arbitrary decimals do), to choose when the inverted index is designed.
- **Cost against an expression index.** For values that are almost unique per row, such as instants, each term holds
  one row: the scan reads one dictionary entry per row, whose key repeats the path and the text (about 35 bytes for
  an instant, against 9 for an encoded `timestamp` in an expression index [I]). It works, at about twice the size;
  the expression index stays the choice for a path queried by range often, and the inverted index the one for paths
  nobody chose in advance.
- Until that design, rule 1 keeps giving ranges no list: conservative and correct (X3).

## Row identifiers, considered and dropped

GIN and Corax name rows by small integers; Krummi's tables have none. The discussion went through these steps:

1. **Identifiers of the index's own** (a counter, a tree from primary key to identifier, another back): compression
   to about 1 byte per entry for any key, at the price of two structures and one more descent per result.
2. The user: "sería como añadir una columna u64 a la tabla y hacer un índice único sobre esa columna", then "no tiene
   por qué ser un índice secundario único, cuya clave está codificada como tupla, mejor sería un árbol con la clave el
   id de fila": a column with the identifier, so updates find it in the row they read anyway, and a fixed-size tree
   from identifier to primary key (module 5; 8 + 16 bytes per row for a `ulid` key).
3. The user: "lo que ganamos en la compresión lo perdemos en el árbol". The arithmetic [S, I]: the identifiers cost
   about 40 bytes per row once and save about 11 bytes per term entry against slowly inserted `ulid` keys, so they win
   above about 3 to 4 terms per row and lose below; against `ulid` keys inserted fast they save about 1 byte per
   entry, so they win only above about 40 terms.
4. The user: "hay otras soluciones: 1) creas la tabla con la clave primaria como u64 con secuencia, problema
   resuelto, si quieres añadir un poco de seguridad añades un ulid como identificador secundario para usarlo en el
   frontend. 2) dos tablas, una con tus datos "principales" con el ulid como clave primaria y otra con el id secuencial
   el ulid como clave externa a la anterior y la columna JSON." And: "No hay problema en que las claves sean i64 no
   tiene por qué ser u64".

Outcome: posting lists hold primary keys, and nothing else. A table that needs compact inverted indexes chooses its
key accordingly:

- **Pattern 1**: an `i64` primary key from a sequence, and a `ulid` column with a unique index (7.2) as the public
  identifier. The lists compress like row identifiers; the `ulid` index costs about what the identifier tree would
  (about 32 bytes per row [I]), but it serves a lookup the application needs; the integer is never exposed, so it
  reveals no volume and cannot be enumerated.
- **Pattern 2**: the main data in a table keyed by `ulid`, the JSON in a second table keyed by an `i64` from a sequence
  with the `ulid` as a column. Krummi has no foreign keys (tables PRD, *Fuera de alcance*) and V1 queries read one
  table (document 4), so the application keeps the relation and joins; both writes share one transaction (G1). It
  also takes large documents out of the main rows, the remedy document 1 gives for wide rows (document 13, pattern 9).

## Compression

Simulations in `C:\Users\JESUS~1.LOP\AppData\Local\Temp\claude\abbc23f7a7515b82\632caf70-8da8-4ba3-915c-cf49780988da\scratchpad\ulidsim`
(`sim.py`, `docid.py`; `python -I sim.py`), 1,000,000 rows, a simple byte-aligned encoding (varint deltas; for
`ulid`, the millisecond delta, then the delta of the random part within a millisecond or its 10 raw bytes at a new
one). Block bit-packing would do better on small deltas; that is not simulated. Bytes per list entry [S]:

| Key, insert rate | Term in every row | In 10 % | In 1 % |
|---|---:|---:|---:|
| `ulid`, 1 per second | 12.0 | 12.2 | 12.9 |
| `ulid`, 100 per second | 11.0 | 11.3 | 11.9 |
| `ulid`, 10,000 per second | 2.9 | 7.9 | 10.6 |
| `ulid`, 100,000 per second | 2.1 | 2.9 | 7.8 |
| `ulid`, 1,000,000 per second | 2.0 | 2.1 | 3.1 |
| Dense increasing integers (a sequence) | 1.0 | 1.0 | 1.3 |

Raw, a `ulid` is 16 bytes. What decides it is how many entries of one list share a millisecond: within one, E15's
generator adds 1, so deltas are tiny; at each new millisecond, 80 random bits arrive. The bench's in-order load runs at
about 1,000 rows per millisecond (990,853 inserts per second [K]); an interactive application, below one. `uuid` v7 and
the sequential `uuid` share the structure; `uuid` v4 does not compress.

## Costs

| | Expression index | Multi-valued | Inverted |
|---|---|---|---|
| Entries per row | 1 | Distinct elements of the array | About 2 per scalar (M: ~400) |
| Bytes per entry | Value + primary key + 6 of cell | Same | 1 to 13 by key type (above); inline lists add the term once |
| Write | 1 extraction, at most 1 entry changed | Element set difference | Term set difference |
| Read | Descent and range, as 7.2 | Same | List intersection, then a recheck per candidate when needed |
| Ranges and order | Yes | Per element | Over one path's values, by term order: strings as designed, numbers with an ordered encoding (open; not used by rule 1 yet) |
| Unique | Yes | No (to decide with 7.2) | No |

## Risks

| Risk | Effect | Mitigation |
|---|---|---|
| Write amplification of inverted indexes | Inserting M costs about 400 list changes | Grouping per transaction; the deferred variant if JX4 asks for it (question 5) |
| Large documents with slow `ulid` keys | 12 bytes per entry, about 5 KB of index per M row | Pattern 1 or 2; measured profile of terms per row before creating the index |
| Recheck cost on broad containment queries | One row read per candidate | Intersect from the shortest list; exact single-term paths need none |
| Hash collisions of long terms | False candidates | Recheck; collisions never cause a wrong result (X3) |
| Hot terms | One leaf changed per transaction for frequent terms | Bounded by one page per term per transaction |
| Normalization of numbers in terms differs from document 13's comparison by value | Containment misses equal numbers | One normalization function shared by terms and structural equality, with property tests |

## Experiments to run when it is designed in detail

| Id | Question | Method |
|---|---|---|
| JX1 | Terms per document on real corpora | Document 10's `profile` over sample data: path and value terms per row, distribution |
| JX2 | List size by key type and insert rate | The simulation above with block bit-packing (FastPFor) instead of varints, and with deletes |
| JX3 | Inline threshold | Dictionary size and write cost against the size at which a list moves to module 14 |
| JX4 | Write cost | Insert 1,000,000 M rows with and without an inverted index; pages written and journal bytes |
| JX5 | Containment queries | Selectivities 0.01 % to 50 %, one to five terms, against a full scan with document 13's reader |
| JX6 | Expression against inverted for one path | The same equality query through each |

## Tests and acceptance criteria

For the PRDs that build these indexes:

- **Plans against a full scan** (X3): a property test generates random documents (every kind, arrays of objects,
  repeated values, members named `*`, numbers written as `1`, `1.0` and `1e0`) and random predicates of *Queries*;
  every plan through an expression, multi-valued or inverted index returns exactly the rows that evaluating the
  predicate on every document returns.
- **The review's counterexamples**, as fixed cases: `status` as a string in one row and as a number in another, both
  found by `JsonExists(status)` through one term (RJ1); `{"items":[{"sku":"A"},{}]}` and `{"items":[{},{"sku":"A"}]}`,
  only the first returned for `items/0/sku = "A"`, both for `JsonAny(items, sku = "A")` (RJ2);
  `{"items":{"*":{"sku":"A"}}}` against `{"items":[{"sku":"A"}]}`, different terms; `{"n":1.0}` and `{"n":1e0}` found by
  `Eq(JsonPath(n), 1)` and by `Eq(JsonAs(i64, n), 1)` without a recheck, and `{"n":1.5}` by neither; `JsonAny(items, sku
  = "A" OR sku = "B")` finds `{"items":[{"sku":"A"}]}` (RJ6); `JsonExists(items/0)` finds `{"items":[{}]}` and not
  `{"items":[]}` (RJ7); `JsonAny(groups/1/items, sku = "A")` does not return
  `{"groups":[{"items":[{"sku":"A"}]},{"items":[]}]}` (RJ8); `JsonContains({"a":{}})` returns `{"a":{"b":1}}` and not
  `{"a":1}`; `Eq(JsonAs(f64, n), 0.1)` returns both `{"n":0.1}` and `{"n":0.10000000000000001}` (Codex review R1), and
  `Eq(JsonAs(decimal(10, 2), n), 1.24)` returns both `{"n":1.235}` and `{"n":1.24}`.
- **No false negatives**: for every predicate of the property test, the candidates contain every row that the full
  scan returns, before any recheck (rule 1).
- **Exactness without recheck**: for each row of *Queries* marked exact, a test hook counts zero rechecks; for the
  others, every candidate is rechecked.
- **Maintenance** (X4): an update that changes one member of M changes only the lists of the terms that differ (a
  hook counts list changes), and the cost of finding them is measured separately (JX4).

## Review history

**2026-11-11, external review (ChatGPT, passed on by the user), findings J1 to J5, written RJ1 to RJ5 here because
J1 to J7 are document 13's requirements.** The reviewer approved the decisions and asked for five corrections. Each was
first checked against the document:

| Finding | Verdict | Kind | Fix |
|---|---|---|---|
| RJ1, path terms with the kind (high) | Valid: `status` as a string and as a number gave two terms, so "has the path" was not one term | Correctness defect | Path terms without kind; empty containers rechecked (*Terms*) |
| RJ2, array positions in terms (high) | Valid: `items/0/sku = "A"` matched `{"items":[{},{"sku":"A"}]}`; found while fixing it, a member named `*` collided with the array step | Correctness defect | Exactness table of *Queries*; tagged path steps; recheck for array indexes and typed numbers |
| RJ3, the test oracle for numbers (medium) | Valid, in document 13 | Test defect | Document 13, *Tests* |
| RJ4, maintenance cost (medium) | Valid: finding the changes reads both documents whole | Wrong guarantee | X4 and *Maintenance* split into finding and applying |
| RJ5, typed paths against JSON Pointer (medium) | Valid, in document 13 | Precision | Document 13, *Paths* |

**2026-11-11, second round, same reviewer.** RJ1 to RJ5 confirmed; three counterexamples against the exactness table,
each first checked against the rules then in force:

| Finding | Verdict | Kind | Fix |
|---|---|---|---|
| RJ6, `OR` under intersection | Valid: the table intersected the equalities of a `JsonAny` element, `OR` included; a document with only `A` was lost | Correctness defect (false negative) | Rule 1: `OR` unites, and only when every branch has a list |
| RJ7, empty elements of arrays | Valid: path terms existed for members only, so `{"items":[{}]}` had no term for `items/0` | Correctness defect (false negative) | Path terms for array elements too (*Terms*) |
| RJ8, `JsonAny` after a positional path | Valid: the `JsonAny` row ignored the steps before the array | Correctness defect (false positive marked exact) | Rule 2(b): no array index anywhere in the full path |

Found while fixing RJ7, and reproducible: `JsonContains` was exact for any document with one term, but since RJ1 a path
term has no kind, so `JsonContains({"a":{}})` would have returned `{"a":1}` as exact. Rule 2(c): a path term never
proves a containment. The reviewer approved the architecture of documents 13 and 14 once these are corrected.

Also aligned: the first version of *Queries* named `JsonHasPath` and `JsonPathEquals`, which document 13 does not
define; it now uses document 13's nodes. The `[*]` notation of multi-valued indexes is now defined as the boundary
between a path to an array and a path inside each element, as document 13's `JsonAny`, since typed paths have no such
step. No product question changed.

## Product questions

1. **An extracted value too long for an index key.** Recommended: the write fails with size not valid, as 7.2 does
   for any entry (N7). Alternative: the row gets no entry, and queries cannot trust the index for that path.
2. **The wrong kind at an indexed path.** Recommended: the entry is `NULL`, consistent with typed extraction (document
   13). Alternative: the write fails, which makes the index a type constraint on the documents.
3. **Terms of an inverted index.** Recommended: path terms and exact value terms, hashing only long ones. Alternative:
   value terms only, hashed with their path, as `jsonb_path_ops` (smaller; no existence queries).
4. **Arrays in containment.** Recommended: positions ignored, as PostgreSQL's `@>`. Alternative: positional.
5. **Maintenance.** Recommended: synchronous, in the write's transaction. Alternative: deferred, as 7.6.
6. **Primary keys in module 14's first version.** Recommended: `i64` and 16-byte keys, variable keys later.
   Alternative: every key type from the start.
7. **Multi-valued expression indexes.** Recommended: yes, with expression indexes. Alternative: leave arrays to the
   inverted index.
8. **Where they land.** Expression and multi-valued indexes after 7.2 and module 17; inverted indexes after module 14.
   Whether each is a child PRD of module 7 or part of module 14 is the user's numbering. Already decided, and outside
   this question: the multi-valued index over several members of one element is a requirement of module 17's PRD,
   with document 13's quantifiers (2026-11-11; aligned 2026-11-12 with the tables PRD's annotation of 7.5).

## Sources

Repository: `docs/prd/krummi/krummi.prd.md` (D4; modules 13 to 16 and the order of 17 and 18),
`docs/prd/krummi/tables.prd.md` (N7, the 7.5 annotation, *Fuera de alcance*), `docs/prd/krummi/tables-schema.prd.md`
(E14, E15), documents 2, 3, 4, 10 and 13 of this set, `docs/benchmark/2026-11-09-windows-margin.md`.

RavenDB (local source, read 2026-11-11): `Corax/Indexing/TermIdMask.cs`; `Corax/Indexing/IndexWriter.cs` (term
dictionaries as compact trees, 717-748; small posting lists in containers, 1822-1908; promotion to a posting list,
1709-1723 and 2190-2198); `Voron/Data/PostingLists/PostingList.cs` (101-114 values and removals, 715-725 merge);
`PostingListLeafPage.cs` (FastPFor encoding, 47-112); `PostingListState.cs` (`NumberOfEntries`).

External, fetched 2026-11-11: PostgreSQL GIN (definition, posting trees, fast update, recheck):
https://www.postgresql.org/docs/current/gin.html ; PostgreSQL `jsonb` indexing (operator classes, jsonpath clauses,
expression indexes): https://www.postgresql.org/docs/current/datatype-json.html ; MySQL JSON (generated-column and
multi-valued indexes): https://dev.mysql.com/doc/refman/8.0/en/json.html

From knowledge, not fetched: FastPFor (Lemire and Boytsov, "Decoding billions of integers per second through
vectorization", 2015); front coding of sorted string lists.
