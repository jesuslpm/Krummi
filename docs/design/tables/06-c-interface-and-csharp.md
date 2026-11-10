# 6. The C interface and the C# binding

Technical design, 2026-11-09. Status: reconciled with the user's decisions (README) and the
[fact-check](verification.md). The query tree format is the versioned binary of
[04-query-tree-and-planner.md](04-query-tree-and-planner.md), the one description of the wire format. The LMDB,
RocksDB, csbindgen, interoptopus, uniffi-bindgen-cs and `LibraryImport` facts were checked against their sources in the
fact-check (claims 6.1-6.10); SQLite's C API and the `SafeHandle`/`ref struct` rules are from the maintainers'
documentation as known and are marked "to verify" where a version matters.

## Summary

1. A new crate `krummi-capi` (`cdylib` + `staticlib`) is the only place outside the pager that uses `unsafe`; D12 is
   annotated to name it (decided), and the automatic check gets a second allowed path. The engine crates stay unchanged.
   The first C surface is the store, transactions and queries by serialized tree.
2. The C API is flat and opaque: `krummi_store`, `krummi_read_tx`, `krummi_write_tx`, `krummi_cursor` handles; every
   function returns a `krummi_status` code, one per error class; the message is asked for separately, from a
   thread-local last error (SQLite style). No panic crosses the boundary.
3. Reads are zero-copy (`krummi_slice`, as LMDB's `MDB_val`), valid until the owning transaction ends; every zero-copy
   method has a copying twin in C#. Handles are reference-counted internally, so a wrong release order gives a clear
   error, never undefined behavior. The write transaction checks its thread.
4. A query returns a cursor that yields **batches of rows** into a caller buffer, as document 4 specifies.
5. The C# binding targets .NET 8 or later and uses `LibraryImport` (declarations written by hand from the generated
   C header: csbindgen emits `DllImport`), `SafeHandle`s, and a `ref struct` scope so that spans cannot escape their
   scope in ordinary code (the cases it cannot stop are in R2); the fluent API is a typed method chain; no async in the first version.
6. The fluent builder lives in C#, serializes to the doc-4 binary tree, and one call executes it. Platforms: only those
   that CI builds and tests, today win-x64 and linux-x64. The same acceptance criteria run from C# against the real
   library.

## Requirements

- R1. Stable, small C ABI (DD7 of the parked documents PRD): a C program and .NET can both use it.
- R2. Preserve D11 across the boundary: zero-copy reads, valid exactly while the transaction is open. The binding
  makes using them after a compile error in ordinary code, through `ref struct` scoping, and states the cases it
  cannot stop (an explicit `Dispose` of the scope followed by ending the transaction, `unsafe` code, `MemoryMarshal`;
  see Zero-copy reads). The copying twin of each method is the full guarantee. Corrected 2026-11-10 after an external
  review: R2 first promised that no safe C# code could use a span after its transaction, which the proposed API does
  not provide.
- R3. Preserve T6: a write transaction stays on its thread; a read transaction may move and be shared.
- R4. Preserve T5/T10: a second write on a thread fails with Misuse; an abandoned write aborts.
- R5. No panic crosses the boundary; every `ErrorKind` maps to a stable code.
- R6. D12: `unsafe` is confined, auditable and checked automatically. Public operations must not cause undefined
  behavior for any sequence of C calls, including wrong-order calls where detectable.
- R7. Executing a query tree is one call (plus a cursor for results); the tree is a byte buffer, not an object graph
  crossing the boundary.
- R8. A NuGet package with native libraries for the platforms CI builds and tests (win-x64 and linux-x64 today).
- R9. The C# acceptance tests mirror the Rust ones and run in CI against the built library.

## Options compared

### The boundary style

