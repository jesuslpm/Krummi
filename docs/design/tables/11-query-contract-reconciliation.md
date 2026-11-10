# 11 - The query contract: reconciliation of documents 4, 6, 8 and 9

Part of the [tables design set](README.md). Written 2026-11-10, after the user forwarded a reviewer's approval of
[document 9](09-query-serialization-format.md) with seven points to settle before the PRDs change. It proposes one
contract for query documents, plans and prepared statements, and lists every change it needs in documents 4, 6, 8, 9
and 10, the README and the PRDs. It changes none of them: each change waits for the user to confirm the product
decisions below, one by one. Written from those documents, the README, `krummi.prd.md`, `tables.prd.md` and
`transactions.prd.md`; external facts were fetched on 2026-11-10 from the sources at the end. Nothing here is
implemented or measured; the comparison of formats is not repeated (the user considers it decided).

Revised the same day after the reviewer's first round on this document (findings C1-C8, forwarded by the user): see
[Review round 1](#review-round-1-2026-11-10). Rewritten passages say "corrected 2026-11-10 after review round 1" with
the finding. Revised again after the second round (C9-C13 and the classification of P1-P18): see
[Review round 2](#review-round-2-2026-11-10); its passages say "corrected 2026-11-10 after review round 2". The
third round (C14) corrected P17 and closed C1-C14 ([Review round 3](#review-round-3-2026-11-10)); §1 is now the table
of product decisions P1-P20 for the user, and §2.8 compares the random sources of 7.1.

The direction, in the forwarded words: "Apruebo la recomendación de adoptar JSON tipado como formato público de
intercambio para los AST lógicos y los planes físicos de Krummi." and "El objetivo es cerrar un contrato coherente,
sencillo, versionado y verificable, que permita a las aplicaciones y a los agentes construir, inspeccionar y ejecutar
consultas y planes sin comprometer el rendimiento de Krummi."

## Summary

1. **Four document kinds, one value encoding.** `query/1` (logical AST) and `plan/1` (physical plan) carry structure
   and parameter declarations only; `execution/1` wraps one of them with the values of one run (`args`) and limits;
   `explain/1` returns a `plan/1` document plus its diagnostics in a separate member. The type of a parameter is
   declared once; a value never repeats it.
2. **Prepared statements are the execution path**, in Rust, C and C#. A statement is prepared in a transaction and
   bound against the catalog that transaction sees; it belongs to the store and runs in any transaction, re-binding
   when the catalog the executing transaction sees at that moment differs. Compiled plans are immutable, shared, and
   keyed by the catalog state they were bound to, so a re-plan for one transaction never replaces another's (corrected
   after review rounds 1 and 2, C1, C2, C11).
3. **Catalog identities are random 64-bit tokens** written in the commit that creates them: tables, columns and
   indexes, and each schema state and index set of a table. A document pins tokens, so after a lost non-durable
   commit, an abort or a diverged restored copy, a stale document fails instead of validating with another meaning
   (2^-64 per comparison). Whether a document or statement is valid depends only on the identities and tokens the
   executing transaction sees, never on how the transaction that prepared it ended (round 2, C9, C10, C13; round 3,
   C14).
4. **The plan round-trips** within the catalog it was made for: the validator rejects every unknown member, and no
   diagnostic is ever a member of a plan.
5. **Three persistence concepts.** A stored logical AST is a versioned public contract; a stored plan has no promise
   beyond safe rejection; a prepared statement is a runtime resource, never persisted. Once bound, a statement and a
   document that carries `bind` never silently re-bind a name to another column (C6).
6. **Values at three levels**: one form Krummi emits per value, a stated set of forms the parser accepts, and the
   semantic normalization of N4. The only byte-level promise is that Krummi re-emits what it emitted unchanged (C3).
   The text forms cover 7.1's whole domain; this document changes no domain (C4).
7. **Limits per phase**, each marked contract maximum, configurable budget or safe default; values bound outside JSON
   are checked against their declarations before any allocation proportional to them (C5). The planner's search is
   bounded by construction and by a cap; the total memory of concurrent executions belongs to the application (C12).
8. **`serde_json` is the recommendation, not yet a permitted dependency**: the root PRD's D12 does not yet hold the
   exception rule, so its adoption and the exception itself are two approvals (C8). The random bits 7.1 needs come
   from the operating system through the pager, with no new crate and no D12 exception (P20, §2.8).
9. **No known correctness defect remains** after round 2; the accepted residual risk is a 2^-64 token collision per
   comparison of a stale document with a catalog state.
10. The core offers what agents need (schemas, introspection, typed errors with JSON Pointer paths, canonical output)
   and depends on no MCP or AI crate; doc 10's tools stay thin adapters.

## 1. Decisions

### Recorded as the user's direction (forwarded 2026-11-10)

- U1. Typed JSON is the public interchange format of logical ASTs and physical plans.
- U2. `Prepare → Bind → Execute` is part of the first public query API in Rust, C and C#, not a later optimization;
  AST reuse is not unconditional plan reuse.
- U3. Applications obtain, inspect, modify and resubmit the plans Krummi generates; the planner's output is reusable
  as input; the executable plan and its diagnostics are separate.
- U4. Persisted AST, persisted plan and prepared statement are three concepts; a plan is not assumed valid after an
  upgrade or a schema change.
- U5. Structure and values stay separate; one unambiguous contract per type.
- U6. JSON Schema helps generation; Krummi's validator is the authority; resource limits, rejection of ambiguous
  input, typed errors with precise paths, full validation of plans, differential tests.
- U7. `serde` and `serde_json` are preferred if the exception is approved and documented in the root PRD and the
  affected modules (round 1, C8: "No debemos dar por aprobada una excepción que todavía no existe en el PRD raíz").
- U8. AST, plans, errors, schemas and introspection serve applications, CLI, MCP and skills; no MCP or AI-provider
  dependency in the core.
- U9. Confirmed by the first review round as well resolved, not to reopen without evidence: typed JSON as the public
  format; structure separate from parameters; prepared statements from V1; explicit, editable plans; executable plan
  separate from diagnostics; cache keys from normalized trees with a full comparison after the hash; a public JSON
  Schema with Krummi's validator authoritative; agent tools decoupled from the core.

### Product decisions P1-P20

Rewritten 2026-11-10 after review round 3 as the decision table the user asked for, replacing the classification
table of round 2. P20 (the random source of 7.1) is new in this round; §2.8 has its comparison.

**Decide one by one: the decisions with real alternatives**, in the order the modules need them:

1. **P20, random source** (blocks 7.1): the pager reads the operating system's generator through features of crates
   it already uses (recommended), or the `getrandom` crate. **Decided 2026-11-10: the pager function (§2.8).**
2. **P11, JSON library** (17): `serde_json` after writing the dependency rule into D12 and annotating it
   (recommended), or a strict reader of Krummi's own. **Decided 2026-11-10: `serde_json`, in the query crate only;
   the dependency rule is already in the root PRD (annotation of `b471cc4`), so module 17's PRD annotates the crates.**
3. **P16, pinning columns** (17): pin column identities (recommended), pin the table's whole schema state, or re-bind
   by name like SQL text. **Decided 2026-11-10: pin column identities.**
4. **P4, re-planning** (17): doc 4's adaptive policy (recommended), re-plan only on catalog changes, or plan every run. **Decided 2026-11-10: doc 4's
   adaptive policy.**
5. **P6, promise for stored queries** (17): none before 1.0 and every published major after it (recommended), or only
   the current and the previous major after 1.0. **Decided 2026-11-10, neither: the user's semantic versioning with
   one reader (§2.1, *Versions*).**
6. **P15, constants** (17, 18): parameters only (recommended), or integer literals for `limit`, `offset` and `k`.
7. **P3 with P14, statement scope** (17, 18): a store-scoped statement shared by threads (recommended), or a statement
   per transaction, or SQLite's one-thread statement holding its values.

**Approve as a block: the rest.** P1, P2, P5, P7, P8 and P19 are backed by earlier decisions; P9, P10, P12, P13, P17
and P18 are new, but their alternatives are minor or there is none left after the reviews; each is stated below with
its recommendation.

**Block approved (2026-11-10).** The user approved the twelve, with the external review's verdict: "Considera aprobadas
estas siete decisiones con las aclaraciones indicadas" (P9, P10, P12, P13, P17, P18, P19), and for P1, P2, P5, P7 and
P8 "conserva las decisiones anteriores que ya estén formalmente aprobadas" (U1-U6, U9). The clarifications, each
applied where the decision lives:

- P9: "los límites necesarios para evitar agotamiento de recursos no puedan desactivarse accidentalmente mediante un
  presupuesto arbitrariamente grande". Every budget that guards memory or work has a hard maximum; §2.5.
- P10: errors structured and precise enough for tools and agents to correct the document (already §2.5's path,
  expected and found).
- P12: "La probabilidad de 2⁻⁶⁴ corresponde a una comparación entre valores independientes, no a la probabilidad
  acumulada de colisión durante toda la vida del almacén." §2.2 states both, and the detectable collisions are drawn
  again and tested.
- P17: "Mantén diferenciada la revinculación de consultas lógicas y la validación de planes físicos explícitos" (§2.2
  keeps them as two rules).
- P18: approved for V1; a C convenience function stays possible later if performance or ergonomics justify it.
- P19: "`column_id` no debería tratarse como un contador si ahora es una identidad aleatoria". `column_id` is now the
  column's random identity (doc 1); the internal counter left is the `u16` schema version, and it fails before any
  state changes.

| # | Proposed decision | Viable alternatives | Recommendation | 7.1 | 7.2 | 17 | 18 | Backing or approval |
|---|---|---|---|---|---|---|---|---|
| P1 | Typed JSON documents (`query/1`, `plan/1`, `execution/1`, `explain/1`) replace the binary envelope; Krummi's operators and the Substrait vocabulary stay | — | Adopt | — | — | Parser, emitter, golden documents | C# writes JSON with `Utf8JsonWriter` | Backed: U1, U9 |
| P2 | Documents run through prepared statements from the first version | — | Adopt | — | — | Statement API | C functions and C# API of §2.2 | Backed: U2, U9 |
| P3 | A statement is prepared in a transaction, belongs to the store, runs in any transaction; closing a store with live statements fails with misuse | A statement per transaction (simpler lifetime, prepared again per transaction) | Store-scoped: reuse across transactions is the point of preparing | — | — | `Statement` tied to the store's lifetime | Handles hold the store; close rule | Approval (block 7) |
| P4 | Logical statements re-plan on a catalog change, a statistics rebuild or values outside doc 4's band; plan statements never re-plan | (a) re-plan only on catalog changes; (b) plan every run | Doc 4's policy: stable for most shapes, adaptive where values matter; a fixed plan is one `explain` away | — | — | Cache, revalidation, counters | — | Approved 2026-11-10; revalidation itself backed by U2 |
| P5 | `explain` returns an editable, resubmittable plan; explain text output only; DD9 no longer applies to documents | — | Adopt | — | — | `explain/1` | Explain through C | Backed: U3, U9 |
| P6 | Stored queries are a versioned contract; stored plans have no promise beyond typed rejection; statements never persist | After 1.0: every published `query` major accepted, or only the current and the previous | None before 1.0 (as D7); after 1.0 every published major, since queries are small and old majors cost a reader each | — | — | Versioning rule, one reader | — | Decided 2026-11-10: semantic versioning, one reader that accepts any earlier version unless it uses a removed construct, and no construct ever changes meaning (§2.1, *Versions*); the three concepts backed by U4 |
| P7 | Text forms cover 7.1's whole date and time domains; no domain change | Restrict 7.1's domain to 0001-9999 (a 7.1 decision) | Whole domain | Text conversion over its domain | — | §2.4 forms | Raw accessor or error for dates outside .NET's range | Backed: U5, C4 |
| P8 | A versioned JSON Schema per kind, published with the library | — | Adopt; distribution is open question 4 | — | — | Schema files, CI check | Shipped schema (if question 4 says so) | Backed: U6, U9 |
| P9 | Document limits are application budgets with safe defaults, ceilings only where an implementation bound exists; a budget that guards memory or work has a hard maximum no setting exceeds | Fixed contract maxima | Budgets, as doc 4 question 9 recommended | — | — | `limits`, defaults, hard maxima | Limits in the C options | Approved 2026-11-10 |
| P10 | Errors with JSON Pointer path, expected, found; new D8 class **invalid document**; stale documents fail with N10's **schema mismatch** | Report bad documents as misuse | New class: an application must tell a bad document from a call in the wrong order | Schema mismatch data (tokens, versions) | — | Error object | Status code | Approved 2026-11-10; the path backed by U6 |
| P11 | `serde`, `serde_derive`, `serde_json` and their crates, after the dependency rule is written into D12, annotated in D12 and module 17's PRD | A strict reader of Krummi's own in safe Rust | `serde_json` (§2.6), the reviewer's preference | — | — | Dependencies of the query crate only | None (C# uses `System.Text.Json`) | Approved 2026-11-10: `serde_json`; module 17's PRD annotates the crates |
| P12 | Catalog identities and state tokens are random 64-bit values, readable by applications through introspection and in emitted documents; collisions stated at 2^-64 per comparison and at most m × 2^-64 over m comparisons in a store's life; a drawn value equal to a known one is drawn again | Identities only inside emitted documents, no introspection call | Introspection: agents and tools need it without running `explain` | Catalog stores identities (a column's is its `column_id`) and `schema_state`; describe call | Index identity, `index_state` | `refs`, `bind`, cache keys | Describe through C | Approved 2026-11-10 |
| P13 | Objective 4, AI-assisted development, in the root PRD; no MCP or AI-provider dependency in the library | Leave the objective out | Add it; the reviews treat it as a goal | — | — | Introspection and schemas serve it | — | Approved 2026-11-10; the dependency rule backed by U8 |
| P14 | One statement serves several threads and open cursors; argument sets and cursors one thread at a time | SQLite's model: values in the statement, one thread at a time | Shared: read transactions are shareable (T6) | — | — | `Send + Sync` statement | Thread-safe handle; argument set handle | Approval (block 7) |
| P15 | No literals: constants such as a fixed `limit` count are parameters | Integer literals for `limit`, `offset` and `k` only | Parameters only: one way to write a value, as the README says | — | — | No literal nodes | The builder turns constants into parameters | Approval (block 6) |
| P16 | A statement pins the column identities it bound; a `query` may carry `bind`, emitted by Krummi, optional on input | (a) pin the whole schema state; (b) re-bind by name | Pin column identities: catches a replaced column, survives unrelated changes | Stable column identities | — | `bind`, binding check | — | Approved 2026-11-10 |
| P17 | Validity follows the identities and tokens the executing transaction sees, whatever happened to the transaction that prepared; no restriction on `prepare` and `explain` in write transactions | None left after C13 and C14 | Adopt | Tokens drawn when a change is made in a write transaction | Same for indexes | Validity rule (logical re-binding and plan validation kept apart), cursors across changes | Cursor misuse after a change | Approved 2026-11-10 |
| P18 | No one-shot C function in V1; convenience methods in Rust and C# | A C one-shot taking `execution/1` | No C one-shot: four calls do it, one path to test; one may be added later if performance or ergonomics justify it | — | — | Rust convenience | C# convenience | Approved 2026-11-10 (V1) |
| P19 | A change past the maximum of an internal counter (the `u16` schema version) fails with a typed error before any state changes; no counter wraps or reuses a value; `column_id` is a random identity (P12), not a counter | — | Adopt; 7.1 fixes the class | The error and its test hook | — | — | — | Approved 2026-11-10; backed by round 2, C10 |
| P20 | The pager exposes one safe function that fills a buffer from the operating system's generator, through `rustix`'s `rand` feature on Linux and `windows-sys`'s `Win32_Security_Cryptography` on Windows; 7.1's generators and the catalog tokens use it | The `getrandom` crate as a dependency of the engine | The pager function: no new crate, no D12 exception, no generator of Krummi's own (§2.8) | Generators and tokens call it | Index tokens | — | — | Approved 2026-11-10: A, random bits from the operating system at each new millisecond, no `XorShift64` in between (§2.8) |

Design, not product: the document layout, `"op"` tags, the type table's grammar, the default values of limits, the
validation phases, the cache key and its entries, the token mechanism, the planner's search cap, the API names and the
test plan.

## 2. The contract

### 2.1 Documents

Every document is a JSON object whose member `"krummi"` names its kind and version, `major.minor` (`"query/1.2"`;
`"query/1"` means 1.0). Below, `query/1` names the whole major.

**Versions (P6, decided 2026-11-10).** User: "Yo creo que lo mejor es utilizar versionado semántico y además intentar
hacer compatibles los formatos hacia atrás. También podría ser que una consulta escrita en la versión 1.2 no tenga
ninguna incompatibilidad con la versión 2.1. No quisiera mantener código antiguo..."

- Semantic versioning per kind: a minor version only adds constructs; a major version may remove them.
- One reader, the current one, with no code kept for old versions. It accepts a document of any earlier version; one
  that uses a removed construct fails with **invalid document**, naming the construct and the version that removed
  it, and one of a later version than the reader fails with a typed error too.
- No construct ever changes meaning: a new meaning takes a new name. User: "en vez de cambiar el significado de like
  por ejemplo, podríamos añadir cs_like como case sensitive like". V1's `prefix` compares bytes, so it is already
  case-sensitive; a case-insensitive one would be a new operation (for example `ci_prefix`), never a change to
  `prefix`. This is what makes accepting an old document safe: it reads with its meaning or fails.
- Before 1.0 there is no promise, as D7 for stores.
- The cost: a stored query that uses a removed construct has no automatic migration; its error says what to change.
- `plan/1` follows the same versioning, but stored plans keep no promise beyond a typed rejection (U4).

| Kind | Holds | Persisted by applications | Accepted by |
|---|---|---|---|
| `query/1` | `table`, optional `bind` (C6), `root` (logical nodes), `params` (declarations) | Yes, the public contract | `prepare` |
| `plan/1` | `refs` (tables, indexes and columns by identity and name; tables also by `schema_state` and schema version), `root` (physical operators), `params` | Allowed, no promise (P6) | `prepare` |
| `execution/1` | `statement` (a `query/1` or `plan/1`), `args`, `limits` | Tests and tools | The capability layer (tools, tests); not the C hot path |
| `explain/1` | `plan` (a `plan/1`), `query` (the normalized `query/1` it came from, with `bind`, when it came from one), `diagnostics` | Output | Nothing: submitting it fails at `/krummi`; its `/plan` is submittable |

```json
{ "krummi": "query/1", "table": "customers",
  "params": [ { "name": "country", "type": "string" },
              { "name": "since", "type": "timestamp" },
              { "name": "n", "type": "u32" } ],
  "root": { "op": "limit", "count": { "param": 2 },
    "input": { "op": "sort", "keys": [ { "col": "created", "desc": true } ],
      "input": { "op": "filter", "predicate": { "op": "and", "args": [
          { "op": "eq", "col": "country", "value": { "param": 0 } },
          { "op": "ge", "col": "created", "value": { "param": 1 } } ] },
        "input": { "op": "scan" } } } } }
```

```json
{ "krummi": "execution/1", "statement": { "krummi": "query/1", "...": "as above" },
  "args": [ "ES", "2026-01-01T00:00:00.000000Z", 20 ] }
```

- **Declarations, values, self-contained documents.** A declaration is a type descriptor plus an optional `name`
  used only in errors and explain; references are `{ "param": i }`, by index. Values never appear in `query/1` or
  `plan/1` (P15). The values of one run are bound through typed APIs in Rust, C and C#; in JSON they are the
  positional `args` array of `execution/1`, one per declaration, in the form its declared type fixes. The
  self-contained document for tests and tools is `execution/1`. The user's first example,
  `{"type":"u64","value":"18446744073709551615"}`, becomes `{"type":"u64"}` in `params` and `"18446744073709551615"`
  in `args`: the same information with the type stated once.
- **Type descriptors** are objects: `{"type":"u64"}`, `{"type":"decimal","precision":12,"scale":2}`,
  `{"type":"vector","element":"f32","dim":768}`, `{"type":"list","element":{"type":"i32"}}` (for `in`).
- **Names inside the tree, ids as checks.** Both kinds name columns; a plan's computed columns get a name from the
  `project` that creates them; doc 8's input ordinals become internal, produced by binding. Ids never replace names;
  they check them (corrected 2026-11-10 after review round 1, C6, and round 2, C9): a plan's `refs` name the table
  (identity, name, `schema_state`, and the readable schema version), each index and each referenced column by identity
  and name together; a query's optional
  `"bind": { "table": "9f3a0c7e5b1d2468", "columns": { "country": "03c1d9a07e44b2f5" } }` does the same for a query.
  Identities and tokens are §2.2's random 64-bit values. A name that resolves to an object with another identity fails
  with schema mismatch, naming the object, the expected and the found identity.
- **Shared vocabulary.** Expression nodes are the same in both kinds (`eq`, `lt`, `le`, `gt`, `ge`, `and`, `or`,
  `not`, `in`, `prefix`, `is_null`, `distance`); doc 4's `VectorWithin` becomes a comparison over a `distance`
  operand, as doc 8 has it. Physical operators take doc 8's names in snake case (`table_scan`, `index_scan`,
  `key_lookup`, `filter`, `project`, `sort`, `top_k`, `limit`, `aggregate`, `key_union`, `key_intersect`); doc 4's
  `IndexRangeScan`, `Union`, `Intersect` and `Fetch` go. Reserved names (`hash_join`, `group_by`, ...) are unknown
  operators to a first-version reader; no opaque `Extension(tag, bytes)` node exists.
- **Tags.** Internally tagged `"op"` (doc 9's D1): it reads naturally and maps to one JSON Schema `const` per variant.
  It costs 1.4-1.6 times the decode of outer tags (doc 9, measured), off the per-run path with prepared statements; B2
  decides whether a hand-written deserializer is worth it.

### 2.2 Prepared statements

Corrected 2026-11-10 after review round 1 (C1, C2, C6): prepare binds against the catalog a transaction sees, not
the latest commit; compiled plans are immutable entries per catalog state. Corrected again after review round 2 (C9,
C10, C11, C13): catalog identities are random tokens, the catalog is read at each call, and the round-1 restrictions
on write transactions are withdrawn.

**Catalog identities (round 2).** Every table, column and index gets a random 64-bit identity when it is created,
and every table carries two state tokens, also random 64-bit: `schema_state`, drawn again at every schema change
(N3) of the table, and `index_state`, drawn again whenever one of its indexes is created or dropped. Each is written
in the catalog entry of the change that draws it, inside the same commit, so no durable write is added (G11
unchanged). They come from the random source that 7.1's `ulid` and `uuid` generators already need (README, generators
entry; §2.8, P20); a test hook may inject a deterministic source. A drawn value that already exists in the store's catalog, or
in the table's schema history, is drawn again; if the random source fails, the change fails with an error and nothing
weaker is drawn (self-review of round 2). In JSON an identity or token is a string of 16 lowercase
hexadecimal digits. A column's identity is doc 1's `column_id` itself, so a column has one identifier (P19, approved
2026-11-10); doc 1's `u16` schema version stays internal to rows and storage and is still emitted, read-only, for
people and error messages. A stale document is caught because the tokens it pins differ from the current ones; two
different states share a token with probability 2^-64 per comparison, and the ids and names must agree as well. That
is the bound of one comparison of independent values, not of a store's life: over m comparisons of stale pins with
current tokens, the probability that any one matches is at most m × 2^-64 (for 10^12 comparisons, about 5.4 × 10^-8).
The collisions Krummi can detect, a drawn value equal to one in the catalog or the table's schema history, are drawn
again, and a test injects a source that repeats a value to prove it. Krummi states these bounds instead of calling a
collision impossible (clarified 2026-11-10 at P12's approval).

**Catalog state.** For one table, the catalog state is (table id, `schema_state`, `index_state`). A read transaction
sees an immutable catalog (G3, T2). A write transaction sees its own schema changes as it makes them (T2), so the
state it sees can change during its life; only its own thread changes or reads it (T6). Every `prepare`, `explain`
and execution therefore reads the state the transaction sees at that call, not at the transaction's start.

**Lifecycle.**

| Step | Rust | C | C# | Effect |
|---|---|---|---|---|
| Create | `tx.prepare(doc)` | `krummi_prepare(tx, doc, &stmt)` (read or write transaction) | `tx.Prepare(builder or UTF-8 JSON)` | Phases 0-4 against the catalog `tx` sees now; records the identities it bound (P16) and that catalog state; the statement outlives `tx` and belongs to the store (P3) |
| Bind | `stmt.args()`, `args.set(i, value)` | `krummi_args_new`, `krummi_args_set_<class>` | `stmt.CreateArgs()`, `args.Set(i, value)` | Checks each value against its declaration before copying it (§2.5); invalid value otherwise |
| Execute | `stmt.query(&tx, &args)` | `krummi_stmt_query(stmt, args, tx, &cursor)` | `scope.Query(stmt, args)` | Requires every parameter bound; obtains the compiled plan for the catalog state `tx` sees now (below); copies the values into the cursor; returns a batch cursor (doc 4) |
| Explain | `stmt.explain(&tx, Some(&args))` | `krummi_stmt_explain` | `stmt.Explain(...)` | `explain/1` for the catalog state `tx` sees now, pinned to its tokens; without values, the generic plan with default estimates |
| Reset | `args.clear()` | `krummi_args_clear` | `args.Clear()` | Unbinds every value; the statement is untouched |
| Close | drop | `krummi_args_free`, `krummi_stmt_free` | `Dispose` | Open cursors keep their own reference to their compiled plan and finish normally |

Bind functions are the C signature of the type table, as `sqlite3_bind_*` are; they are not a second format. One-shot
execution is a convenience method on top in Rust and C# (P18).

**Execution in a transaction X.** If the catalog state X sees equals the statement's bound state, the compiled plan
is used. Otherwise the statement re-binds against what X sees: a logical statement resolves its names again and
requires the same column identities (P16), then plans; a plan statement re-runs phases 2-4 against X and fails with
schema mismatch when a table or column identity, a name or `schema_state` differs, or a referenced index is gone. An
old read transaction after a schema change therefore runs a plan for its own catalog, and a plan `explain` produced
in T1 runs in T1 although a later commit dropped its index (the counterexample of C1).

**Write transactions (P17, round 2).** Inside a write transaction W, `prepare` and `explain` work on the catalog W
sees, its uncommitted changes included, and each schema or index change in W draws new tokens at once. So:

- A statement prepared in W and executed after a later change in W re-binds (the tokens differ) and sees the change:
  a renamed or dropped column fails with schema mismatch; an added column or index is planned for.
- Validity is decided by what the executing transaction sees, never by how W ended (corrected 2026-11-10 after review
  round 3, C14). A statement or document is valid in a transaction X when every identity and token it pins exists in
  the catalog X sees: a logical statement pins the table and column identities; a plan pins the table identity, its
  `schema_state` and the identities of the indexes and columns it uses.
- So a document bound to a committed state that W did not change (W wrote only rows, or changed other objects) stays
  valid in W, after W commits and after W aborts. A document bound to a state that only W created is valid in W while
  that state is current, valid elsewhere only after W commits and while the state lasts, and fails with schema
  mismatch everywhere once W aborts, because the state never existed outside W. A plan that uses an index fails in
  every transaction that no longer sees the index: in W after W drops it, and after W commits, in every transaction
  that sees the drop; a transaction that still sees the earlier state (an older read, or any transaction after W
  aborts) runs it.
- Compiled plans W put in the shared cache under tokens that vanished with an abort are never hit again; LRU evicts
  them.
- A cursor does not survive a change of its own write transaction: in Rust the cursor borrows the transaction and the
  change needs it mutably, so it does not compile (as T8's references); in C the cursor's next call after a mutating
  call of its write transaction returns misuse, as doc 6's write-transaction slices.

**Compiled plans: ownership and lifetime (C2).** A compiled plan is immutable and reference-counted. It holds the
operator tree, identities, column positions for its schema and the declared types; it holds no page number, tree or
cursor reference, and no bound value: operators open their trees in the executing transaction when the cursor is
created, and encode range bounds from the cursor's own copy of the values. The store's cache maps (shape hash, then a
full comparison of the normalized tree, declared types, table id, `schema_state`, `index_state`) to one compiled plan.
Tokens are never reused (up to the 2^-64 bound), so a key never denotes two catalogs. Statistics and values change
only which plan is cheaper, never whether a plan is correct for its catalog state, so every entry is correct for every
transaction that sees its key:

- A re-plan for another catalog state inserts another entry; it never touches an entry of another key.
- A re-plan for the same key (newer statistics, values outside doc 4's band) replaces the entry for later lookups
  only; running cursors keep the plan they took.
- A plan made for one run's values (doc 4: "the planner runs again for this call") is used by that cursor and not
  cached.
- Counters that drive re-planning (doc 4's two re-plans before "plan per call") are atomics; a race changes cost,
  never results. Eviction is LRU over a bounded number of entries (doc 4: 256), and an evicted plan lives on in the
  cursors using it.

**Invalidation and re-planning.**

| Event seen by the executing transaction | Logical statement (`query/1`) | Plan statement (`plan/1`) |
|---|---|---|
| A column renamed | Fails with schema mismatch: the name no longer resolves (re-prepare) | Fails with schema mismatch |
| A column dropped, and another added under its name (N3: "es nueva") | Fails with schema mismatch: same name, another identity (P16) | Fails with schema mismatch |
| Any other schema change (N3) | Re-binds and re-plans | Fails with schema mismatch, naming the table, both schema versions and both tokens |
| Table dropped and recreated under the same name | Fails: table identity differs | Fails: table identity differs |
| An index created | Re-plans (`index_state` changed) | Re-binds and runs: it does not use the index |
| An index the plan uses dropped or recreated | Re-plans | Fails: index identity differs or the index is gone |
| Statistics rebuilt | Re-plans at the next run, same key | Runs unchanged; only explain's estimates change |
| Values whose estimate leaves doc 4's band (4x) | Plans that run separately; after two such runs, every run | Runs unchanged |
| A non-durable commit lost in a crash (G11), then other changes | A pinned column identity or token from the lost state matches nothing: schema mismatch | Same |
| A restored backup or a compacted copy (modules 10, 11) | Tokens are copied unchanged, so documents made before the copy mean the same on both; later changes on either draw independent tokens | Same |
| Krummi upgraded | Not applicable: statements die with the process | Not applicable |

A logical statement keeps the AST, its shape key and its bound identities; it never keeps "the" plan. Applications that
want a fixed plan prepare the `plan` member of an `explain/1`.

**Exhaustion (C10, P19).** Identities and tokens are random, so they have no maximum; `column_id` is one of them, not
a counter. Doc 1's `u16` schema version and any other internal counter fail with a typed error, checked before any
state changes, when the next change would exceed them (the class is 7.1's); none is ever wrapped or reused, and none
identifies a catalog state in a document.

**Threads (P14).** A statement is `Send + Sync` (C#: thread-safe); an argument set and a cursor are used by one thread
at a time (documented, as cursors in doc 6); an execution in a write transaction runs on that transaction's thread
(T6; the C interface checks it, doc 6).

**Cost of a run** (estimate): a comparison of catalog states and, for logical statements, doc 4's revalidation (a
histogram lookup or two boundary descents); bound values copied once; no JSON. Decoding stays in prepare (doc 9: 1-9
µs per small document, measured).

### 2.3 Plans and their diagnostics

The rule that resolves doc 9 (line 257 "fields the validator ignores on input" against line 260 "unknown fields ...
rejected") and doc 8 (line 119, `est` "ignored by the executor"):

1. A `plan/1` document holds only members that change what it does. The validator rejects every other member,
   including `est_rows`, `est_cost` and any metric.
2. Diagnostics exist only in `explain/1`'s `diagnostics`: one entry per operator, addressed by the JSON Pointer of the
   operator in that same `explain/1`'s `plan` (for example `"/plan/root/input"`), with estimated rows and their
   confidence, cost, and after execution actual rows, batches, time, peak memory and spill (doc 8, *Explain*).
3. A plan Krummi emits for a transaction passes Krummi's validator unchanged, with the same meaning, in that
   transaction and in every transaction that sees the identities and tokens it pins, however the transaction that
   made it ended; preparing `explain.plan` there and running it with the same values returns the rows of the statement
   that produced it. Elsewhere it fails with schema mismatch, never runs with another meaning, up to §2.2's 2^-64
   bound, also after a lost non-durable commit or an abort (corrected after review rounds 1, 2 and 3, C1, C9, C13,
   C14).
4. Krummi writes documents in one deterministic form (fixed member order, canonical values, §2.4), so it re-emits a
   document it emitted byte for byte; input member order is free (I-JSON §2.3).
5. Resource limits are arguments of the execution (`limits` of `execution/1`, `QueryLimits` in the APIs), never
   members of a plan. Doc 8's R1 says "plans carry resource limits", against its own line 151; this fixes R1.

The explain text tree stays output only (DD9).

### 2.4 Values

Corrected 2026-11-10 after review round 1 (C3, C4). Three levels, kept apart:

- **Emitted form**: Krummi writes exactly one form per value.
- **Accepted forms**: what the parser takes. For every type except floats it is the emitted form only. For floats, any
  JSON number (RFC 8259 grammar) is accepted, so `1`, `1.0` and `1e0` are three accepted forms of one value.
- **Semantic normalization**: N4's rules applied after parsing: `-0` becomes `+0` and every NaN the canonical NaN; `in`
  lists are sorted and deduplicated after that normalization.

The guarantee is no stronger than this: every accepted document maps to exactly one typed value; Krummi re-emits a
document it emitted byte for byte; parsing a document Krummi emitted gives back the same typed values. Re-emitting
arbitrary accepted input gives the canonical form, not the input.

| Type | Emitted | Also accepted | Rejected (examples) |
|---|---|---|---|
| `bool` | `true`, `false` | — | `0`, `"true"` |
| `i8`-`i32`, `u8`-`u32` | Integer number | — | `1.0`, `1e2`, `-0` (parsed as a float, a type error), out of range |
| `i64`, `u64` | Decimal string, e.g. `"-9223372036854775808"`, `"18446744073709551615"` | — | Numbers, `"+1"`, `"007"`, `"-0"`; `"18446744073709551616"` (invalid value) |
| `f32`, `f64` | Shortest number that reads back to the same bits; `"NaN"`, `"Infinity"`, `"-Infinity"` | Any JSON number, correctly rounded to the type (ties to even); subnormals kept, never flushed; a decimal below the smallest subnormal rounds to zero | Other strings; a number whose rounding overflows (`1e400`, `3.5e38` for `f32`): invalid value, never infinity |
| `-0`, NaN payloads | Never emitted | `-0` (normalized to `+0`) | — |
| `decimal(p, s)` | String with exactly `s` fractional digits (no point when `s = 0`), no exponent, `+`, leading zeros or sign on zero | — | `"1234.5"` for scale 2, `"1.2345e3"`, `"-0.00"`; more than `p` digits (invalid value) |
| `string` | JSON string; escapes only where JSON requires them | Any JSON escape of the same characters | Lone surrogates (I-JSON §2.1) |
| `bytes` | Base64, RFC 4648 §4 alphabet, padded, pad bits zero (§3.5) | — | base64url, missing padding, non-zero pad bits, whitespace |
| `date` | `"2026-11-10"` for years 0000-9999; outside them the year is a sign and at least five digits, `"+10000-01-01"`, `"-00001-12-31"` (ISO 8601 expanded, proleptic Gregorian, year 0 = 1 BC) | — | `"+2026-11-10"`, `"+010000-01-01"`, `"2026-1-1"`; days outside 7.1's domain |
| `time` | `"13:45:07.123456"`, six fractional digits | — | `"13:45:07"`, leap second `60` |
| `timestamp` | `"2026-11-10T13:45:07.123456Z"`, the date part as for `date`, uppercase `T` and `Z`, six digits | — | Offsets, lowercase, fewer digits |
| `duration` | Decimal string of microseconds (an `i64`) | — | ISO 8601 `"PT1M30S"` |
| `uuid` | Lowercase RFC 9562 text in RFC byte order, also for columns ordered as SQL Server (an ordering property of the key, doc 3) | — | Uppercase, braces, no hyphens |
| `ulid` | 26 uppercase Crockford Base32 characters, first at most `7` | — | Lowercase, `I`, `L`, `O`, `U` |
| `vector(e, d)` | Base64 of doc 1's layout without the norm: `f32`/`f16` little-endian, `i8` bytes then the `f32` scale, `bit` packed | — | Number arrays (doc 9's D5 closed); a wrong length; NaN, infinities, a zero vector for cosine (doc 5) |
| `list` (for `in`) | Array of element values | — | Nested lists; `null` elements |
| null | `null` in `args`, for any declared type | — | — |

Evidence for the float rows, from `serde_json` 1.0.154's `de.rs` (fetched): with `float_roundtrip`, an `f32` is parsed
directly as `f32` (`parse_concise_float::<f32>`, `parse_truncated_float::<f32>`), not through `f64`, so no double
rounding; an infinite result is the error `NumberOutOfRange`; a negative zero integer becomes `F64(-0.0)` (lines
517-521), which an integer field refuses as a type error. Without `float_roundtrip`, these exactness claims do not
hold.

**Dates (C4).** Doc 1 stores `date` as `i32` days since 1970-01-01 and `timestamp` as `i64` microseconds ("Range
+-292,000 years"); 7.1 fixes their domains (`tables.prd.md:91`). The form above covers any domain 7.1 fixes, with one
spelling per value, and is RFC 3339 for years 0000-9999. Restricting the domain to 0001-9999 (the first draft's P7)
is an option for 7.1, not for this contract; the comparison is in [Review round 1](#review-round-1-2026-11-10), C4.

**Relation to I-JSON (RFC 7493).** Krummi documents follow its MUSTs (UTF-8, no duplicate names, no lone surrogates)
and deviate from four points: (a) unknown members are errors, the "Must-Understand" policy §4.2 admits "has proven
necessary in certain protocol designs"; (b) bytes use the standard alphabet, not §4.4's recommended base64url,
because .NET 8's `Convert` and `Utf8JsonWriter` write the standard one (from knowledge) and JSON is not a URL; (c)
durations are microsecond strings, not §4.3's ISO 8601 durations, which spell one value several ways and carry
calendar units Krummi lacks (doc 1); (d) strings may contain Unicode noncharacters, which §2.1 forbids, because
Krummi's `string` stores them and every stored value must be representable. Years outside 0000-9999 are not RFC 3339
either; a strict I-JSON or RFC 3339 receiver may reject (d) and such dates.

### 2.5 Validation, safety and limits

Krummi's validator is the authority; the JSON Schema of each kind (draft 2020-12) is published for generation and
documentation, and cannot express the schema-dependent rules (column exists, types match, ranges sorted). Phases, for
both kinds, the first failure returning the error:

| Phase | Checks | When |
|---|---|---|
| 0. Syntax | Size, UTF-8, JSON grammar, nesting, duplicate members, lone surrogates | Prepare |
| 1. Structure | Kind and version, known members and ops, arity, depth, node count, declarations, value grammar of §2.4 | Prepare |
| 2. Binding | Table, columns, indexes in the catalog the transaction sees at the call; identities, names and `schema_state` agree (plans, `bind`, P16) | Prepare; again when the executing catalog state differs |
| 3. Types | Bottom-up output schemas, operand and parameter types, vector dimensions (doc 8) | With binding |
| 4. Invariants | Queries: normalization (doc 4). Plans: doc 8's operator invariants and derived properties | With binding |
| 5. Admission | Ranges, `k`, `in` sizes, optional cost cap: what depends on values | Each run |

**Limits per phase** (corrected 2026-11-10 after review round 1, C5). Kinds: **ceiling**, a bound of the contract or
of an implementation that no setting raises; **budget**, configurable by the application per store or per call;
**default**, the safe value of a budget.

| Phase | What grows | Limit | Kind | Checked before |
|---|---|---|---|---|
| Parse | Bytes of a `query/1`, `plan/1` or `execution/1` | 1 MiB | Budget (doc 4's default) | Reading the input: its length is known |
| Parse | JSON nesting | 128 | Ceiling (`serde_json`'s `remaining_depth: 128`) | Each nested value |
| Parse | Memory of the typed tree | Proportional to the bytes | Follows from the size budget | — |
| Prepare | Node depth, nodes, declared parameters | 64, 4,096, 1,024 | Budgets (defaults) | The structure phase, while decoding |
| Prepare | Names | B4, 7.1's column-name limit | Ceilings of the catalog | Binding |
| Prepare, execute | Planning work | At most 65,536 estimate operations per planning, each a histogram lookup or a boundary descent, each `in` element one (below; a proposal) | Budget (default) | Each estimate |
| Prepare | Compiled plans in the cache | 256 entries, LRU (doc 4) | Budget | Insertion |
| Bind | `string`, `bytes` | Their column's maximum (7.1) | Ceiling of the catalog | Copying: the length comes with the C call |
| Bind | Vector | Exactly the declared element and dimension | Contract | Copying |
| Bind | `in` list | 10,000 elements | Budget (doc 8, line 148: "default 10,000, like an `IN` list") | Allocating: the count comes with the call |
| Bind | Base64 in `execution/1` | Decoded length computed from the string length and compared first | Contract | Decoding |
| Execute | Ranges | 10,000 | Budget (doc 8) | Admission |
| Execute | `top_k` `k` | 10,000 | Ceiling (README, vectors entry: "`k` up to 10,000") | Admission |
| Execute | Memory, spill, rows examined, deadline, cancellation | 64 MiB, 1 GiB, none, none | Budgets per call (doc 8, README *Plans*) | Each reservation, batch, 1,024 rows |
| Execute | Concurrent executions | Each has its own budget; no store-wide cap (README *Plans*); the total is the application's (below) | Decided | Documented: they add up |

**Hard maxima of budgets (P9, approved 2026-11-10).** A budget that guards memory or work (the document size, depth,
nodes, parameters, planning work, cache entries, `in` lists, ranges, memory and spill) also has a hard maximum, and a
setting above it fails with a typed error naming the budget and its maximum, instead of being clamped; no such budget
has a "none" setting. Rows examined and the deadline guard time, not memory, so "none" stays their default. Module
17's PRD fixes the maxima.

Nothing allocates in proportion to an untrusted size before the check of its row: lengths and counts arrive with
the C calls and the JSON string lengths before any copy or decode. Statements and argument sets are application
objects with no count limit, like transactions (T7).

**The planner's search (corrected 2026-11-10 after review round 2, C12).** V1 queries read one table (doc 4, README),
so the search space is small and is bounded without a general optimizer. With n leaf predicates (n ≤ the node budget)
and m indexes on the table (7.2's limit per table), doc 4's steps cost:

- Access paths: per leaf predicate, at most m + 2 candidates (each index whose leading columns it constrains, the
  primary key, the full scan): at most n(m + 2) candidates, each estimated once (a histogram lookup or two boundary
  descents; an `in` list one per element, within its bind budget).
- `and`: the cheapest single path plus residual filters, or the intersection of two paths taken only among the four
  cheapest (at most six pairs).
- `or`: a union of the best path of each child, chosen bottom-up, or a residual filter; trees are never rewritten into
  disjunctive or conjunctive normal form, so nothing grows exponentially.
- Order: at most m + 1 alternatives that provide the order (each index, the primary key) against an explicit sort or
  top-k.

The work is linear in the n(m + 2) candidates, times the elements of their `in` lists, plus a constant, and the budget
of estimate operations caps it (it counts work, not candidates, because one `in` candidate may cost 10,000 lookups;
corrected after the self-review of round 2): past it the planner keeps the best plan found so far, which is still a
correct plan, and explain's diagnostics say `"search_truncated": true`. Joins and grouping (later) will need a real enumeration budget; their PRD fixes it.

**Who owns what under concurrency (C12).** Krummi guarantees each execution's own budgets (memory, spill, rows,
deadline, cancellation), a bounded plan cache, and that executions sharing a statement share nothing mutable but the
cache and its heuristic counters (§2.2). The total memory of simultaneous executions is the sum of their budgets, and
the application, which decides how many run at once, owns it, as the README decided ("no store-wide cap in the first
version"). Doc 10's tools are applications here: the CLI runs one execution per command, and the MCP server (phase 2)
runs one tool call at a time with its lower defaults (doc 10: deadline 10 s, 10 million rows examined, 100 rows of
output).

**Ambiguous input is rejected**: duplicate members, unknown members and ops, a value in a form other than the accepted
ones of §2.4, `args` of the wrong length, an unbound parameter at run time.

**Errors.** One object, the same in the Rust error data, the C detail (`krummi_last_error_detail`, UTF-8 JSON, next
to the status code of doc 6) and the tools: `class`, `code`, `path` (an RFC 6901 JSON Pointer into the submitted
document, for example `/root/input/predicate/args/1/col`), `expected`, `found`, `message` (English in the library,
Spanish in the CLI, D8). A document that is not well-formed JSON has no path; its error carries `line` and `column`.
Execution errors (resource exhausted, `PlanInvariantViolated`) carry the path of the operator. This replaces doc 6's
byte offset and doc 8's node index.

**Full validation of plans.** Every submitted plan passes phases 0-5; there is no trusted shortcut. The planner's own
plans skip revalidation in release builds, and in debug and test builds every plan the planner emits goes through the
same validator.

**Cache key.** §2.2: the hash is a lookup aid; a hit compares the stored normalized tree with the new one, because a
collision, accidental or crafted, would otherwise run another query's plan: a wrong result, not a cache miss. Values
are never part of the key, so §2.4's accepted forms cannot split or merge entries.

**Differential tests** (a structurally valid plan need not express a query's intent; Krummi never claims it does):

1. For generated queries over generated schemas and rows (the property tests of AC-N1), the planner's plan and every
   alternative within its margin return results that are admissible for the query, by document 10's rule (C7),
   against the reference plan (`table_scan`, `filter`, `sort` or `top_k`, `project` or `aggregate`) on one snapshot.
   A generated query that wants equality gives itself a total order by adding the primary key to its sort keys.
   Corrected 2026-11-10 after the Codex review (R8): the first version required the same rows with ties by primary
   key, which a correct `sort` and `limit` plan over tied rows does not give.
2. Round trip: emit, parse, compare typed trees; emit again, compare bytes; run `explain.plan` as a plan statement in
   the same transaction and compare rows with the logical statement.
3. Mutated plans (dropped ranges, swapped operators, keys removed from a `key_lookup` input) either fail validation
   with a typed error or return rows the reference comparison flags; fuzzing of phases 0-5 finds no panic.
4. For explicit plans written by applications or agents, the guarantee is safe execution of the plan's own semantics;
   equivalence with a logical query is checked by comparison (doc 10, C7 and C8), not proven.

### 2.6 Dependencies

Corrected 2026-11-10 after review round 1 (C8): nothing here treats the exception as granted.

**Where the rule stands.** The root PRD's D12 names the Pager as the only part that may use `unsafe`
(`krummi.prd.md:52`) and has two annotations about specific uses (lines 91, 93); the 2026-11-06 annotation shows that
an `unsafe` mode of a dependency (`lz4_flex`) needed the user's decision. The general rule, "widely used and
maintained, pure Rust, sin una alternativa segura razonable, annotated with its reason in the PRD that introduces it",
is recorded in the README (*Dependencies with `unsafe` inside*, "Sí") and proposed as an annotation to D12 in
`tables.prd.md:265-268`, not yet applied. So two approvals: write the rule into D12, then annotate the `serde` set
under it in D12 and in module 17's PRD (P11).

**Facts**, fetched 2026-11-10:

| Crate | Version | Downloads (crates.io) | Lines mentioning `unsafe` (HEAD, tests excluded) | Note |
|---|---|---|---|---|
| `serde_json` | 1.0.154 | 1.58 billion | `read.rs` 3, `ser.rs` 5, `value/mod.rs` 1; `raw.rs` 7 behind the `raw_value` feature | Doc 9 counted only `read.rs` |
| `serde` / `serde_core` | 1.0.229 | 1.68 billion | 2 in the repository | `serde_derive` runs at compile time |
| `itoa` | 1.0.19 | 1.62 billion | 16 | Integer formatting |
| `memchr` | 2.8.3 | 1.71 billion | 349 (SIMD) | No build script, pure Rust |
| `zmij` | 1.0.23 | 664 million | 72 | Float formatting; "Pure Rust implementation", a port of a C++ original |

**The real risks compared.**

| Risk | Own strict reader in safe Rust | `serde_json` and its crates |
|---|---|---|
| Memory safety | None from the parser (D12 holds) | `unsafe` in five crates; a defect there is undefined behavior on untrusted input |
| Logic defects on untrusted input (accepting what it should refuse, wrong numbers) | New code at a security boundary; Krummi carries the fuzzing and the review alone | Mature code used by most of the Rust ecosystem; Krummi still fuzzes its own typed layer |
| Float exactness | `str::parse` is correctly rounded (from knowledge); shortest output through `Display` (from knowledge); both to test (B4) | `float_roundtrip` (fetched, §2.4); B4 |
| Effort | Estimated by doc 9 at the size of the binary reader, plus a writer; every rule of §2.4 by hand | Typed structures by derive; the rules of §2.4 still in custom deserializers |
| Supply chain and churn | None | Five crates to follow; a CI step pins the list (below) |

Pure Rust, widely used and maintained: met. "Without a reasonable safe alternative": met only if the user weighs
the logic-defect risk of new parser code above the memory-safety risk of mature `unsafe` code, which is the
reviewer's stated preference ("Mi preferencia sigue siendo utilizar `serde_json`"). If approved:

- Only the query crate of module 17 depends on them; the engine crates do not. AC21's check still covers Krummi's code;
  a CI step compares the query crate's normal dependency tree with the annotated list, so a new transitive crate
  fails the build.
- `serde_json` with `default-features = false, features = ["std", "float_roundtrip"]`: not `arbitrary_precision`,
  `raw_value` (keeps `raw.rs` out) or `unbounded_depth` (keeps the 128 ceiling).
- Typed structures with `#[serde(deny_unknown_fields)]`; duplicate members of a struct fail in serde's derived code
  (from knowledge; a golden test pins it, also inside `"op"`-tagged nodes, which serde buffers).
- The JSON Schemas are hand-written files versioned with the kinds, checked in CI against the golden documents with a
  JSON Schema validator as a development dependency only.

### 2.7 AI integration

The core offers, as library functions with no MCP or AI dependency: the JSON Schema of each kind, embedded in the
library (`krummi_document_schema(kind)` in C) so a tool gets the schema of the exact version it links; a schema
description of the store (tables, columns, types, primary keys, indexes, identities, state tokens, schema versions, P12); `explain/1`; the
error object of §2.5; canonical output for golden files. Doc 10's CLI, skill and later MCP server parse a request, call
one of these functions and print the response; MCP's `inputSchema` and `outputSchema` are the published schemas. A CI
check keeps `rmcp`, `tokio` and any AI SDK out of the engine and query crates' dependency trees.

### 2.8 The random source of 7.1 (P20)

Added 2026-11-10 with the decision table. 7.1 needs random bits for its generators (README, generators entry: `ulid`
with 80 random bits, `uuid` version 7) and, with this contract, for the catalog identities and tokens of §2.2. Rust's
standard library has no public API for the operating system's generator, so the bits come through a system call,
which needs `unsafe` somewhere. The user asked not to weaken D12 and not to write a cryptographic generator of
Krummi's own. Facts checked in the repository and the local crate sources on 2026-11-10:

- `crates/krummi-pager/Cargo.toml` already depends on `rustix` 1.1.5 (unix; features `fs`, `mm`), `windows-sys`
  0.61.2 (windows; features `Win32_Storage_FileSystem`, `Win32_Foundation`, `Win32_System_Memory`), `memmap2`,
  `lz4_flex`, `zerocopy` and `twox-hash`. The pager is the only crate D12 lets use `unsafe` (`krummi.prd.md:52`; the
  crate's description says so), and its operating-system calls live in `crates/krummi-pager/src/sys.rs`.
- `rustix` 1.1.5 has a `rand` feature (`rand = []` in its manifest); `src/rand/getrandom.rs` wraps Linux's
  `getrandom` system call with one internal `unsafe` block, only on Linux kernels (`#[cfg(linux_kernel)]`), which
  covers D3's Linux.
- `windows-sys` 0.61.2 declares `BCryptGenRandom` (`bcrypt.dll`) and `ProcessPrng` (`bcryptprimitives.dll`) behind the
  feature `Win32_Security_Cryptography`; the declarations are `link!` items, with no code of their own.
- `getrandom` 0.4.3 is already in `Cargo.lock`, but only through development and bench dependencies (`tempfile`,
  `canopydb`, `fjall`); the library's normal tree (`cargo tree -p krummi -e normal`) does not contain it. It depends on
  `cfg-if` and `rand_core` 0.10, and on `libc` on Linux with glibc (already in the library tree through `memmap2` and
  `rustix`); on Windows it calls `ProcessPrng`; lines mentioning `unsafe` in its sources: `util.rs` 12,
  `backends/linux_android_with_fallback.rs` 8, `lib.rs` 3, `backends/windows.rs` 2.

| | A. A pager function (recommended) | B. The `getrandom` crate |
|---|---|---|
| What | `krummi_pager::fill_random(&mut [u8]) -> io::Result<()>`, safe, in `sys.rs`: `rustix::rand::getrandom` on Linux, `BCryptGenRandom` with the system-preferred generator on Windows (or `ProcessPrng`, which `getrandom` uses) | `getrandom::fill` called from 7.1's crate |
| New crates | None: one feature on each of two existing dependencies | `getrandom`, `cfg-if`, `rand_core` in the library's tree |
| `unsafe` | The pager's own call on Windows (D12 allows it) and rustix's existing kind of wrapper on Linux | Inside a new dependency of a crate D12 forbids `unsafe` in |
| D12 | No exception: see below | Needs the dependency rule written into D12 (still only proposed, `tables.prd.md:265-268`) and an annotation for this crate, so 7.1 waits on that root PRD change |
| Code to keep | About 20 lines and a test per platform (estimate) | A call |
| Generator of Krummi's own | None: the bytes are the operating system's | None |
| Behavior on failure | The error goes up; the schema change or generator call fails, nothing weaker is drawn (§2.2) | Same |

Considered and dropped: reading `/dev/urandom` with `std::fs` (safe and enough on Linux, but Windows still needs a
system call, so it does not remove A's Windows part); seeding a generator from `std`'s `RandomState` (its randomness
is an implementation detail of hash maps, not a documented source); any generator written in Krummi (the user
excluded it).

**Does A need a D12 annotation?** No change to D12's rule. D12 governs Krummi's own code and lets the pager use
`unsafe`; the call that reaches the operating system is the pager's own, of the same kind as the file and memory calls
already in `sys.rs`. The `windows-sys` feature adds declarations only; the `rustix` feature adds a wrapper of the same
kind as the `fs` and `mm` features the pager already uses. The 2026-11-06 annotation for `lz4_flex` was different: the
PRD had chosen its safe encoder, and switching to the `unsafe` one traded a safe alternative for speed, which needed
the user's word; reading the system generator has no safe alternative in `std`. Recommended: approve P20 as the
product decision, and record in 7.1's PRD where the generators' randomness comes from; a D12 annotation is optional,
only for uniformity with 2026-11-06.

P20 and P11 stay separate: P20 is for 7.1, now, and needs no change to D12; P11 is for modules 17 and 18, later, and
needs the dependency rule in D12 first.

**Decided (2026-11-10): A, with the user's sequential algorithm, and no pseudo-random generator.** The user chose A
and first asked to keep "mi algoritmo, que es más rápido, usa el random del sistema operativo pero luego usa
XorShilf64" (a `XorShift64`, `x ^= x << 7; x ^= x >> 9`, seeded once from the operating system). Checked: that pair of
shifts has full period 2^64 - 1 (its matrix over GF(2) has that order; the control `13, 7, 17` gives the same). It was
not adopted, and the user agreed ("Sí"), for two reasons:

- Speed does not matter here: the sequential algorithm (README, generators entry) draws random bits only when the
  millisecond changes and increments within it, so at most one draw per millisecond and generator; catalog tokens are
  drawn only on schema and index changes.
- `Next()` returns the whole state, and the 64-bit low random part is one full output, so one generated value
  predicts every later one. RFC 9562 §6.9 asks for a cryptographically secure generator so that identifiers are hard
  to guess, and a library cannot know whether an application exposes its keys.

So each generator keeps the user's algorithm (48 bits of Unix milliseconds, 80 random bits incremented within a
millisecond; `uuid` version 7 its 74 random bits) and takes the random bits straight from `fill_random` at each new
millisecond; identities and tokens take theirs from it too. No pseudo-random generator sits between the operating
system and Krummi. 7.1's PRD records where the generators' randomness comes from.

## Review round 1 (2026-11-10)

The reviewer's findings on the first version, forwarded by the user. Each was checked against the documents before
it was accepted.

**C1, high: prepare against the latest commit, execution in older snapshots. Verdict: real, correctness defect.**
- Evidence: the first version prepared "against the latest committed catalog" (§2.2, Create); T2 makes a read see
  what was committed when it opened; doc 8 phase 2 requires index ids and names to exist.
- Counterexample (the reviewer's, confirmed): T1 opens with `ix_customer`; W drops it and commits; `explain` in T1
  plans with it; `prepare` of that plan against the latest catalog fails, against §2.3's round-trip promise.
- Fix: prepare in a transaction and bind against its snapshot; re-bind per executing catalog state (§2.2); bindings to
  uncommitted states stay inside their write transaction (P17, from the self-review below).
- Invariant: a document is always bound against the snapshot of the transaction that uses it.
- Acceptance: (a) the counterexample runs in T1 and returns the rows of the logical statement in T1; the same plan in
  a transaction opened after W fails with schema mismatch naming the index; (b) a statement prepared after a schema
  change runs in an older read transaction against that transaction's schema; (c) and (d), superseded by round 2
  (C13) and round 3 (C14): W creates an index, prepares a logical statement and takes the plan from its `explain`,
  which uses the index; after a commit both work in later transactions; after an abort the plan fails with schema
  mismatch, the logical statement runs without the index, and no transaction runs a plan with that index; (e) a stress
  test of threads
  preparing and running while other threads commit schema changes: every result equals the reference plan on its own
  snapshot.
- Changed: Summary 2, §1 P3 and P17, §2.2 (catalog state, lifecycle, execution, uncommitted states), §2.3 rule 3, §3.

**C2, high: concurrent reuse of a shared statement. Verdict: partly valid.** The first version already keyed the cache
by schema and index-set version, so a re-plan for T2 did not reach T1's key; but it never stated that compiled plans
are immutable, what they may hold, or what a re-plan for the same key does to a running cursor.
- Counterexample to rule out: T1 (old catalog) and T2 (new catalog) run one statement on two threads; T2's re-plan
  overwrites "the statement's plan" and T1's next batch reads an index T1 cannot see.
- Fix: §2.2, *Compiled plans*: immutable, reference-counted entries per catalog state; no page, tree or value inside;
  same-key replacement affects later lookups only; counters are atomics.
- Invariant: a cursor runs one compiled plan from creation to end, and that plan's catalog state equals its
  transaction's.
- Acceptance: interleaved runs of one statement in T1 and T2 on two threads always use the entry of each one's catalog
  state (checked through a test hook and `explain`); a statistics rebuild during a run leaves the running cursor's
  plan unchanged; results equal the reference on each snapshot.
- Changed: §2.2, §3 (doc 4, *Plan caching*).

**C3, medium: canonical values. Verdict: valid; the first version overstated it.** It said "Output equals the accepted
input form" while accepting several float spellings and normalizing `-0` and NaN.
- Counterexample: input `1.0` for an `f64` is re-emitted as `1`; `-0` as `0`.
- Fix: §2.4's three levels and the stated guarantee. Checked: `u64` max and max + 1, `i64` bounds, decimal scale,
  `-0`, NaN, infinities, subnormals, `f32` rounding (serde_json source, fetched).
- Invariant: one typed value per accepted document; byte-identical re-emission of Krummi's own output only.
- Acceptance: golden cases for every row of §2.4's table (each accepted and each rejected example); B4 of doc 9 over
  every `f32` bit pattern and 10^8 random `f64` (emit, parse, same bits, NaN as the canonical one); random decimal
  strings parsed as `f32` equal `str::parse::<f32>`.
- Changed: Summary 5, §2.3 rule 4, §2.4.

**C4, high: the domain of dates. Verdict: valid.** P7 restricted 7.1's domain for a serialization convenience. Doc 1
stores `date` as `i32` days and `timestamp` as `i64` microseconds; `tables.prd.md:91` leaves the domains to 7.1.

| | A. Whole domain, one text form (recommended) | B. Domain 0001-9999 in 7.1 |
|---|---|---|
| Values | Every value 7.1 admits has a text form | Values outside are refused on write |
| Text | RFC 3339 inside 0000-9999; signed expanded years outside, one spelling | RFC 3339 only |
| Interoperability | A strict RFC 3339 reader rejects dates outside 0000-9999; .NET `DateOnly` and `DateTime` cannot hold them, so the C# binding needs a raw accessor or an error for them (module 18) | Every value fits .NET and RFC 3339 |
| Decision | None needed here; 7.1 fixes its domain as planned | A product decision of 7.1's PRD |

- Fix: §2.4's `date` row, with P7 reworded; no domain changes.
- Acceptance: golden cases for 0000-01-01, 9999-12-31, `+10000-01-01`, `-00001-12-31` and the bounds 7.1 fixes, in
  both directions; each non-canonical spelling rejected.
- Changed: Summary 5, §1 P7, §2.4.

**C5, medium: limits and admission. Verdict: partly valid.** Document and run-time limits existed; prepare, planning
and binding of values outside JSON had none stated, and the first version made the document limits fixed maxima,
against doc 4's question 9 ("all configurable").
- Counterexample: a C caller binds a 2 GiB `bytes` value or a list of 10^9 elements; nothing in the first version said
  it is refused before the copy.
- Fix: §2.5's table per phase, each limit marked ceiling, budget or default, and the rule that lengths and counts are
  checked before allocation.
- Acceptance: each limit has a case that exceeds it by one and fails with its class and path before allocating (a
  test allocator counts bytes); concurrent runs of one statement each respect their own budgets.
- Changed: Summary 6, §1 P9, §2.5.

**C6, high: re-binding stored logical queries by name. Verdict: valid.** N3: "Una columna añadida con el nombre de
una que se quitó es nueva". Doc 1 gives columns a stable `column_id` (line 220).
- Counterexample (confirmed): `status` dropped, a new `status` of the same type added; a stored query, or a statement
  prepared before, binds to the new column and returns other rows without error.
- Fix: P16: a statement pins the column ids it bound; a `query` document may carry `bind`, always emitted by Krummi.
  Alternatives in §1.
- Invariant: once bound, a name never silently denotes another column.
- Acceptance: drop and re-add `status`: the statement fails with schema mismatch naming `status` and both ids; the
  stored query with `bind` fails the same way; without `bind` it binds to the new column (documented); a rename makes
  both fail; an unrelated added column changes neither.
- Changed: §1 P16, §2.1, §2.2, §2.5, §4.

**C7, medium: product decisions introduced indirectly. Verdict: valid.** One-shot calls, parameters for constants,
concurrency on a statement, the date domain, fixed maxima and the re-planning policy were presented as recommended
defaults. Fix: §1's revised table with F/N and alternatives (P7, P9, P14, P15, P18; P4 split into what follows and
the design heuristics).

**C8, D12. Verdict: valid.** The first version's P11 asked the user to admit the crates "under the dependency rule",
which the root PRD does not hold yet. Fix: §2.6, two approvals and a comparison of risks; P11 reworded.

### Self-review of the corrections

1. **New problem: schema versions are positions, so they are reused after an aborted change** (C1 with C2). Doc 1
   (`01-row-format-and-types.md:170`) makes the schema version an index into the table's schema history; a write
   transaction that adds a version and aborts leaves that index free for the next change. A plan cached under (table
   id, version 6) from the aborted state would be found by a later committed version 6 with other columns. Fixed by
   P17: uncommitted states are never cached, never emitted, and their statements never leave the transaction. The
   alternative, a store-wide catalog counter that no abort rolls back, is heavier (a counter outside transactions) and
   still reusable after a crash. *Superseded by round 2 (C9, C13): random identity tokens replace both; the round-1
   restrictions of P17 are withdrawn.*
2. **Residual: non-durable modes.** G11's modes may lose recent commits in a crash; a plan document an application
   stored from a lost commit could later meet a recommitted schema version with other columns. Column ids (P16) catch
   it when they differ; when ids are reused too, nothing does. Proposed: document it for G11 modes; not fixed.
   *Superseded by round 2 (C9): closed by the tokens.*
3. **Requirement on 7.1 and 7.2 found while fixing C6:** table, index and column ids must never be reused among
   committed states (doc 1's append-only history gives it for columns; tables and indexes need the same rule).
   *Superseded by round 2: public identities are random; internal numbers fail at their maximum (P19).*
4. **C6 with C1:** a statement prepared in a newer snapshot and run in an older one where the name resolves to another
   id fails with schema mismatch: correct, since the column differs; noted in the acceptance of C6.
5. **C5 with shared statements:** after a statistics rebuild, every thread running a statement may re-plan the same
   key at once; results stay correct and the cost is bounded by the number of threads. A single planning per key is
   an optimization to measure, not to build now.
6. **C3 with the cache key:** values are not in the key and `in` deduplication runs after normalization, so `-0` and
   `0` in one list are one probe. No other interaction found.

## Review round 2 (2026-11-10)

The reviewer's second round, favorable, with findings C9-C13 and comments on the classification of P1-P18, forwarded
by the user. Each finding was first checked against the existing guarantees to try to refute it. One mechanism,
random catalog identities written in the commit that creates them (§2.2), proposed by the coordinator as an idea to
test, closes C9, C10's identity question and C13 together; it was checked against the counterexamples below before
it was adopted.

**C9, high: identities reused after a lost non-durable commit. Verdict: real, correctness defect** (the first round
had recorded it as a documented residual, which does not keep the promise of §2.3).
- Refutation attempted: G11's modes may lose recent commits; the round-1 rules keyed documents on the schema version,
  a position in the schema history (`01-row-format-and-types.md:170`), and on ids allocated from committed state, so
  both can be drawn again after the loss. No existing guarantee prevents it.
- Counterexample (the reviewer's, confirmed): commit a schema change without durability; export a `plan/1` or a
  `query/1` with `bind`; crash and lose the commit; make another change that takes the same version and ids; the old
  document validates with another meaning.
- Fix: random 64-bit identities for tables, columns and indexes, and random `schema_state` and `index_state` tokens per
  table, written inside the commit of the change (§2.2). The change lost in the crash took its tokens with it; the next
  change draws new ones. No extra durable write, so G11 is unchanged; the guarantee is the same in every durability
  mode, so no weaker product guarantee is needed for G11.
- Invariant: a document validates only against the catalog state it was made for, up to 2^-64 per comparison.
- Acceptance: in a non-durable mode with a crash hook, commit a schema change, take `explain` and a normalized query
  with `bind`, crash so the commit is lost, reopen, make another schema change that reuses the readable schema version:
  both documents fail with schema mismatch naming the table, both versions and both tokens; a restored backup and its
  source each make a schema change: documents from either after the change fail on the other, documents from before
  work on both.
- Changed: Summary 3, §2.1, §2.2 (identities, invalidation table), §2.3 rule 3, §3.

**C10, medium: exhaustion of versions and counters. Verdict: valid.** Doc 1's schema version is a `u16`; doc 4's
index-set version and doc 1's `column_id` had no stated maximum behavior.
- Counterexample: a table that reaches 65,535 schema versions; a wrapped or reused number would let an old plan
  validate.
- Fix: no counter identifies a catalog state any more (the tokens do, and have no maximum); the index-set version is
  replaced by `index_state`; every internal counter fails without effect with a typed error when the next change would
  exceed it, never wraps (§2.2, *Exhaustion*; P19).
- Invariant: no identity is silently reused while a document could validate with another meaning.
- Acceptance: with a test hook that lowers the schema version maximum, the change past it fails without effect with
  7.1's class, the table keeps working, and every earlier document still validates or fails as before.
- Changed: §1 P19, §2.2, §3 (7.1).

**C11, high: schema changes inside one write transaction. Verdict: partly valid.** The round-1 execution rule
compared catalog states at each run, but §2.2 claimed "A snapshot is immutable (G3, T2)", which T2 contradicts for a
write transaction ("más sus propios cambios"), and the behavior of a statement across its own write transaction's
changes, indexes and cursors was unstated. T6 keeps a write transaction on one thread, so there is no concurrency
inside it, only sequence.
- Counterexample: W prepares S on `t`, renames a column of `t`, runs S: under "immutable snapshot" S would run against
  the catalog at W's start and read a column under its old name.
- Fix: every call reads the catalog the transaction sees at that moment; each change in W draws new tokens, so S
  re-binds; a cursor never survives a change of its own write transaction (§2.2, *Catalog state*, *Write
  transactions*).
- Invariant: an execution validates against the catalog visible at the call, including the transaction's own
  uncommitted changes.
- Acceptance: the reviewer's sequence, for each change (rename, drop and add a column, create and drop an index used
  by the plan): open W, prepare S on `t` (a logical query and the plan from its `explain`), make the change in W, run
  S in W. Rename and drop fail with schema mismatch for both; an added column runs; a created index runs and the
  logical statement may use it (`explain` shows it); a dropped index re-plans the logical statement and fails the plan
  statement. Then commit and run S in a new read transaction, which follows what that transaction sees (§2.2); and
  the abort variant, where S runs as on the committed state it was prepared on (corrected after review round 3, C14:
  the first wording made S fail after any abort). A C cursor's next call after a change in its write transaction
  returns misuse; a Rust compile-fail test pins the borrow.
- Changed: Summary 2, §2.2, §2.5 (phase 2).

**C12, medium: planning and concurrency limits. Verdict: partly valid.** The search space of V1 is bounded by
construction (one table, doc 4's steps), but no document said so or capped it, and the owner of the total memory of
concurrent executions was not named.
- Counterexample: a predicate tree of 4,096 nodes over a table with many indexes; rewriting `or` into normal form, or
  pairing every path for intersections, would grow far beyond the node count.
- Fix: §2.5, *The planner's search*: per-step bounds, no normal-form rewriting, intersections among the four cheapest
  paths, and a planning budget whose excess keeps the best plan found and says so in explain; *Who owns what under
  concurrency*: Krummi owns per-execution budgets and the bounded cache; the application owns how many run at once;
  doc 10's tools run one at a time with lower defaults.
- Invariant: planning work is bounded by a budget; executions sharing a statement share nothing mutable except the
  cache and its heuristic counters.
- Acceptance: a query at the node budget over a table at 7.2's index limit plans within the budget and returns the
  reference rows; with the budget lowered, explain says `"search_truncated": true` and the rows still equal the
  reference; 16 threads run one statement with distinct values at once and each cursor returns the reference rows for
  its own values; changing an argument set after `execute` leaves the open cursor's rows unchanged; one execution that
  exhausts its memory budget fails alone, the others finish.
- Changed: Summary 7, §2.5.

**C13, product decision: the restrictions of P17. Verdict: valid; they are not needed for correctness.** They only
protected the shared cache and documents from reused numbers; with random tokens, a plan or statement made in W is
valid wherever the identities and tokens it pins are visible and fails safely where they are not (the first wording,
"valid after commit, fails after abort", was too broad; corrected after review round 3, C14). Fix: P17 rewritten as the behavior to confirm, with no restriction on write transactions; the agent workflow
the reviewer describes (create a table and indexes, prepare, inspect plans, commit) works in one write transaction.
Acceptance: the C11 cases with commit and abort. Changed: §1 P17, §2.2, Review round 1 (C1's acceptance (c) and (d)
superseded).

**Classification of P1-P18.** Applied to §1: P4 is partly derived (revalidation follows; the policy needs approval);
P6 is partly derived (the open part is the promise for stored queries); P12 is partly derived (plans must name what
they bind, but publishing identities through introspection, and their random form, need approval); P17 rewritten
after C13; P14, P15 and P18 stay new. P19 is added for C10.

**Conclusion.** No known correctness defect remains in this contract. The accepted residual risk is a token collision,
2^-64 per comparison of a stale document with a catalog state, which Krummi states. The decisions that need the
user are those marked "Yes" in §1's table.

### Self-review of round 2

1. **New problem: the planning budget counted candidates, not work.** An `in` list estimates one value per element
   (doc 4, step 3), so 1,024 candidates could still mean millions of histogram lookups. Fixed in §2.5: the budget
   counts estimate operations, each `in` element one.
2. **New requirement: identities unique within a store, and no predictable fallback.** Two objects of one store drawn
   with the same identity would make lookups by identity ambiguous, and a token source that falls back to a
   predictable value would bring C9 back. Fixed in §2.2: a drawn identity or token that already exists in the store's
   catalog, or in the table's schema history, is drawn again; if the random source fails, the schema change fails with
   an error and draws nothing weaker.
3. **Checked, no problem: MVCC.** A read transaction sees only committed tokens, so a document that pins a state W
   created fails in other transactions until W commits, and works in those opened after; this is T2's visibility,
   applied to the catalog. (A document that pins only committed identities works everywhere they are visible; round 3,
   C14.)
4. **Checked, no problem: copies.** Backups and compaction copy tokens; a copy and its source agree until one of them
   changes, and documents made before the divergence mean the same on both, because tokens name catalog states, not
   data.
5. **Minor: cache entries of aborted write transactions** stay until LRU evicts them; a write transaction that makes
   many changes and prepares after each may push useful entries out. Cost only.
6. **Minor: error texts.** After a lost commit two different states may show the same readable schema version, so the
   schema mismatch error also carries both tokens (§2.2's invalidation table).

## Review round 3 (2026-11-10)

The reviewer approved the random identities and state tokens, considered C1-C13 resolved, and found one
contradiction, forwarded by the user.

**C14: validity after commit or abort. Verdict: valid, correctness defect in the wording of P17.** Round 2 said that
what is prepared in a write transaction stays valid after its commit and fails after its abort, which ties validity
to the preparing transaction's outcome instead of to the catalog.
- Counterexample 1 (confirmed): W prepares a query on a committed schema, changes only rows and aborts; the schema did
  not change, so the query must stay valid, but the round-2 rule made it fail.
- Counterexample 2 (confirmed): W prepares a plan, drops one of its indexes and commits; the plan must become invalid,
  but the round-2 rule made it valid "after the commit".
- Fix: the reviewer's rule, written into P17, §2.2 (*Write transactions*), §2.3 rule 3, Summary 3, round 1's C1
  acceptance and round 2's C11, C13 and self-review: validity depends only on whether the identities and tokens a
  document or statement pins are in the catalog the executing transaction sees. A document bound only to a state that
  vanished with an abort is rejected; one bound to a committed state that still exists stays valid.
- Invariant: the outcome of the preparing transaction never decides validity by itself.
- Acceptance:
  1. W prepares a logical statement and takes a plan with `explain` on a committed schema, inserts and deletes rows,
     aborts: both run in a new read transaction and return its rows.
  2. W prepares a plan that uses `ix_a`, drops `ix_a`, commits: the plan fails with schema mismatch naming `ix_a` in W
     after the drop and in every transaction opened after the commit; a read transaction opened before the commit
     still runs it; the logical statement re-plans without `ix_a`.
  3. The abort variant of 2: the plan runs again in every transaction after the abort.
  4. W adds a column and takes a plan that reads it, then aborts: the plan fails with schema mismatch everywhere; a
     logical statement prepared in W before the change still runs.
- Changed: Summary 3, §1 P17, §2.2, §2.3, Review rounds 1 and 2 (as marked).

**Closing.** C1-C14 are closed. The acceptance cases of rounds 1-3 are pending, as tests of the modules that own
them: 7.1 and 7.2 (identities and tokens drawn at each change, exhaustion of counters), 17 (binding, validity,
compiled plans, values, limits, planner budget, round trip, differential tests) and 18 (C and C# lifecycle, threads,
cursors across changes, error detail). No further review round is open.

## 3. Changes, document by document

[P] needs the user's confirmation of the item named; [D] is design; [R] records a decision.

### Document 4 (13)

| Section | Now | Becomes | |
|---|---|---|---|
| Summary, lines 8-10 | Compact versioned binary; "tree bytes are the plan cache key" | JSON documents of doc 11; key from the normalized tree | P1 |
| Requirements, 5 (line 24) | DD9: readable dump that cannot be read back | The JSON document is the readable form and is input; explain text output only | P5 |
| §2 Serialization (54-73) | Custom binary chosen; MessagePack/CBOR "decode to a generic value tree" | Superseded by docs 9 and 11; the claim corrected (doc 9, claim 1) | D |
| Query tree sketch (107-123) | `Limit { limit: Param or u64 }`; `VectorWithin` | Parameters only (P15); `distance` operand shared with doc 8 | P15, D |
| Notes, line 128 | Column ids, names allowed "on the wire" | Names in documents, ids as checks (`bind`, P16); closes question 6 | P16 |
| Notes, line 134 | `Extension(tag, len, bytes)` reserved | Reserved op names, rejected as unknown; no opaque node | D |
| Wire format (136-149) | Bytes layout; "Shape key = hash of `table + nodes` bytes" | Pointer to doc 11 §2.1; key of §2.2 with equality check on hit (doc 9, claim 2) | D |
| Wire format, line 147 | One pass validates "column ids and types" | The phases of §2.5 (doc 9, claim 3) | D |
| Planner step 9 (169) | `IndexRangeScan`, `Union`, `Intersect`, `Fetch` | Doc 8's operators (doc 10 finding) | D |
| Plan caching (188-194) | Key with schema and index-set version | Entries per catalog state keyed by tokens (`schema_state`, `index_state`), immutable and shared, replacement rules (§2.2); the planner's search bounds (§2.5) | D |
| How C# reads rows, 1 (208-210) | No `Prepare` in the first C surface | Prepare, bind, execute from the first version | P2 |
| Costs and risks (230, 236) | Custom wire format row; "spill to disk deferred" | Contract drift row (schema, validator, emitter); spill per doc 8 (doc 10 finding) | D |
| Questions 4-7, 9 (252-271) | 4, 5, 6, 7 "still open"; 9 open | 4 decided JSON; 5 by P11; 6 by P16; 7 superseded by plans as input (README); 9 by P9 | R, P9 |

### Document 6 (10)

| Section | Now | Becomes | |
|---|---|---|---|
| Header and summary 1, 6 (lines 4-5, 14, 25-26) | Queries by serialized doc-4 binary tree, "one call executes it" | Prepared statements over JSON documents | P2 |
| R7 (43-44) | One call per tree, a byte buffer | Prepare in a transaction (UTF-8 JSON), bind, execute | P2, P3 |
| Handles and closing (105-123) | Store, transactions, cursors | Statements and argument sets hold the store's reference; close with live statements is misuse | P3 |
| Threads (128-135) | Read and write transactions, cursors | Plus statements (shareable) and argument sets (one thread); a cursor's next call after a change of its own write transaction returns misuse (round 2, C11) | P14, P17 |
| Status codes (148-150) | Pager and tree classes | Plus invalid document, schema mismatch, invalid value, duplicate key, resource exhausted, cancelled | P10 |
| C sketch (183-187) | `krummi_read_query`, `krummi_write_query` | §2.2's functions, explain, error detail, document schema; no one-shot (P18); writes by tree removed (out of scope, README) | D, P18 |
| C# builder (239-244) | Binary serializer; error "with the byte offset"; golden bytes | `Utf8JsonWriter`; error path (doc 9, claim 4); golden JSON shared with Rust | D |
| Testing (254-261) | Golden query bytes | Golden documents; prepare, bind, invalidation and thread tests (acceptance of C1, C2) | D |
| Risks (272) | Format change guarded by the version field | Document versioning of §2.1 and P6 | D |
| Question 8 (293-296) | "still open" | Decided (README, "Sí"; doc 10 finding) | R |

### Document 8 (11)

| Section | Now | Becomes | |
|---|---|---|---|
| Summary 1, 3 (9-11, 16) | Binary envelope; error "with a node index" | `plan/1`; error path | P1 |
| R1 (27-29) | "plans carry resource limits" | Limits travel with the execution (as line 151 says) | D |
| Candidates table (63) | "One forward pass, canonical" | Superseded note | D |
| Column naming (68-70, 166-169) | Plans name input ordinals | Names in documents, ids in `refs`, ordinals after binding | D |
| Reserved tags (107-110) | `Extension(tag, bytes)` | No opaque extension | D |
| Plan format sketch (112-128) | Bytes; `est` trailing field; shape key "the bytes before" the parameters; "planner's output is the same bytes" | `plan/1` with `refs`; no `est`; key of §2.2; same document | D |
| Schema version (124-125) | Equality of `schema_version` | Catalog state of §2.2 (random identities, `schema_state`), bound against the catalog the executing transaction sees; the readable version kept for messages | D, P12, P16 |
| Validation (130-149) | Magic and lengths; `{ code, node index, ... }`; phase 5 "Fills missing `est` fields" | Phases and limits of §2.5; error object; estimates computed for diagnostics and admission, never written into the plan | D, P9 |
| Explain (238-251) | Stats "indexed by pre-order node number" | `explain/1`, diagnostics by JSON Pointer; round trip of §2.3 | P5 |
| Costs and risks (257) | "A second wire format" | Removed; contract drift row | D |
| Product questions (267, 273) | "plan bytes" stored | Format changed 2026-11-10; stored plan documents per P6 | R, P6 |

### Document 9 (9)

| Section | Now | Becomes | |
|---|---|---|---|
| Summary 7 and decided-items table (38-55) | Open for the user | Decisions of this document, dated | R |
| Matrix and risks (107, 300) | "3 uses in `read.rs`" | §2.6's inventory | D |
| Type table (159-175) | "at most `s`" fractional digits; expanded years open (D4); Base64 "padded" without the I-JSON deviation; arrays maybe (D5) | §2.4's three levels and table | D |
| Where types live (177-183) | Value optional next to its declaration | Declarations without values; `args` | D |
| Example and plan text (235-258) | Values in `params`; explain fields "the validator ignores" | §2.1 examples; §2.3 rule | D |
| Rules (260-264) | I-JSON, unknown fields rejected; doc 4's limits | I-JSON with §2.4's deviations; §2.5 limits per phase | D, P9 |
| C sketch (270-281) | Binds on the statement; one-shot `krummi_read_query`; `krummi_prepare(store, ...)` | §2.2 | P3, P14, P18 |
| Libraries (283-285) | `serde_json` with `float_roundtrip` | §2.6, two approvals | P11 |
| Questions and details (322-341) | 1-7 open; D1-D5 open | 1, 2, 4, 7 decided by the forwarded message; 3 by P6 and open question 1; 5 to doc 10; 6 is P11; D1-D5 closed by §2.1 and §2.4 | R |

### Document 10 (5)

| Section | Now | Becomes | |
|---|---|---|---|
| C1 (65) | Schema version visibility open | Required, with ids (P12) | P12 |
| R3 and CLI errors (43-44, 182-184) | `node` index | `path` | D |
| What it needs from doc 9 (142-158) | Text form mapped to a binary envelope; tool prints names and ids | The JSON documents themselves; names, with ids as checks | D |
| Risks (324) | Text form drifts from the binary | Removed | D |
| Question 8 (348-349) | Open | Decided (P5) | R |

### README (7)

| Entry | Now | Becomes | |
|---|---|---|---|
| *Plans* (121-127) | "in the query tree's binary envelope", "plan bytes" | JSON `plan/1`, the earlier decision kept as history with its date | P1 |
| C interface (66-73) | "queries by serialized tree" | Prepared statements | P2 |
| Query tree (61-65) | "tags ... reserved in the format" | Op names reserved; P15's answer on constants | P15 |
| Dependencies (128-131) | Rule only | Plus the `serde` set, if both approvals of P11 are given | P11 |
| Line 244-245 | Docs 9 and 10 "no decision recorded yet" | Decisions recorded, with the forwarded words, including round 1 | R |
| New entries | — | One per confirmed P item, with the user's words | R |
| Documents table (258-270) | 10 documents | Adds 11 | D |

### PRDs

`krummi.prd.md` (6), annotations in Spanish written with the module 17 PRD; several are already proposed in
`tables.prd.md` (*Anotaciones a otros PRD*) and not yet applied:

| Place | Change | |
|---|---|---|
| Objetivos (18-25) | Objective 4, AI-assisted development, scoped to developers' agents | P13 |
| D2 (42) | Public surface adds the C interface and the C# binding (pending) and the versioned documents with their schemas | P1, P8 |
| D7 (47) | Query documents are versioned apart from the store format; stored plans per P6 | P6 |
| D12 (52) and AC21 (447) | First the dependency rule (pending in `tables.prd.md`); then the `serde` exception; `krummi-capi` | P11 |
| Reparto (215-238) | Modules 17, 18 and 19 (pending); 17 includes prepared statements and the documents | P2 |
| Fuera de alcance (332, 339) | Text language and SQL stay out, documents in (pending); MCP and AI-provider dependencies out of the library | P13 |

`tables.prd.md` (3): the module 17 line of the root annotation (252-256) adds prepared statements, the documents and
their schemas (P2, P8); 7.1's *Fija* (91) must make the schema version and the identities visible (P12), store the
random identities and state tokens of §2.2 in the catalog entry of each change, fix the class of the typed error at
the maximum of its schema versions and column ids (P19), state that its generators and tokens take their randomness
from the operating system through the pager (P20), and provide `ulid` and `uuid` text conversion; 7.2 draws an
index's identity and the table's new `index_state` when an index is created or dropped (corrected after review round
2); its domains of `date` and `timestamp` stay its own decision (C4); the D12 annotation (265-268) names the `serde`
set if P11 is approved.

Documents 1 and 2 (design, with 7.1): doc 1's *Schema evolution* records each schema version's `schema_state` token
and the column identities; doc 2's `meta` tree (line 183) holds the table's identity and `index_state`. Neither row
format changes.

Module 17 PRD, when written: the document kinds and their versioning; the statement lifecycle, catalog identities and
states, write transactions (P17) and the invalidation table; the plan and diagnostics rule; §2.4's three levels;
limits per phase, the planner's budget and errors; the acceptance criteria of rounds 1 and 2 plus the round trip, the
differential tests and fuzzing without panics.
Module 18 PRD: the C functions and C# API of §2.2, thread rules, closing with live statements, golden documents shared
with Rust, the error detail, dates outside .NET's range (C4).

## 4. Open product questions

1. **Promise for stored queries.** Decided 2026-11-10 (P6): semantic versioning with one reader (§2.1, *Versions*).
2. **A performance threshold for prepared runs.** Should the module 18 PRD set one (for example a prepared point lookup
   from C# against a key-value `get`, B1 of doc 9), or only measure it?
3. **Plan stability for logical statements.** Is preparing an explained plan the only way to pin a plan, or should a
   logical statement offer an option that disables automatic re-planning?
4. **Distribution of the schemas.** Embedded in the library only, or also as files in the crate, the NuGet package and
   the documentation site?

(The first version's question on rebinding by name is now P16; on queries in write transactions, P3's "any
transaction". Questions 1 and 3 are decided with P6 and P4 in §1's table; 2 and 4 can wait for the PRDs of modules 18
and 17.)

## Sources

Repository (paths under `C:\projects\Krummi`): `docs/design/tables/README.md`, `04-query-tree-and-planner.md`,
`06-c-interface-and-csharp.md`, `08-execution-plans.md`, `09-query-serialization-format.md`, `10-agent-tooling.md`
(read whole); `01-row-format-and-types.md` (types, domains, schema versions, column ids), `05-vectors.md` (NaN rule) in
part; `docs/prd/krummi/krummi.prd.md` (objectives, D2, D7, D8, D12, out of scope, AC21; working copy with the
uncommitted annotation of 2026-11-09); `docs/prd/krummi/tables.prd.md` (N3, N4, N10, 7.1, *Anotaciones a otros PRD*);
`docs/prd/krummi/transactions.prd.md` (T2, T6, T7); `docs/prd/documents/documents.prd.md` (DD9);
`crates/krummi-pager/Cargo.toml` and `src/sys.rs`; `Cargo.lock` and `cargo tree` (normal and development trees,
2026-11-10); the local crate sources of `rustix` 1.1.5 (`Cargo.toml`, `src/lib.rs`, `src/rand/`), `windows-sys`
0.61.2 (`Cargo.toml`, `src/Windows/Win32/Security/Cryptography/mod.rs`) and `getrandom` 0.4.3 (`Cargo.toml`,
`src/backends/windows.rs`, `src/`).

External, fetched on 2026-11-10:

- I-JSON, RFC 7493 (§2.1 surrogates and noncharacters, §2.2 numbers, §2.3 duplicates and member order, §4.2
  Must-Ignore, §4.3 time and durations, §4.4 base64url): https://www.rfc-editor.org/rfc/rfc7493
- JSON Pointer, RFC 6901 (`~0`, `~1`, array indexes): https://www.rfc-editor.org/rfc/rfc6901
- RFC 3339 (`date-fullyear` four digits, lowercase `t`/`z` allowed, duration grammar):
  https://www.rfc-editor.org/rfc/rfc3339
- Base64, RFC 4648 (§3.5 canonical encoding, §4 alphabet, §5 base64url): https://www.rfc-editor.org/rfc/rfc4648
- `serde_json` 1.0.154: manifest https://raw.githubusercontent.com/serde-rs/json/master/Cargo.toml (dependencies,
  features and their comments); sources under https://github.com/serde-rs/json/tree/master/src (every `src/*.rs`
  downloaded and searched for `unsafe`; `de.rs`: `remaining_depth: 128`, `parse_number` turning `-0` into
  `F64(-0.0)`, the `f32` path under `float_roundtrip`, `NumberOutOfRange` on infinite results; `lib.rs`:
  `#[cfg(feature = "raw_value")] mod raw`; `error.rs`: lone surrogate errors)
- `zmij`: https://github.com/dtolnay/zmij (README, Cargo.toml); `itoa`: https://github.com/dtolnay/itoa ; `memchr`:
  https://github.com/BurntSushi/memchr (root listing, no build script); `serde`: https://github.com/serde-rs/serde .
  Counts are lines containing `unsafe` in non-test `.rs` files at HEAD, comments included, so approximate.
- crates.io API (downloads, latest stable, update dates): https://crates.io/api/v1/crates/serde_json and likewise for
  `serde`, `itoa`, `memchr`, `zmij`

From knowledge, not fetched here: serde derive rejecting duplicate struct fields and its behavior under `"op"`
tagging with `deny_unknown_fields`; .NET 8's `Convert` and `Utf8JsonWriter` writing standard Base64; .NET `DateOnly`
and `DateTime` limited to years 1-9999; SQLite's `prepare`/`bind` model; `str::parse` for floats being correctly
rounded and `Display` printing the shortest round-trip form. Every cost here is an estimate or comes from doc 9's
measurement.
