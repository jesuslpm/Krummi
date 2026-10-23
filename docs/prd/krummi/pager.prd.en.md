# Krummi Pager — PRD

**Version**: 1.0
**Date**: 2026-10-22
**Parent document**: [Krummi, an embedded storage engine inspired by Voron — PRD](krummi.prd.en.md) (v1.1)

> English translation of [pager.prd.md](pager.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Build the **Pager**, module 1 of the [parent's split](krummi.prd.en.md#split-into-child-prds): the base on
which all the others rest. It gives Krummi its data file divided into pages, the recognition and format
version of a store, its atomic creation, the exclusive lock, file growth, the detection of damaged pages and
the temporary in-memory mode.

When this module is closed, the integrating developer can create, open, query and close stores, with the
opening checks of the parent that already exist in this module (see [Flow](#flow)). They cannot store data yet:
that arrives with transactions (module 2) and the structures that come after.

**Why now**: all modules read and write pages through the Pager, and the parent leaves here the decisions that
fix the on-disk format (page size, store files, identification mark).

This PRD inherits all the decisions (D1–D12), guarantees (G1–G12), validations and acceptance criteria of the
parent; only what is specific to the module is added here. The clarifications and exceptions to parent rules are
declared in its decisions and annotated in the parent (see [Annotations to the parent](#annotations-to-the-parent)).

### Product decisions

| ID | Decision |
|---|---|
| P1 | **Public surface of the module**: open or create a store (in a folder or in temporary mode), query its information and close it. Reserving, reading and writing pages is internal to the library: it is not part of the public contract. |
| P2 | **Page size: 8 KiB** (8,192 bytes), fixed for all stores and not configurable. |
| P3 | **Files of a store**: a single file, `krummi.data`, in the store's folder, which also contains its two copies of the header (format version and what is needed to open it). In this module the header is only written when the store is created, so its two copies are equal; modules 2 and 3 fix when it is rewritten, keeping P6. Later modules, such as the journal (3), add their own files. `krummi.data` and `krummi.data.new` (P8) are reserved names. |
| P4 | **Identification mark** (D10 of the parent): a fixed sequence of bytes at the start of `krummi.data`, in a single place and outside the header copies. A folder contains a Krummi store when it has a `krummi.data` file that starts with the intact mark. |
| P5 | **Format version**: the current one when this PRD was written is `1`. An intact store with a lower number than the current one is of an older format, including `0`, which no version uses and only appears in tests; with a higher one, of a newer format. It is a clarification of D7 of the parent, annotated in it. |
| P6 | **Double header**: each header copy carries its own integrity check. On opening, the most recent intact copy is used. If only one copy is damaged, the store opens without error with the other, because no damaged data is returned; the damaged copy is not repaired on opening and is overwritten the next time the header is written. If both are damaged, the store is damaged and cannot be opened. It is a clarification of G6 of the parent, annotated in it. |
| P7 | **Foreign files**: in the folder of a store (with a recognized `krummi.data`), files and folders with other names are ignored and are never modified or deleted. A folder without `krummi.data` follows the parent's rule: if it has any entry, including hidden ones, it is not a store. |
| P8 | **Atomic creation**: the library prepares the store in `krummi.data.new`, takes it to disk and only then renames it to `krummi.data`; when the opening ends without error, the new store survives a power cut. If the creation fails, the library deletes `krummi.data.new`; if it is interrupted, it may remain. A folder whose only content is the file `krummi.data.new` is treated as empty: the next opening deletes it and creates the store. In any other case `krummi.data.new` is not touched. The folders that the opening creates because they did not exist (step 1 of the order of opening) are kept even if the creation fails. It is a clarification of the parent's flow and folder validation, annotated in it. |
| P9 | **Lock**: the exclusive lock (D6) belongs to the store and lasts from step 2 of the order of opening until closing, also while it is being created: of several that open the same empty or nonexistent folder at the same time, one creates the store and the others fail with the store-in-use class. The operating system releases it when the process ends, also if it crashes; a crash never leaves a store locked. Locking leaves no more files in the folder than those of P3 and P8. Opening the same store through another path (relative, with different capitalization on Windows, through a link) also fails with the store-in-use class. Closing the store, or ceasing to use it without closing it explicitly, releases the lock. |
| P10 | **Optional maximum size**: on opening, the application can set a maximum size of the store in bytes, which holds while it is open and is not saved in it. It must be at least 1 MiB (1,048,576 bytes), in both modes; if it is not a multiple of 8,192, it is rounded down. Without it, the limit is the disk and the file system. With it, no growth takes `krummi.data` beyond the maximum: a growth jump is trimmed to the maximum, and only fails, with the maximum-size error class, when the requested pages do not fit. The store remains usable. An existing store that is already larger than the maximum opens as is and is read normally, but cannot grow. In temporary mode it limits the memory the store uses. |
| P11 | **Growth**: a new store takes up at most 1 MiB. The size of `krummi.data` is always a multiple of 8,192 and grows in jumps that are never smaller than the previous one and that increase as it grows; it never shrinks by itself (reclaiming space is compaction, module 11). When growing, the space is really reserved on disk, so that lack of space is detected when growing (G7) and not afterwards. A growth that fails leaves the file at the size it had. If an interrupted growth leaves the file with an incomplete final page, the opening ignores it. |
| P12 | **Information of a store**: while it is open, the query always works, also if the store no longer accepts writes (G12), and returns: the format version; the page size; the size of the store in bytes (that of `krummi.data` without the incomplete final page, or in temporary mode the one its pages take up); the number of pages, including those of the header; the effective maximum size after rounding, or that there is none; and whether the store is temporary. |
| P13 | **Out of memory**: if the system does not give the memory the store needs to open or grow, in either of the two modes, the operation fails with the out-of-memory error class; if the store was already open, it remains usable. |
| P14 | **Close**: closing always releases the lock. If taking data to disk fails on closing, the close returns the input/output error class, and the lock is released anyway. |
| P15 | **Reads without copy in the Pager** (D11 of the parent): reading a page returns a reference to the memory mapping of `krummi.data` (or to the memory of the temporary store), without copying it. In this module the reference is valid while the store remains open, and the compiler prevents using it after closing it; module 2 ties it to the read transaction (D11). That `krummi.data` grows in the meantime does not invalidate the references already handed out. Checking the integrity of the page (G6) does not require copying it. |
| P16 | **Unsafe code** (D12 of the parent): the Rust unsafe code that the Pager needs (mapping the file into memory and reading over that mapping) stays inside the Pager, behind a safe interface, and each use carries written the condition that makes it correct. No public operation of the engine can cause undefined behavior, unless another program modifies `krummi.data` while the store is open (see Out of scope). |
| P17 | **Physical read failure**: as in Voron, a disk failure when reading a page through the mapping (an unreadable sector, a disk that disconnects) ends the process instead of returning an error: the system reports it with a signal or a processor exception that cannot safely be turned into an error. On reopening, if the failure persists, it appears on opening or on reading that page. It is an exception to G12 of the parent, annotated in it; G12 still holds for writes and for everything that is not read through the mapping. |

*Annotations of 2026-10-22 from module 3 ([journal.prd.en.md](journal.prd.en.md)):*

- *P3 and P7: the journal files (`muninn.journal.` followed by 19 digits) and the `muninn.temp` subfolder are names
  reserved by the store, which creates and deletes them; other names are still foreign (decision J2).*
- *P5: the current format version becomes `2`; a store of format `1` is of an older format (decision J3).*
- *P10 and P11: recovery grows `muninn.data` as much as the recovered commits need even if it exceeds the maximum
  size requested on opening, and the growth made by a write that was never committed may not survive a crash
  (decision J8).*
- *P17: it also holds for the files of `muninn.temp`, which reads use without copying (decision J7).*

*Annotations of 2026-10-23 from the rename ([rename-to-krummi.prd.en.md](rename-to-krummi.prd.en.md)):*

- *Whole document: the product is called Krummi, its packages and the files of a store take the names of R2 and
  R3, and this document was updated with the new names (decisions R1–R13). The file names and format numbers of
  the earlier annotations are read according to R3 and R5, and the current format is `3`.*
- *P4: the identification mark is replaced by a new one; a `krummi.data` that starts with the MuninnDB mark does
  not start with the mark (decision R4).*
- *P5: the current format version becomes `3`; a store of format `2` is of an older format (decision R5).*
- *P8: a lone `muninn.data.new`, which used to be an interrupted creation and was deleted, is no longer deleted:
  it is a foreign entry and the folder is not a store (decision R6).*

### Starting state

Verified on 2026-10-22:

| Aspect | Situation |
|---|---|
| MuninnDB | There is no code: neither Rust files nor a Cargo workspace. The repository only has the methodology, the tooling and the PRDs. |
| Parent PRD | [muninndb.prd.en.md](krummi.prd.en.md) v1.1, with the annotations of this PRD: it assigns to this module the page size, the store files and the foreign ones, the lock without leftovers, the identification mark, the maximum size and the out-of-memory error in temporary mode. |
| Voron (reference) | 8 KiB pages; header with mark, version, revision and hash, in two copies that alternate (in Voron they are two separate files); data file and journal in a subfolder; optional maximum size; configurable initial size. |

## Actors

- **Integrating developer**: creates, opens, queries and closes stores from their application, and handles the
  errors by their class.

The operator and the command-line tool do not take part in this module (they arrive with module 12).

## Flow

1. The application opens a store indicating a folder, with an optional maximum size, or in temporary mode.
2. The library first checks the options: a maximum size smaller than 1 MiB fails with the invalid-options class
   without touching anything, before any other check.
3. Then it follows the [parent's order](krummi.prd.en.md#order-of-the-checks-on-open), with these clarifications:
   - step 1: if the folder does not exist, it creates it with its intermediate folders;
   - step 2: the lock creates no files (P9);
   - step 3: the folder contains a store according to P4; it is empty if it has no entry or only the file
     `krummi.data.new` (P8), and then the store is created;
   - step 4: at least one copy of the header is intact (P6);
   - step 5: the format version is the current one (P5);
   - step 7: the file contains the header pages complete.

   Steps 6 (key) and 8 (recovery) of the parent apply from modules 9 and 3.
4. The application queries the information of the store (P12).
5. The application closes the store (P14).

In temporary mode only step 2 of this flow and the creation of the in-memory store apply.

## Validations and messages

The parent's hold. This module adds these situations, for the library only (the tool does not open stores
until module 12, which will fix its messages if it shows them):

| Situation | Rule | Error class |
|---|---|---|
| Maximum size smaller than 1 MiB, in any mode | The opening fails without touching anything (flow, step 2). | Invalid options |
| A growth needs more pages than fit in the maximum size | The operation fails and the store remains usable (P10). It is checked before writing, so it wins over disk full. | Maximum size |
| The system gives no memory to open or grow | The operation fails; an open store remains usable (P13). | Out of memory |
| `krummi.data` is a folder, or does not start with the mark (including an empty file or one shorter than the mark) | It is a folder that does not contain a store (parent's row). | Not a store |
| `krummi.data` cannot be read because of permissions | The opening fails without touching anything. | Access |
| `krummi.data` has the mark, but does not contain the header pages complete | The opening fails without touching anything (flow, step 3, clarification of step 7). | Damage |
| A written page is read that is no longer in `krummi.data` because the file was trimmed while the store was closed | The read fails (G6). | Damage |
| A `krummi.data.new` that has to be deleted (P8) cannot be deleted | The opening fails without touching anything else. | Access |
| A system write fails when creating the store | The opening fails; the cleanup of P8 applies. | Input/output |
| Taking data to disk fails on closing | The close reports the error and releases the lock (P14). | Input/output |

## Output

The module writes nothing to the screen. Its public output is the information of a store (P12). Example, for a
store just created on disk without maximum size:

| Datum | Value |
|---|---|
| Format version | the current one (today `1`) |
| Page size | `8192` |
| Size of the store in bytes | multiple of `8192`, at most `1048576` |
| Number of pages | the size divided by `8192` |
| Maximum size | none |
| Temporary | no |

On disk, the store's folder contains only `krummi.data`, besides the foreign files it already had.

## Reuse

None: it is the first module with code. It uses the parent's common error classes and order of opening.

## Out of scope

- Transactions, isolation and a single writer (module 2).
- Journal, durability of commits and recovery after a crash (module 3).
- Reuse of freed pages (module 4).
- Encryption (module 9).
- Any command of the command-line tool, including the verification that reports a damaged header copy
  (module 12).
- A public API to reserve, read or write pages (P1).
- Configurable page size (P2).
- Reducing the size of `krummi.data` (module 11).
- Configurable initial size or growth jumps (P11), and changing the maximum size while the store is open
  (P10).
- Repairing a damaged header copy on opening (P6).
- Checking the integrity of pages that are not read, in the background or on opening.
- Deleting, renaming, moving, trimming or modifying the folder or `krummi.data` from another program while the
  store is open: the result is undefined. On Linux the exclusive lock does not prevent another program from doing
  so.
- Performance thresholds: they are set by the benchmark (module 6, D5 of the parent).

## Left to the implementation

- The bytes of the identification mark and its length, the layout and size of the header, how it is decided
  which copy is the most recent and the algorithm of the integrity check.
- The exact size of a new store (up to 1 MiB) and of each growth jump (P11).
- How the lock is taken, as long as P9 is met.
- The names of the types, functions and fields of the public API, which follow Rust conventions and are in
  English.
- How the tests simulate write failures, lack of space and of memory, and interruptions.

## Acceptance criteria

The criteria that need to reserve, write or read pages (AC-P10, the last part of AC-P11, AC-P12,
AC-P13, AC-P14, AC-P15, AC-P17 and the adapted ones of the parent) are internal tests of the library over its
internal page API, visible to the tests; those that alter the header use the constants of the implementation
itself.

This module checks at library level these criteria of the parent, adapted to what exists in it:

- **AC8**: as is.
- **AC9**: with valid test stores whose format number is `0` (older) and the current one plus one (newer).
- **AC10**: altering each byte of a small store in which the test has written pages, and then reading all the
  written pages; alterations of a single header copy make no difference (P6).
- **AC11**: with a growth instead of a transaction; the pages already written keep being read and, when space is
  freed, the next growth works.
- **AC12**: with a failure when writing pages; later writes fail with the input/output class, page reads and the
  information query work, and on reopening the store opens and the pages written before the failure are read
  (recovery arrives with module 3).
- **AC14**: temporary mode creates no files or folders and keeps nothing when closed.
- **AC16** and **AC18**: as is.
- **AC20**: reading a page returns a reference inside the mapping of `krummi.data` (or of the temporary store's
  memory), and using it after closing the store does not compile (P15), with a compile-failure test over the
  internal API.
- **AC21**: given unsafe test code outside the Pager, the automatic check fails; over the engine's code, it
  passes (P16).

And in addition:

1. **AC-P1** — Given a folder that does not exist, with intermediate folders that do not exist either, when it is
   opened, then they are created, the folder contains only `krummi.data`, and the information says the current
   format, page `8192`, a size that is a multiple of `8192` of at most `1048576` bytes, a number of pages equal
   to the size divided by `8192`, no maximum size and not temporary.
2. **AC-P2** — Given an empty folder, and another whose only content is the file `krummi.data.new`, when they are
   opened, then in both only `krummi.data` remains with a new store.
3. **AC-P3** — Given a folder without `krummi.data` with only a foreign file (for example `.gitkeep`), and another
   with `krummi.data.new` and a foreign file, when they are opened, then they fail with the not-a-store error
   class and the folders remain identical byte for byte.
4. **AC-P4** — Given a store created and closed, when it is opened again, then it opens without error and its
   information is the same.
5. **AC-P5** — Given a store opened by a process, when that process is killed, then another process opens it
   without error; and given a store that the process stops using without closing it explicitly, when it opens it
   again, also.
6. **AC-P6** — Given two processes, and two threads of the same process, that open at the same time the same
   folder that does not exist, when they finish, then one opens the store, the other fails with the
   store-in-use class, and the folder contains only `krummi.data`; and given an open store, when it is opened
   through another path that reaches the same file, then it fails with the store-in-use class.
7. **AC-P7** — Given a store with foreign files and folders in its folder, when it is opened, used and closed,
   then it opens without error and the foreign ones remain identical byte for byte.
8. **AC-P8** — Given a store, when one of the two header copies is damaged without touching the mark, then it
   opens without error and its information is the same; and when both are damaged, the opening fails with the
   damage error class and the store remains identical byte for byte.
9. **AC-P9** — Given a creation interrupted at each of its write points, by the process and by a simulated power
   cut, when the folder is opened again, then only `krummi.data` remains with a complete store; and given a
   creation that ends without error followed by a simulated power cut, when it is reopened, then the store is
   complete.
10. **AC-P10** — Given a store with a maximum size of 1.5 MiB, when pages are written until it is full, then
    `krummi.data` reaches exactly 1.5 MiB and the next page fails with the maximum-size class, the file keeps
    its size and the pages already written are read; and after closing and reopening without maximum size,
    growth works.
11. **AC-P11** — Given a maximum size smaller than 1 MiB, on disk and in temporary mode, when it is opened, then
    it fails with the invalid-options class without creating anything; given a maximum size that is not a
    multiple of `8192`, the information shows the one rounded down; and given an existing store larger than the
    requested maximum size, when it is opened, then it opens and reads normally, and the first growth fails with
    the maximum-size class.
12. **AC-P12** — Given a store in which pages are written until it grows at least three times, when its size is
    observed, then it is always a multiple of `8192`, the third jump is larger than the first, and after closing
    and reopening the size has not decreased.
13. **AC-P13** — Given a `krummi.data` to which an incomplete final page is added, when it is opened, then it
    opens without error and its information ignores that page; given a `krummi.data` trimmed within the header
    pages, the opening fails with the damage class; and given one trimmed right after the header, it opens and
    reading a written page that was lost fails with the damage class.
14. **AC-P14** — Given a temporary store, when it is opened, then its information says temporary, current format
    and page `8192`; with a maximum size, a growth that does not fit fails with the maximum-size class; with a
    simulated memory failure, it fails with the out-of-memory class; and in both cases it remains usable. A
    simulated memory failure when growing a store on disk also fails with the out-of-memory class and leaves it
    usable.
15. **AC-P15** — Given a disk without space, when a store is created, then it fails with the disk-full class and
    neither `krummi.data` nor `krummi.data.new` remains; and given an open store and a disk without space, when
    it needs to grow, then it fails with the disk-full class and the file keeps its size.
16. **AC-P16** — Given a simulated failure when taking data to disk during the close, when it is closed, then it
    returns the input/output class and the store can be opened again.
17. **AC-P17** — Given a reference to a written page, on disk and in temporary mode, when the store grows several
    times while it is kept, then the reference points to the same address and its content is the same (P15).

## Annotations to the parent

This PRD clarifies six points of the parent, which carries the corresponding annotations:

- G12: a physical read failure over the mapping ends the process, as in Voron (P17).
- D12: the undefined-behavior exception when another program modifies `muninn.data` while the store is open
  (P16).
- G6: a single damaged header copy does not prevent opening (P6).
- D7: a format number lower than the current one, including `0`, is of an older format (P5).
- Order of opening: the options are checked before step 1 and win over everything else (flow, step 2).
- Step 1 of the flow, step 3 of the order of opening and the row for a folder that is not a store: the
  `muninn.data.new` file left by an interrupted creation does not count as a foreign entry when it is the only
  content of the folder, and the next opening deletes it (P8).

## References

- Parent and root: [krummi.prd.en.md](krummi.prd.en.md) v1.1
- Voron, in the RavenDB repository: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron>

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| c37552ac | 2026-10-22 10:08 UTC+01:00 | 1 h 9 min | 0 min | 1 h 9 min | 35 min | 34 min | 22 |

<!-- prd-time: [{"session": "c37552ac-2871-4449-abde-3b084495ba67", "start": "2026-10-22T09:08:44.181000+00:00", "wall": 4160, "discarded": 0, "agent": 2113, "human": 2047, "interventions": 22, "idle_minutes": 10}] -->