| System | Handles | Errors | Returned memory | Threads | Takeaway |
|---|---|---|---|---|---|
| SQLite | opaque `sqlite3*`, `sqlite3_stmt*` | int code; `sqlite3_errmsg(db)` per connection | column pointers valid until next `step`/`reset`; `sqlite3_free` for allocations | serialized / multi-thread modes | Stable for decades; "valid until the next call on the object" is a lifetime users misuse |
| LMDB | `MDB_env*`, `MDB_txn*`, `MDB_cursor*` | int code; `mdb_strerror(code)` (pure function) | `MDB_val` {size, ptr} into the map, valid until txn end or next write | txn tied to its thread unless `MDB_NOTLS` | Closest model to Krummi (D11 + T6); a static `strerror` is simple but carries no detail |
| RocksDB C | opaque `rocksdb_t*` etc. | `char** errptr` out-parameter (`*errptr` is a malloc'd message), caller frees | `rocksdb_get` returns a malloc'd copy (a zero-copy `rocksdb_get_pinned` exists in the same header); iterators give pointers valid until `next` | thread-safe DB | Self-contained error text; a copy per plain read |

Krummi takes LMDB's value model, RocksDB's detailed message and SQLite's thread-local "last error" so signatures stay
small. Rejected: copying every read (breaks D11 and the performance goal).

### Error reporting: thread-local vs out-parameter

| | Thread-local last error | `char** err` out-parameter |
|---|---|---|
| Signature | `krummi_status f(...)` | one extra argument on every call |
| P/Invoke | trivial | marshalling and a free on the C# side |
| Pitfall | another call on the thread overwrites the message; the binding reads it immediately | none, but every caller allocates and frees |

Recommendation: the status code is returned; `krummi_last_error_message()` returns the detail of the last failure on
the calling thread, valid until the next Krummi call on that thread; C# copies it into a `string` in the same
synchronous frame as the failed call (never across an `await`, since a continuation may resume on another thread).

### Rust to C# bridges

| Tool | What it gives | Against |
|---|---|---|
| plain cbindgen + hand-written `LibraryImport` | full control of the ABI; the C header is also the product for C users; source-generated marshalling; trimming and AOT friendly | the C# declarations are written by hand (about 40 functions) |
| csbindgen (Cysharp) | generates C# declarations from the Rust `extern "C"` items; its README (1.9.x) says it generates `DllImport` code, all examples are `[DllImport(..., CallingConvention.Cdecl)]`, with function pointers for callbacks, and its sources contain no `LibraryImport` generator; it also shows the NuGet `runtimes/<rid>/native/` layout and `NativeLibrary.SetDllImportResolver` | C# only; emits `DllImport`, not the decided `LibraryImport`; wrappers by hand anyway |
| interoptopus | one Rust description generates bindings with wrapper classes: C# (stable) and Python (experimental) backends are listed in its README; a C backend was not confirmed | a macro layer over the ABI; its generated classes compete with our own `SafeHandle` / ref-struct design; smaller community |
| uniffi | rich types, objects, async, callbacks | C# support is third party (`uniffi-bindgen-cs`, a separate NordSecurity project, v0.11.1 for uniffi 0.31, needs .NET 8+); serializes through a `RustBuffer` per call (not checked), which would contradict zero-copy; extra runtime dependency |

Decided combination: cbindgen for the header, and the `[LibraryImport]` declarations (about 40 functions) written by
hand from it, because `LibraryImport` is the decided style and csbindgen cannot emit it (converting its `DllImport`
output by script would be brittle). A CI test compares the exported function names of the header with the declared
entry points so that the two cannot drift. csbindgen remains usable as a cross-check or a throwaway generator, never as
the source of the committed declarations. The ABI is the contract, not a tool.

## Decided design

### Crate and D12

New workspace crate `crates/krummi-capi`, `crate-type = ["cdylib", "staticlib", "rlib"]`, depending only on `krummi`
(its safe public API). Annotation to D12 (decided; to be written into the root PRD): "`krummi-capi` is the
only other crate that may use `unsafe`, because a C interface cannot be written without it. Its `unsafe` is limited to
converting raw pointers and lengths into safe references, creating and releasing handles, and the lifetime extension
described in its design document. No engine logic lives there." The automatic check (today: no `unsafe` outside the
pager) becomes an allow-list of two paths. Inside the crate: `#![deny(unsafe_op_in_unsafe_fn)]`,
`clippy::undocumented_unsafe_blocks = deny`, a `// SAFETY:` comment per block, and all the `unsafe` in one module
(`raw.rs`: pointer-to-slice, handle box/unbox, lifetime extension); the exported functions are thin and call safe
helpers.

Rejected: `extern "C"` inside the `krummi` crate (spreads `unsafe` into the engine crate D12 keeps clean) and a
separate repository (versions drift, CI is duplicated; the alternative of question 2).

