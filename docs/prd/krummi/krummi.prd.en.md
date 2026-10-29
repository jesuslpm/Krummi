# Krummi, an embedded storage engine inspired by Voron — PRD

**Version**: 1.1
**Date**: 2026-10-22
**Parent document**: none (root PRD)

> English translation of [krummi.prd.md](krummi.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Build **Krummi**, an embedded, transactional storage engine written in Rust, **for production use**, that
reproduces the capabilities of **Voron**, the storage engine of RavenDB. It is delivered as a library that other
Rust applications embed in their own process, plus a command-line tool to operate it.

This document is the umbrella of the product: it sets its objectives, the guarantees and rules that apply to all
modules, and splits the product into child PRDs, each with its own execution and its own closing (see
[Split into child PRDs](#split-into-child-prds)). The first one is the **Pager**.

**Objectives**:

1. **A reliable engine**: ACID transactions, durability against process crashes, system crashes and power cuts,
   and detection of any damage in what the engine stores.
2. **All the capabilities of Voron**: the product is complete when the 16 child PRDs of the
   [split](#split-into-child-prds) are closed (decision D4).
3. **Competitive, measured performance**: "competitive" means meeting the thresholds set by the benchmark child
   PRD (decision D5).

**Why**: Voron is a design proven for years in production (one writer with concurrent readers that do not block,
a write-ahead journal, automatic recovery), but it only exists for .NET. Krummi brings that design to Rust as a
product of its own, without being tied to its on-disk format.

### Product decisions

| ID | Decision |
|---|---|
| D1 | **Production product, with its own format.** Krummi neither reads nor writes Voron or RavenDB files. It reproduces their capabilities, not their format. |
| D2 | **Public surface**: an embeddable Rust library (the main product) and a command-line operations tool, with messages in Spanish. |
| D3 | **Platforms**: Windows x64 and Linux x86-64 with glibc, on local disks. Both are tested in every child PRD. |
| D4 | **Scope**: all the capabilities of Voron, including the structures added for Corax (containers, posting lists, compact trees and lookups), which are built after the core. |
| D5 | **Performance**: measured with a reproducible comparative benchmark against other embedded Rust engines, with the same workloads on the same machine. The benchmark child PRD (6) sets the engines, the workloads and the thresholds for modules 1 to 5; later modules with an impact on performance set their own threshold on that benchmark. Before the benchmark, modules 1 to 5 have no threshold. |
| D6 | **One process per store.** The process that opens a store locks it exclusively; concurrency (one writer and many readers) is between threads of that process. Opening a store that is already open, from another process or from the same one, fails. The command-line tool works only on stores that nobody has open. |
| D7 | **Format evolution**: each store records its on-disk format version, a positive integer that changes with every incompatible format change, also before 1.0. An engine version opens only the format it knows; faced with another one, older or newer, it refuses without modifying anything. From product version 1.0, the command-line tool migrates an older format to the current one without touching the original; before 1.0 there is no migration and the data is recreated. |
| D8 | **Library errors**: errors are grouped into classes that the application tells apart without reading text; each class covers the situations assigned to it by the [validations table](#validations-and-messages) or by the owning child PRD. Each error carries as separate data what its message needs (folder, versions, page number, cause). The text of those errors is in English; the command-line tool shows the message of each situation in Spanish. |
| D9 | **Stores that need recovery in the tool**: if a store was closed by a crash, the tool's commands recover it on opening (G5), as the library would, and then do their work. The only ones that do not modify it are verification and data rescue (copying whatever can be read from a damaged store), which work on the store as it is. |
| D10 | **Recognizing a store**: a folder contains a Krummi store when its identification mark is intact. Everything else that remains readable without a key (G8), including the format version, carries its own integrity check: if it is altered, the store is damaged; it is neither of another format nor ceases to be a store. Child PRD 1 sets the mark and where it is. |
| D11 | **Zero-copy reads** (v1.1): reads return to the application references to engine memory, without copying the data into a caller's buffer. The references are valid as long as the read transaction that obtained them is open, and the compiler prevents using them afterwards. If a page is stored encrypted (module 9) or compressed (module 5), it is decrypted or decompressed once into the transaction's memory and the reference points there. Writes work on a copy of the page, which snapshot isolation (G3) requires. |
| D12 | **Unsafe code** (v1.1): the Pager (module 1), with the encrypted variant that module 9 adds to it, is the only part of the engine that may use unsafe Rust code (`unsafe`). It encapsulates it behind a safe interface; the rest of the engine is forbidden from using it and an automatic check enforces this. |

*Annotation of 2026-10-22 to D12: no public operation causes undefined behavior, unless another program modifies the
store's files while it is open, which is out of scope (decision P16 of
[pager.prd.en.md](pager.prd.en.md)).*

*Annotation of 2026-10-22 to D7: an intact store whose format number is lower than the current one is of an older
format, also with the number `0`, which no version uses and appears only in tests (decision P5 of
[pager.prd.en.md](pager.prd.en.md)).*

*Annotation of 2026-10-22 to D11: the reference of a read may also point to the memory where the engine keeps a
committed version of the page that is not in the data file, never to a copy made for that read (decision T16 of
[transactions.prd.en.md](transactions.prd.en.md)).*

*Annotation of 2026-10-23: the product is called Krummi; its packages and the files of a store take the names of R2
and R3, and this document was updated with the new names (decisions R1–R13 of
[rename-to-krummi.prd.en.md](rename-to-krummi.prd.en.md)). The file names and format numbers of the earlier
annotations are read according to R3 and R5, and the current format is `3`. A folder with a MuninnDB store does not
contain a Krummi store (D10, decision R6).*

*Annotation of 2026-10-24 to D7: with free space, the current format version becomes `5` (decision F11 of
[free-space.prd.en.md](free-space.prd.en.md)).*

*Annotation of 2026-10-25 to D7: with the tree catalog, the current format version becomes `6` (decision A13 of [trees-keys.prd.en.md](trees-keys.prd.en.md)).*

*Annotation of 2026-10-25 to D11: listing the trees returns a copy owned by the application, not references to engine memory (decision A3 of [trees-keys.prd.en.md](trees-keys.prd.en.md)).*

*Annotation of 2026-10-27 to D7: with large values, the current format version becomes `7` (decision V9 of [trees-large-values.prd.en.md](trees-large-values.prd.en.md)).*

*Annotation of 2026-10-27 to D11: every tree value, also one of up to 256 MiB stored off its leaf page, is returned as a single contiguous sequence of bytes without copying, also in temporary mode (decision V2 of [trees-large-values.prd.en.md](trees-large-values.prd.en.md)).*

*Annotation of 2026-10-29 to D7: with multi-value trees, the current format version becomes `8` (decision M14 of [trees-multivalue.prd.en.md](trees-multivalue.prd.en.md)).*

### Starting state

Verified on 2026-10-22 against the repository and against the public Voron code in the RavenDB repository:

| Aspect | Situation |
|---|---|
| MuninnDB | There is no product code or earlier PRD. The repository contains only the working methodology, the project skills, the worktree guard and its scripts. |
| Voron, core (reference) | Data file divided into 8 KiB pages and memory-mapped, with encrypted and 32-bit variants; write-ahead journal with recovery; scratch area for the pages modified by the current transaction; free-space management; keys of up to 1,024 bytes. |
| Voron, structures (reference) | B+ trees (with multi-value trees, leaf-page compression and streams), fixed-size trees, raw data sections, tables with schema and indexes (with dictionary value compression), containers, posting lists, compact trees and lookups. |
| Voron, operation (reference) | Full and incremental backup, compaction, store schema upgrade between versions, statistics and storage report, and a separate recovery tool. |

## Actors

- **Integrating developer**: embeds the library in their Rust application, opens stores, runs transactions,
  makes backups while the store is in use and handles errors by their class.
- **Operator**: uses the command-line tool on a store that nobody has open to back up, restore, compact, verify,
  inspect, migrate or rescue data.
- **Maintainer**: runs the comparative benchmark and the tests on Windows and Linux before each release.

## Product guarantees

They apply to all modules. A child PRD may extend them; if it needs to relax one, it declares it as an exception
and annotates this document.

| ID | Guarantee |
|---|---|
| G1 | **Atomicity**: a committed write transaction applies all its changes; an aborted one, or one interrupted before it starts to commit, applies none; one interrupted during the commit applies all or none, never a part. |
| G2 | **Durability**: in durable mode (the default), when the commit of a transaction ends without error, its changes survive a process crash, an operating system crash or a power cut. A disk that honors the commands to take the data to the physical medium is assumed; a disk that ignores them is outside the guarantee. |
| G3 | **Snapshot isolation**: each read transaction sees the store as it was when it started, even if writes are committed afterwards. Readers do not block the writer, nor does the writer block readers. |
| G4 | **A single writer**: at any moment there is at most one write transaction open per store; the others wait their turn. The transactions child PRD (2) sets the waiting order, the waiting limits and what happens if a thread asks for a second write while it has one open. |
| G5 | **Recovery**: when opening a store after a crash, the engine returns it, without user intervention, to a state that includes at least all the transactions whose durability was guaranteed, and no part of an uncommitted transaction. What was left half-written at the end of the journal is a commit that did not finish and is discarded; damage in what was already committed is damage (G6). An interrupted or failed recovery is repeated in full on the next opening. |
| G6 | **Integrity**: the engine never returns damaged data from any part of the store as valid; when it detects damage, the operation fails with the damage error class. What a read had already returned before reaching the damage was valid. The same holds for backup, compaction, migration and verification, which check everything they read. |
| G7 | **Disk full**: if a write does not fit on disk, the operation fails with the disk-full error class, the transaction applies none of its changes and the store stays open and readable. When there is space again, later writes work without reopening the store. |
| G8 | **Confidentiality**: with encryption on, no user data reaches the disk unencrypted: not the data file, nor the journal, nor the temporary files, nor the backups, which are left encrypted. Only what is needed to recognize the store and its format version stays readable without the key. A wrong key is never reported as damage to the store. |
| G9 | **Temporary mode**: a store can be opened in memory only, for tests, without a folder. It meets G1, G3, G4 and G6; it creates no file or folder and keeps nothing when closed. Requesting encryption in this mode is an invalid-options error. |
| G10 | **Complete copies or nothing**: the operations that produce another store or file (backup, restore, compaction, migration, data rescue) write only to the destination given by the caller, which must not already exist nor be inside the source. If they fail or are interrupted, the source is left as it was, except for the recovery after a crash that some of them do on opening it (D9), and no half-done result is left under the destination's name. |
| G11 | **Non-durable modes**: the journal child PRD (3) may offer commit modes without immediate durability. In them G1, G3, G4 and G6 still hold; only G2 and the exact point to which G5 returns are relaxed, and the child PRD sets how far. |
| G12 | **System failures on read or write**: if the system fails when writing or when taking data to disk for a cause other than lack of space, the operation fails with the input/output error class, and the store stops accepting writes until it is closed and reopened, which recovers it (G5); reads keep working. If the system fails on a read, the operation fails with the same class and the store remains usable. A system failure is never reported as damage, nor damage as a system failure. |

*Annotation of 2026-10-22: exception to G12. As in Voron, a physical disk failure when reading through the
memory-mapped file projection ends the process instead of returning an error; G12 still holds for writes and for
what is not read through the projection (decision P17 of [pager.prd.en.md](pager.prd.en.md)).*

*Annotation of 2026-10-22: G6 is made precise for the store header. If only one of its two copies is damaged, the
store opens without error with the other one, because no damaged data is returned (decision P6 of
[pager.prd.en.md](pager.prd.en.md)).*

*Annotation of 2026-10-22: temporary exception to G1 (only on crashes), G2, G5 and G12 (in what the reopening
recovers) until module 3. Without a journal, a crash, an input/output failure that the close does not resolve or
ceasing to use the store without closing it when the write fails may leave the data file without part of what was
committed since the last opening; after a close without error, what was committed is there, and G6 still holds
(decision T9 of [transactions.prd.en.md](transactions.prd.en.md)).*

*Annotation of 2026-10-22: the previous temporary exception ends with module 3; what was committed durably can no
longer be missing after reopening (decisions J4 and J13 of [journal.prd.en.md](journal.prd.en.md)).*

*Annotation of 2026-10-22: G5, G6 and D10 are made precise for the journal. A damaged, incomplete or missing journal
commit that the journal itself does not record as durable is discarded without error as a commit that did not
finish; and if the last durable commit is damaged with no later commit, it cannot be told apart from one and is
also discarded (decision J9 of [journal.prd.en.md](journal.prd.en.md)).*

*Annotation of 2026-10-23: with the group write of the journal, the above holds by groups of commits written
together: if the last group is damaged with none after it, the damaged commit and the following ones are discarded;
and the current format version becomes `4` (decisions E9 and E15 of
[journal-group-commit.prd.en.md](journal-group-commit.prd.en.md)).*

## Flow

1. The integrating developer opens a store giving its folder, or in temporary mode. If the folder does not exist,
   the library creates it, with the intermediate folders, and creates the empty store. Creation is atomic: either
   a complete store is left, or the folder is left empty, with no remains of the store.

   *Annotation of 2026-10-22: after an interrupted creation a temporary file may remain until the next opening,
   which deletes it and creates the store (decision P8 of [pager.prd.en.md](pager.prd.en.md)).*
2. The opening does the checks in the [fixed order](#order-of-the-checks-on-open) and, if the store was closed by
   a crash, recovers it (G5).
3. The application opens read transactions from any thread and one write transaction at a time (G3, G4); it
   reads and modifies the data structures and commits or aborts.
4. The application closes the store and the exclusive lock is released.
5. With the store closed, the operator runs the command-line tool operations (D9).

## Split into child PRDs

Each module is a child PRD of this one, with its own execution. The order is the build order: each one relies on
the previous ones. The *Sets* column names what that child PRD must decide, in addition to its content and its
own errors and messages.

| # | Child PRD | Content | Sets |
|---|---|---|---|
| 1 | **Pager** | Data file divided into pages; format version; exclusive lock; file growth; detection of damaged pages; atomic creation; temporary mode. | The page size; the files that make up a store and what to do with foreign files in its folder; how the store is locked without leaving remains that contradict atomic creation; the maximum store size; the out-of-memory error in temporary mode. |
| 2 | **Transactions** | Read and write transactions with snapshot isolation, one writer and concurrent readers. | What G4 assigns to it; whether a transaction can move from one thread to another; the reader limit; what happens when closing a store with open transactions or when using a closed one. |
| 3 | **Journal and recovery** | Write-ahead journal that makes commits durable (G2), with compression; moving what was committed to the data file; recovery (G5). | The non-durable modes (G11); disk full during recovery. |
| 4 | **Free space** | Reuse of the pages that transactions free. | What happens to store growth when a long-running reader prevents pages from being reused. |
| 5 | **B+ trees** | Trees with variable-size keys and values, large values, in-order traversal, prefix search, multi-value trees, fixed-size trees and leaf-page compression. | The maximum key, value and tree-name sizes, and their errors. |
| 6 | **Comparative benchmark** | Reproducible benchmark against other embedded Rust engines (D5). | The reference engines, the workloads, the command that runs it, the shape of the report and the thresholds for modules 1 to 5. |
| 7 | **Tables** | Raw data sections; tables with schema, primary key and secondary indexes; dictionary value compression. | Their errors (duplicate key, different schema). |
| 8 | **Streams** | Values of arbitrary size stored and read in parts. | — |
| 9 | **Encryption** | Authenticated encryption at rest of everything the engine writes to disk (G8). | The errors and messages for a wrong, missing or surplus key. |
| 10 | **Backup and restore** | Full and incremental backup from the library with the store in use; restore. | Disk full at the destination; how incrementals are chained; the error of restoring a damaged or incomplete backup. |
| 11 | **Compaction** | Copy of the store without the free space, to reclaim disk. | Disk full at the destination. |
| 12 | **Operations tool** | Backup, restore and compaction commands for closed stores (modules 10 and 11); inspection, statistics and storage report; integrity verification; data rescue from a damaged store; format migration from 1.0 (D7). | The name of the executable, of its commands and options; the help and the version; the incorrect-usage messages; the encoding and line breaks of the output; how paths are displayed; interruption with Ctrl+C; the exact output of each command, including how verification reports each damage; what migration does with an older-format store that needs recovery. |
| 13 | **Containers** | Storage of small values addressed by identifier. | — |
| 14 | **Posting lists** | Sorted, compressed lists of identifiers. | — |
| 15 | **Compact trees** | Trees with compressed keys. | — |
| 16 | **Lookups** | Fixed-key dictionaries. | — |

*Annotation of 2026-10-24 to module 4: a long-running read does not prevent reusing freed pages nor make
`krummi.data` grow; what it retains are page versions in `krummi.temp`, which count toward the maximum size
(decision F7 of [free-space.prd.en.md](free-space.prd.en.md)).*

*Annotation of 2026-10-24 to module 5: it is split into six child PRDs (5.1 to 5.6) under a module PRD with no
execution of its own, and it counts as closed for objective 2 when all six are; the maximum sizes of a key (2,048
bytes), a value (256 MiB) and a tree name (1,024 bytes) and their errors are in B4–B8 of
[trees.prd.en.md](trees.prd.en.md).*

## Validations and messages

Situations common to the whole product. The library returns the error class of each row (D8); the message column
is the text shown by the command-line tool, where `<carpeta>` is the store's folder, `<destino>` the destination
of a copy, `<versión>` a format number and `<número>` the page number, in decimal and counting from 0. Child PRD
12 sets how `<carpeta>` and `<destino>` are displayed and the text of `<causa>`.

| Situation | Rule | Error class | Message in the tool |
|---|---|---|---|
| The folder cannot be used: it is a file, permission is missing, the file system is read-only or the path is not valid | The opening fails without touching anything. | Access | `No se puede usar la carpeta <carpeta>: <causa>.` |
| The source folder does not exist (tool only) | The command fails: the tool never creates a store in a source folder. | Access | `La carpeta <carpeta> no existe.` |
| The store is already open, by another process or by the same one | The opening fails without touching the store (D6). | Store in use | `El almacén <carpeta> ya está abierto.` |
| The folder is not a Krummi store: it has some entry, including hidden ones, that is not part of one, or it is empty and the tool opens it | The opening fails without touching anything. | Not a store | `La carpeta <carpeta> no contiene un almacén de Krummi.` |
| The store is recognized as a Krummi one, but what is needed to open it is damaged | The opening fails without touching the store (G6). It wins over the damaged-page row when the damage is found on opening. | Damage | `El almacén <carpeta> está dañado y no se puede abrir.` |
| The store has an older format | The opening fails without touching the store (D7). | Different format | `El almacén <carpeta> tiene el formato <versión>, anterior al formato <versión> de esta versión de Krummi.` |
| The store has a newer format | The opening fails without touching the store (D7). | Different format | `El almacén <carpeta> tiene el formato <versión>, posterior al formato <versión> de esta versión de Krummi.` |
| A page is damaged, after opening | The operation fails and returns no data from that page (G6). | Damage | `Página <número> dañada en el almacén <carpeta>.` |
| The disk is full | The operation fails and the transaction applies nothing (G7). | Disk full | `No hay espacio en disco para completar la operación en <carpeta>.` |
| Other system failure when writing or taking data to disk | The operation fails and the store stops accepting writes until it is reopened (G12). | Input/output | `Error de escritura en el almacén <carpeta>: <causa>.` |
| System failure when reading | The operation fails and the store remains usable (G12). | Input/output | `Error de lectura en el almacén <carpeta>: <causa>.` |
| The destination of a copy already exists | The operation fails without writing anything (G10). | Invalid destination | `El destino <destino> ya existe.` |
| The destination of a copy is inside the source | The operation fails without writing anything (G10). | Invalid destination | `El destino <destino> está dentro del almacén <carpeta>.` |
| Encryption is requested in temporary mode (library only) | The opening fails (G9). | Invalid options | — |

The tool writes each message on its own line on the error output.

### Order of the checks on open

When several situations hold, the first one in this list wins:

*Annotation of 2026-10-22: before step 1 the opening options are checked; invalid options win over everything else
and the opening fails without touching anything (flow of [pager.prd.en.md](pager.prd.en.md)).*

1. The folder can be used.
2. The store is not already open.
3. The folder contains a Krummi store, that is, its identification mark is intact (D10), or it is empty, and
   then the library creates one.

   *Annotation of 2026-10-22: for this step and for the not-a-store folder row, the temporary file left by an
   interrupted creation does not count as an entry when it is the only content of the folder (decision P8 of
   [pager.prd.en.md](pager.prd.en.md)).*
4. What remains readable without a key (G8), including the format version, passes its integrity check (D10).
5. Its format is that of this version. A store of another format that also needs recovery is rejected without
   recovering (D7 prevails over G5).
6. With encryption, the key is correct (child PRD 9).
7. The rest of what is needed to open it is not damaged.
8. If it was closed by a crash, the recovery finishes (G5). Damage in what was already committed is reported as a
   damaged store.

## Output

Outside the store's folder, the library writes only to the destinations given by the caller (G10), and writes
nothing to the screen.

Common conventions of the command-line tool; child PRD 12 sets the exact output of each command:

- Results go to standard output and errors to the error output.
- Exit codes: `0` success; `1` the operation failed, including a verification that finds damage; `2` incorrect
  usage (invalid command or options).
- Error example, with the store open by an application:

  ```text
  El almacén D:\datos\pedidos ya está abierto.
  ```

  Exit code: `1`.

## Reuse

None: this is the first PRD of the product. Voron is the behavior reference; its code is neither reused nor
translated, and its format is not reused (D1).

## Out of scope

- Compatibility with Voron or RavenDB files (D1).
- Network server, remote protocol, query language or SQL.
- Replication, distribution and clusters.
- Access to the same store from several processes, including reading (D6).
- Opening a store in read-only mode, including stores on read-only media.
- macOS, 32-bit systems, ARM64 and Linux with musl (D3).
- Network or synchronized file systems (NFS, SMB, OneDrive folders or similar).
- Disks that ignore the commands to take data to the physical medium (G2).
- Bindings for other languages (C API, .NET, Python).
- An asynchronous Rust API.
- RavenDB's own features on top of Voron: documents, full-text indexes, merging of transactions from several
  clients.
- Transactions that span several stores.
- Management and custody of encryption keys, and changing or rotating the key of an existing store.
- Secure erasure of deleted data.
- Integration with external logging, metrics or tracing systems.
- Copying the files of an open store as a way of backing up: the backup is made with the library or the tool.
- Migration between formats before version 1.0, and migration to an older format in any version (D7).
- License and publication of the crate: decided before the first publication, in another PRD.

## Left to the implementation

- The compression and encryption algorithms and the internal organization of the pages, as long as they meet the
  guarantees and the thresholds of the comparative benchmark.
- The English text of the library's errors (D8).

## Acceptance criteria

They apply to the complete product. Each child PRD checks at library level those that concern its module, from
module 1, and adds its own. Child PRD 12 also checks at tool level AC8, AC9, AC10, AC15 and AC17.

1. **AC1** — Given a store in durable mode, when the commit of a transaction ends without error and then the
   process is killed, then on reopening the store the changes are present (G2, G5).
2. **AC2** — Given a store in durable mode, when the commit of a transaction ends without error and then a power
   cut is simulated (everything not taken to disk is lost, and the last write of each file may be left
   half-done), then on reopening the changes are present (G2).
3. **AC3** — Given a store in durable mode, when the process is killed with an open write transaction that has
   not started to commit, then on reopening none of its changes are present and all those of the transactions
   committed before are (G1, G5).
4. **AC4** — Given a store in durable mode, when a crash is provoked, of the process and simulated power, at each
   point where the commit of a transaction writes or takes data to disk, then on reopening all its changes are
   present or none, never a part, and all those of the transactions committed before (G1, G5).
5. **AC5** — Given a recovery that is interrupted at each of its write points, when the store is opened again,
   then the result is the same as that of an uninterrupted recovery (G5).
6. **AC6** — Given an open read transaction, when another write one is committed, then the read one keeps seeing
   the previous data and a new read sees the new data; and while there are open readers, the write starts and
   commits without waiting for them (G3).

   *Annotation of 2026-10-23 to AC6: in a store on disk, a new read only sees durable commits; after a deferred
   commit, it sees it when it is durable, not on return (decision E3 of
   [journal-group-commit.prd.en.md](journal-group-commit.prd.en.md)).*
7. **AC7** — Given several threads that ask at the same time for a write transaction, when they are observed,
   then there are never two open at once and, within the waiting limits set by child PRD 2, all end up
   committed (G4).
8. **AC8** — Given an open store, when another process, or the same one, tries to open it, then the opening
   fails with the store-in-use error class; in the tool, with `El almacén <carpeta> ya está abierto.` and exit
   code `1` (D6).
9. **AC9** — Given a store of an older format and another of a newer format, created for the test with their
   format number changed and otherwise valid, when they are opened, then the opening fails with the
   different-format error class, the tool shows the message of the corresponding variant with exit code `1`, and
   the store's files are left byte-for-byte identical (D7).
10. **AC10** — Given a small test store, when each of its bytes is altered separately and the whole store is
    opened and read, then each alteration produces an error of the damage class or no observable difference in
    the data read, except those of the identification mark, which produce the not-a-store error; never data
    different from what was written (G6, D10). Each child PRD repeats it with the structures it adds.

    *Annotation of 2026-10-22 to AC10: an alteration in the last commit of the journal may also cause it to be
    discarded without error (decision J9 of [journal.prd.en.md](journal.prd.en.md)).*

    *Annotation of 2026-10-23 to AC10: with the group write, that discard holds for an alteration in the last group
    of commits written together (decision E9 of [journal-group-commit.prd.en.md](journal-group-commit.prd.en.md)).*
11. **AC11** — Given a disk with no space, when a write transaction does not fit, then it fails with the
    disk-full error class, applies no change, the store still allows read transactions and, when space is freed,
    a following write commits without reopening (G7).
12. **AC12** — Given a simulated failure when taking data to disk, when the commit fails, then the error is of
    the input/output class, the following writes fail, reads work and, on reopening, the store is recovered
    (G12).
13. **AC13** — Given encryption on, when known values are written and the store is closed, then none of those
    values appears in the clear in any file of the store's folder, of its backups or of the temporary folder,
    which the test replaces with its own folder; and when it is opened with a wrong key, the error is not of the
    damage class (G8).
14. **AC14** — Given a store in temporary mode, when it is used and closed, then the content of the temporary
    folder, which the test replaces with its own folder, and of the working folder is the same as before opening
    it, and when another one is opened in temporary mode it is empty (G9).
15. **AC15** — Given an operation that produces a copy (backup, restore, compaction, migration or data rescue)
    on a source that does not need recovery, when it fails or is interrupted, then the source is left
    byte-for-byte identical and no result is left under the destination's name; and when the destination
    already exists or is inside the source, it fails with the invalid-destination error class without writing
    anything (G10).
16. **AC16** — Given a store creation that is interrupted, when the folder is opened again, then a complete store
    is opened or a new one is created as in an empty folder (flow, step 1).
17. **AC17** — Given a store closed by a crash, when a backup command of the tool is run on it, then the store is
    left recovered and the backup contains the committed transactions; and when verification or data rescue is
    run, the store is left byte-for-byte identical (D9).
18. **AC18** — The tests of each child PRD pass on Windows x64 and on Linux x86-64 with glibc (D3).
19. **AC19** — The comparative benchmark is run with the command set by its child PRD and produces on Windows
    and on Linux a report with the same structure, the same engines, the same workloads and the same fields, in
    which Krummi meets the thresholds in force (D5).
20. **AC20** — Given a read of any data structure, when it returns a value, then the reference points to engine
    memory (the file projection, or the transaction's memory if the page is encrypted or compressed) and not to a
    copy made for that read; and using the reference after closing the transaction does not compile (D11). Each
    child PRD with reads checks it.
21. **AC21** — Given the engine's code, when it is compiled, then an automatic check fails if there is unsafe code
    outside the Pager and its encrypted variant (D12).

## References

- Voron, in the RavenDB repository: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron>
- Repository methodology: `docs/methodology/`

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| c37552ac | 2026-10-22 10:08 UTC+01:00 | 1 h 9 min | 0 min | 1 h 9 min | 35 min | 34 min | 22 |

<!-- prd-time: [{"session": "c37552ac-2871-4449-abde-3b084495ba67", "start": "2026-10-22T09:08:44.181000+00:00", "wall": 4160, "discarded": 0, "agent": 2113, "human": 2047, "interventions": 22, "idle_minutes": 10}] -->