### Handles, lifetimes and release order

The Rust types borrow (`ReadTransaction<'s>` borrows `Store`); a C handle cannot carry a lifetime, so handles own
reference-counted state:

- `krummi_store`: a box holding an `Arc` of the store.
- `krummi_read_tx`: a box holding an `Arc` to the store and the transaction, whose `'s` is extended to `'static` (one
  `unsafe` in `raw.rs`). It is sound because the Arc keeps the store alive for the transaction's whole life and the
  transaction field is declared, hence dropped, before the Arc; a test pins the field order.
- `krummi_cursor`: a box holding an `Arc` to its transaction's inner state, which has an "ended" flag. A cursor used
  after `krummi_read_end` returns `KRUMMI_MISUSE`, never undefined behavior.

So releasing in the wrong order is an error, not UB. Closing the store with open transactions: T8 of the transactions
PRD is already decided for Rust, as a compile-time rule (`Store::close(self)` consumes the store and transactions
borrow it, `ReadTransaction<'s>`; the code does not compile), so the engine has no runtime rule to forward. A C handle
has no compiler, so the C interface needs a runtime rule of its own, a **new decision** (question 8). The recommended
behavior is `MISUSE` with the store left open; with handles that own an `Arc` of the store it follows naturally from
closing by `Arc::try_unwrap`, which fails while any transaction handle still holds a reference (to confirm in the
PRD). Double free of a handle is a caller bug C cannot detect (documented, as in SQLite); `*_free` accepts null.

D11 across the boundary: a slice is valid until the read transaction ends. In a write transaction it is valid until
the next mutating call on that transaction (as in LMDB), stated in the header.

### Threads

- Read transactions are Send + Sync in Rust (T6): any thread, concurrently. A cursor is used by one thread at a time
  (documented; C# cursors are not thread-safe).
- Write transactions: the capi records the opening thread and checks it at the start of every call taking the handle;
  another thread gets `KRUMMI_MISUSE` ("write transaction used from another thread"). This is T6 enforced at runtime
  because C has no compiler to enforce it; the cost is one thread-id comparison.
- A second write on the same thread already fails in the engine (T5).

### Panics and ABI

Each exported function goes through one macro with `catch_unwind(AssertUnwindSafe(...))`; a panic becomes
`KRUMMI_INTERNAL` plus a message. The cdylib profile must use `panic = "unwind"`, not `abort`. A live write
transaction is aborted by its `Drop` (T10), so the store stays usable. Null pointers, lengths above `isize::MAX` and
invalid UTF-8 names return `KRUMMI_INVALID_ARGUMENT`.

ABI stability: functions are only added; the only structs passed by value are `krummi_slice` and the result-row
pair of the batch call; options live in a `krummi_options` struct whose first field is `uint32_t size` (the Win32
pattern; SQLite's extensible structs use an `iVersion` field instead) so fields can be appended;
`krummi_abi_version()` returns major.minor and the C# binding refuses a different major at load. Status codes are
numbers, never renumbered, one per `ErrorKind` (Access, StoreInUse, NotAStore, Damaged, DifferentFormat, DiskFull, Io,
InvalidOptions, MaxSize, OutOfMemory, Timeout, Misuse, InvalidSize, DifferentTreeType, TreeNotFound,
TreeAlreadyExists) plus INTERNAL and INVALID_ARGUMENT. cbindgen generates the header in CI; it is committed and a CI
step diffs it against the last release to catch breaking changes.

### Sketch of the C functions

```c
typedef struct krummi_store krummi_store;
typedef struct krummi_read_tx krummi_read_tx;
typedef struct krummi_write_tx krummi_write_tx;
typedef struct krummi_cursor krummi_cursor;
typedef struct { const uint8_t *ptr; size_t len; } krummi_slice;
typedef int32_t krummi_status;                       /* 0 = ok */

uint32_t       krummi_abi_version(void);
krummi_status  krummi_last_error_kind(void);         /* thread-local */
const char    *krummi_last_error_message(void);      /* UTF-8, valid until the next call on this thread */

krummi_status krummi_store_open(const char *path_utf8, const krummi_options *o, krummi_store **out);
krummi_status krummi_store_close(krummi_store *s);   /* MISUSE while transactions are open (question 8) */

krummi_status krummi_read_begin(krummi_store *s, krummi_read_tx **out);
void          krummi_read_end(krummi_read_tx *t);
krummi_status krummi_write_begin(krummi_store *s, int64_t timeout_ms, krummi_write_tx **out);
krummi_status krummi_write_commit(krummi_write_tx *t);   /* consumes the handle, also on failure */
void          krummi_write_abort(krummi_write_tx *t);

krummi_status krummi_read_get(krummi_read_tx *t, const char *tree, krummi_slice key,
                              krummi_slice *value, int *found);
krummi_status krummi_write_set(krummi_write_tx *t, const char *tree, krummi_slice key, krummi_slice value);
krummi_status krummi_read_cursor(krummi_read_tx *t, const char *tree, krummi_cursor **out);
krummi_status krummi_cursor_next(krummi_cursor *c, krummi_slice *key, krummi_slice *value, int *has);
void          krummi_cursor_free(krummi_cursor *c);

/* tables (modules 7 and 17): one call executes a serialized query tree (doc 4 wire format) */
typedef struct { krummi_slice key; krummi_slice value; } krummi_row;   /* a clustered table row: tuple key, doc-1 row */
krummi_status krummi_read_query(krummi_read_tx *t, krummi_slice tree_bytes, krummi_cursor **rows);
krummi_status krummi_write_query(krummi_write_tx *t, krummi_slice tree_bytes, uint64_t *affected);  /* if the PRD adds writes by tree */
krummi_status krummi_cursor_next_batch(krummi_cursor *c, krummi_row *rows, size_t capacity, size_t *count); /* *count == 0: end */
```

Tree handles are not exposed: functions take the tree name. The engine's `open_tree(&str)` opens a tree by name on
each call (`crates/krummi/src/lib.rs:439,680`); B9 of the trees PRD only says that several trees can be open at once,
and no per-transaction name cache exists. The capi therefore pays one catalog lookup per call or keeps its own small
map from tree name to open tree inside the transaction handle's state, which avoids a fifth handle type; queries are
unaffected, since the tables are opened inside Rust. A batch is the decided result shape: up to `capacity` rows per
call, each a reference to a table row (the encoded primary key and the doc-1 row, which does not repeat the key
columns), valid until the next call on the cursor or the end of the transaction, whichever comes first. Computed
values (a projected distance, aggregates) travel in the cursor's own buffer, read through accessor calls (doc 4).
C# decodes the row columns in place from the doc-1 format and the key columns from the key (doc 3). The key-value
functions above (`krummi_read_get`, `krummi_write_set`, `krummi_read_cursor`, `krummi_cursor_next`) are the tree
surface of the boundary rules; the PRD decides whether the first release keeps them next to the table surface.

### C# binding

- **LibraryImport vs DllImport.** Decided: `[LibraryImport]` (.NET 7+, the source generator ships with the .NET 7 SDK):
  source-generated marshalling, no runtime IL stub, trimming and AOT safe; the minimum is .NET 8. All signatures are
  blittable (pointers, `nuint`, `int`); names go as UTF-8 spans encoded by the binding (`stackalloc` for short names).
  The declarations are written by hand from the header (csbindgen emits `DllImport`; see the bridges table).
  `NativeLibrary.SetDllImportResolver` selects `runtimes/<rid>/native/`.
- **SafeHandle and release order.** `StoreHandle`, `ReadTxHandle`, `WriteTxHandle`, `CursorHandle` derive from
  `SafeHandle`. A transaction object calls `DangerousAddRef` on its `StoreHandle` for its whole life, so critical
  finalization cannot release the store first; the native reference counts are the second line of defense. `Dispose`
  is the normal path; finalizers only release the handle (and abort a write transaction) so a leak never wedges the
  writer lock.
- **Zero-copy reads.** Values are `ReadOnlySpan<byte>` over native memory. The compiler ties a span to a lifetime only
  through `ref struct` scoping, so: `ReadTransaction` is an ordinary class (storable, shareable), reads go through a
  `ref struct ReadScope` obtained by `using var scope = tx.Enter();`, and `scope.Get(...)` returns a span whose escape
  scope is the scope's, so the compiler rejects returning or storing it past the `using` (C# 11 `scoped` rules, to
  verify on the chosen language version). Ending the transaction while a scope is live throws. Cursors are `ref
  struct`s, so they cannot be captured by lambdas or live across `await`. The compiler bounds a span by the
  scope's lexical lifetime, not by its `Dispose` (added 2026-11-10 after an external review): code that calls
  `scope.Dispose()` explicitly, then ends the transaction, can still read a span inside the same block after the native
  memory is gone. A `ref struct` cannot prevent it, and the binding cannot see a span being read. Residual risk: an explicit `Dispose` of the scope followed by ending the
  transaction, `unsafe` code, or `MemoryMarshal` can still read freed memory (documented, as in LMDB); a Roslyn
  analyzer that flags a span used after its scope's `Dispose` is a possible later addition, not in the first version. Each zero-copy method has a copying twin
  (`GetCopy`) for callers who prefer safety.
- **Write transactions** are a `sealed class` that stores `Environment.CurrentManagedThreadId` and throws from another
  thread (the native check is the backstop); never usable across `await`.
- **Async.** None in v1. Reads are memory-bound with no blocking I/O besides page faults; `Begin` of a write may wait
  up to its timeout. An `async` API would imply thread hops that break the write-thread rule. Documented pattern:
  `await Task.Run(() => { using var tx = store.Write(); ...; tx.Commit(); })`, the whole transaction inside one
  synchronous delegate. Read transactions may be handed between threads.
- **Errors.** One `KrummiException` carrying an `ErrorKind` enum mirroring the codes, created by reading the
  thread-local message right after a nonzero status.
- **Voron as the idiomatic reference.** Voron: `using (var tx = env.WriteTransaction()) { var table =
  tx.OpenTable(schema, "name"); table.Set(...); tx.Commit(); }`, `Slice` over unmanaged memory, `ReadByKey(...,
  out TableValueReader)` returning readers into mapped pages. Keep the shape (transaction scope, then table, then
  operations; explicit `Commit`, rollback on dispose) and replace `Slice` by `ReadOnlySpan<byte>`; `tx.Table("x")`
  returns the fluent builder.
- **Fluent builder and serializer.** The builder produces an immutable node graph; a `QuerySerializer` writes it into a
  pooled `ArrayBufferWriter<byte>` in the doc-4 format (the format of document 4: magic, version, flags, table, node
  count, pre-order nodes as tag, length and payload, then the parameter block, kept separate so a plan can be reused), then calls `krummi_read_query`. Krummi treats the
  bytes as untrusted: version check, bounded depth and counts, full structural validation before touching data; an
  invalid tree returns `INVALID_ARGUMENT` with the byte offset. Rust and C# share golden byte files; each side must
  produce and parse them.
- **Packaging.** Decided platforms: only those that CI builds and tests, today win-x64 and linux-x64
  (`.github/workflows/ci.yml`: `windows-latest`, `ubuntu-latest`). User: "de momento no tenemos compilación ni pruebas en
  arm64." One NuGet `Krummi` with `runtimes/win-x64/native/krummi.dll` and `runtimes/linux-x64/native/libkrummi.so`
  (glibc, as built on `ubuntu-latest`); no arm64, osx or musl package until CI builds and tests that target. Alternative:
  one package per RID plus a meta package (the pattern of the SQLitePCLRaw bundles, not checked); start with one and
  split if size demands. The package version tracks the ABI major.minor.

### Testing across the boundary

1. Rust tests inside `krummi-capi` call the `extern "C"` functions: every status code, null arguments, wrong thread,
   wrong release order, injected panic (test-only function), commit twice.
2. A small C program compiled in CI includes the generated header (as C99 and as C++) and runs the happy and error
   paths.
3. A C# xUnit project runs the acceptance criteria that matter at the boundary against the built library; a twin test
   carries the AC id in its name so a script checks the coverage table. Extra C# tests: ref-struct escape (Roslyn
   compile-fail test expecting diagnostics), finalizer order (`GC.Collect` after dropping references), thread rule,
   read sharing across threads, golden query bytes.
4. CI runs the C# tests on win-x64 and linux-x64, the platforms of the package.

## Costs and risks

- The `unsafe` crate is the main soundness risk. Mitigations (one `raw.rs`, refcounted handles, ended flag, Miri on
  the Rust-called functions) bound it; Miri cannot cross a real FFI call.
- Zero-copy cannot be made fully safe in C#: the ref-struct scope protects ordinary code only.
- The `'static` lifetime extension is where the Rust compiler stops helping; it needs the field-order test and review.
- Native builds for two platforms and NuGet signing: a CI cost (about a week of setup, less than for the wider set of the first draft).
- Freezing the ABI commits us; the header is reviewed as a product. Ship `0.x` as "unstable until 1.0".
- A query-format change breaks old clients: guarded by the version field and the ABI check.
- Order of magnitude: capi 2-3 days for the key-value surface, 2 more with tables; C# core 3-4 days; packaging and CI
  3-5 days.

## Product questions

1. **Scope of the first C surface.** *Decided 2026-11-09:* the store, transactions and queries by serialized tree
   (README: "Me sirve"). The boundary rules (handles, errors, threads) are proven early on the existing key-value API,
   and queries add the functions above once module 17 exists.
2. **D12 annotation.** *Decided:* `krummi-capi` may use `unsafe`, with the two-path allow-list; the alternative of a
   separate repository is dropped.
3. **C# builder style.** *Decided:* a typed method chain with typed column references, no expression trees, AOT
   friendly. LINQ expression trees stay a possible later addition.
4. **Minimum .NET.** *Decided:* .NET 8 or later, `LibraryImport`.
5. **Async.** *Decided:* none in the first version; `Task.Run` around whole transactions.
6. **Platforms of the first NuGet.** *Decided:* win-x64 and linux-x64 only, the platforms CI builds and tests. The
   first draft proposed osx-arm64 as well; it needs a CI job first.
7. **Shape of query results.** *Decided:* a cursor that returns batches of rows into a caller buffer, with zero-copy row
   references valid until the next call or the end of the transaction (README: "a cursor that returns batches of
   rows"). The first draft recommended one row per call and rejected batches; document 4's batch cursor won, and a
   row-at-a-time loop is a thin helper on top of it.
8. **Closing a store with open transactions.** *New decision for the C interface, still open.* T8 is decided for Rust
   as a compile-time rule, so there is nothing pending in the transactions PRD. Options: (a) `krummi_store_close`
   returns `MISUSE` and keeps the store open (recommended); (b) it closes and invalidates the transactions, which the
   reference-counted handles cannot do without the Arc outliving it. Recommendation: (a).

## Sources

Repository:
- `C:\projects\Krummi\docs\design\tables\README.md`
- `C:\projects\Krummi\docs\prd\krummi\krummi.prd.md` (D11, D12 and their annotations)
- `C:\projects\Krummi\docs\prd\krummi\transactions.prd.md` (T5, T6, T8, T10)
- `C:\projects\Krummi\crates\krummi\src\lib.rs` (Store, ReadTransaction, WriteTransaction, trees)
- `C:\projects\Krummi\crates\krummi-pager\src\error.rs` (ErrorKind)
- `C:\projects\Krummi\docs\prd\documents\documents.prd.md` (DD7, DD8)
- `C:\projects\ravendb\src\Voron\Impl\Transaction.cs`, `C:\projects\ravendb\src\Voron\Data\Tables\Table.cs`

External (LMDB, RocksDB, csbindgen, interoptopus, uniffi-bindgen-cs and LibraryImport fetched in the fact-check of
2026-11-09; the SQLite and `SafeHandle` pages were not):
- https://www.sqlite.org/c3ref/intro.html and https://www.sqlite.org/threadsafe.html
- http://www.lmdb.tech/doc/ (MDB_val, mdb_txn_begin, MDB_NOTLS)
- https://github.com/facebook/rocksdb/blob/main/include/rocksdb/c.h
- https://github.com/mozilla/cbindgen
- https://github.com/Cysharp/csbindgen
- https://github.com/ralfbiedert/interoptopus
- https://github.com/mozilla/uniffi-rs and https://github.com/NordSecurity/uniffi-bindgen-cs
- https://learn.microsoft.com/dotnet/standard/native-interop/pinvoke-source-generation (LibraryImport)
- https://learn.microsoft.com/dotnet/api/system.runtime.interopservices.safehandle
- https://learn.microsoft.com/dotnet/csharp/language-reference/builtin-types/ref-struct
