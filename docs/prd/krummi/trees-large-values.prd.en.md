# Krummi Large Values — PRD

**Version**: 1.0
**Date**: 2026-10-27
**Parent document**: [Krummi B+ Trees — PRD](trees.prd.en.md) (v1.0)

## Objective and context

Build child PRD **5.3** of the [split of the trees module](trees.prd.en.md#split-into-child-prds): values of up to
256 MiB in normal trees, which are stored, read, scanned and removed with the same operations of 5.1 and 5.2, and
which are returned without copying as a single sequence of bytes, also in temporary mode.

When this change closes, an application can store in an entry a document, an image or any value of up to
268,435,456 bytes, and not only of up to 1,024. As in Voron and in LMDB, a value that does not fit in its leaf page
is stored in contiguous pages inside the store itself, and Read returns a reference to those pages, without copying
them.

**Why now**: it is the third submodule of the split. The tables (module 7) store their rows as tree values, and a row
of several KiB does not fit in the provisional maximum of 1,024 bytes of 5.1.

**What is optimized**: the design prioritizes values of up to a few MiB. Larger values, up to the maximum of
256 MiB, are supported and tested, but not optimized (V10). Values of arbitrary size, read and written in parts, are
the streams (module 8).

This PRD inherits everything from its ancestry, the [trees module](trees.prd.en.md) (B1–B12) and the
[root](krummi.prd.en.md), and what the parent inherits from the earlier modules: the [Pager](pager.prd.en.md)
(P1–P17), the [Transactions](transactions.prd.en.md) (T1–T18), the [Journal](journal.prd.en.md) (J1–J17), the
[grouped and direct journal writing](journal-group-commit.prd.en.md) (E1–E16) and the
[Free space](free-space.prd.en.md) (F1–F14). It also reuses what its siblings
[Trees and Key Operations](trees-keys.prd.en.md) (A1–A15 and its annotation) and
[In-Order Scan and Prefix Search](trees-scan.prd.en.md) (R1–R11) set. Only what is specific to 5.3 is added here.
What changes or refines earlier rules is declared in its decisions and annotated in the affected documents (see
[Annotations to other PRDs](#annotations-to-other-prds)).

In this document, an **off-leaf value** is a value that is not stored in the leaf page of its entry, but in pages of
its own in the store; its entry stays in the leaf, with its key.

### Product decisions

| ID | Decision |
|---|---|
| V1 | **Maximum size of a value** (B6): 268,435,456 bytes (256 MiB), for any valid key, also one of 2,048 bytes. It replaces the provisional maximum of 5.1 (A11). With B8, the maximum that an invalid size error reports for a value in a normal tree becomes `268435456`. Adding on a key that is already there changes nothing (A4, A10), so it does not reserve room for the value either and does not fail with out of memory, maximum size or disk full because of its size. |
| V2 | **Contiguous zero-copy read** (refines D11 of the root parent and B10): Read and the scans return every value, inside its leaf or off it, as a single contiguous sequence of bytes, with the shape and lifetime of today's references. The reference points to engine memory according to T16, also to a version kept in the versions area of `krummi.temp` (J7), or, in a write, to its own copy of the value, as today; never to a copy made for that read. It holds on disk and in temporary mode, where memory grows in separate chunks. The bytes of a reference do not change while it is valid, even if a later write reuses the pages of the value (G3). |
| V3 | **Where an off-leaf value lives**: in contiguous pages of the store itself, in `krummi.data` or in the memory of the temporary store. The change adds no files to the store (P7). The numbers that the format stores for an off-leaf value, such as its page and its length, are stored in little endian, like the rest of the format, never in the processor's native order, so they do not depend on it. |
| V4 | **Leaf threshold**: from which size a value is stored off its leaf remains left to the implementation (parent, *Left to the implementation*). The execution evaluates at least two thresholds: one that lets a single entry fill its leaf and another that keeps at least two entries per leaf. It compares them with the data of the measurement (V11), by performance, space used and the complexity they add, chooses the most balanced one and leaves the evaluation and the choice in the closing report. |
| V5 | **Space** (F8): an off-leaf value of n bytes takes up at most one page more than the ones its bytes would fill, that is, n divided by 8,192 rounded up, plus one; no other page of the store stays retained by it. When it is overwritten with another value, the pages that the new one does not take up are freed (F8) in that same write and become available according to F6. The pages of an off-leaf value return to the free space when the value stops being in the tree: when it is overwritten with Store, when it is removed with Remove or Remove range, when the tree is emptied (A2), when it is deleted and when it is overwritten with Rename (A3). An emptied tree takes up the same as a newly created empty one (A2), and one left with no entries by Remove range, the same as an emptied one (R7), also if it had off-leaf values. |
| V6 | **Contiguous pages and growth** (F5): an off-leaf value needs its pages one after another within a region; the maximum of V1 always fits in one. If there is no free gap of the necessary size, the value is stored where F5 says, even if the store has enough scattered free pages: there is no fragmented space error. Growth and its failures follow F5, P10, P11, P13 and G7. |
| V7 | **Damage in an off-leaf value** (G6, A9): the engine never returns a value with any damaged byte. Read, and a scan that needs that entry, fail with the damage class and the number of a page of the value, and fail again with that class every time they need it; at which step of the scan the failure appears follows what 5.2 left to the implementation. No operation that removes, overwrites or frees a value reads its pages: Store and Add on that key, Remove, Remove range, emptying the tree (A2), deleting it and overwriting it with Rename (A3). Damage in the pages of the value does not make them fail, so the application can always remove or replace a damaged value, and empty or delete its tree; they can fail by damage in the other pages of the tree, as A9 says. The other keys of the tree are not affected. |
| V8 | **Errors**: there are no new classes. The failures of T11 apply in the operations and in the commit, also out of memory (P13) or disk full (G7) when committing a write with off-leaf values. Reading a value does not reserve memory, so it does not fail with out of memory because of its size. |
| V9 | **Format `7`**: the change is incompatible (B12): an engine of format `6` does not know off-leaf values, would read their reference as if it were the value and, when freeing or reorganizing pages, could reuse those of the value. The current format version becomes `7`, and a store of format `6` is of an earlier format even if it is empty; before 1.0 there is no migration and the data is recreated (D7). |
| V10 | **Very large values**: the maximum of V1 is supported and tested (AC-V1), but not optimized. For them it is accepted that the value is written twice, in the journal and in `krummi.data`, and the memory that the commit needs to write it in the journal. The measurement (V11) does not include values of more than 8 MiB. |
| V11 | **Measurement** (B12, no threshold by D5): the change includes a reproducible measurement, on Windows and on Linux, on a store on disk with durable commit, with random keys of 16 bytes and values of random bytes, which do not compress, both with a fixed seed. For each value size of 1, 2, 3, 4, 6 and 8 KiB, 64 KiB, 1 MiB and 8 MiB, in a new store with 256 MiB of values in total: storing them at random, in writes of 16 MiB of values (16,384 entries per write with 1 KiB, 2 with 8 MiB); after closing and reopening it, without emptying the system cache, reading them all at random in a single read transaction, reading the bytes of each value returned; and the size of the store in bytes that its information gives (P12) after the last commit of the load. In addition, in another new store, a mixed load of 1 million keys, 90% with values of 100 bytes and 10% with values of 2 to 8 KiB, with the size chosen at random with the same probability for each number of bytes in that interval and the large entries spread at random among the others, in writes of 1,000 entries, which measures storing them, reading them all at random, scanning the whole tree forward and the size of the store. The report includes the operating system, the processor and the disk. Its results go in the closing report and feed the evaluation of V4. |

### Starting state

Verified on 2026-10-27 in the code:

| Aspect | Situation |
|---|---|
| Public API | Normal trees have the operations of 5.1 and the scans and Remove range of 5.2. Read and the scans return each value as a zero-copy reference to a single sequence of bytes, inside a page. |
| Values | They always live in the leaf page of their entry; the maximum is 1,024 bytes (A11), chosen so that at least two entries of the maximum size fit in each leaf. There are no off-leaf values. |
| Pages | Pages of 8,192 bytes. Each page carries its own integrity check and its number in its first 16 bytes, also each page of a range reserved at once, so today the bytes of a range of pages are not contiguous. The integrity of a page is checked every time it is read from `krummi.data`. No PRD fixes the algorithm of the check or that each page carries its own (Pager, *Left to the implementation*). |
| Memory on disk | `krummi.data` is projected entirely into memory; when it grows, it is projected again completely and the earlier projections are kept until closing, so the references already delivered stay valid. A range of pages is contiguous in the most recent projection. |
| Memory in temporary mode | It grows in separate chunks that never move, each of the size of the growth. A page is never split between two chunks, but a range of pages can be. |
| Writes and versions | A write keeps in memory its copy of each page that it modifies or reserves, page by page. The committed versions that are not yet in `krummi.data` are kept page by page in `krummi.temp` (on disk) or in memory (in temporary mode). Neither guarantees that the pages of a range stay contiguous. |
| Journal | One record per commit, with the pages changed, compressed when it takes up less; it has no maximum size per commit. |
| Free space | A range reserved at once has at most 65,407 pages, about 511 MiB, and never crosses from one region to another (F5). |
| Errors | Sixteen classes, those of the starting state of 5.2. Invalid size reports name, key, value or prefix. |
| Format | The current format version is `6` (A13). All the engine's numbers on disk (page numbers, lengths, integrity checks) are stored in explicit little endian, not in the processor's native order; no PRD fixes it as a rule. There is a store of format `6` created with the version of 5.1 and kept as test data, which 5.2 opens and scans (AC-R14). |

## Actors

- **Integrating developer**: stores, reads, scans and removes entries with values of up to 256 MiB, and handles the
  errors by their class.
- **Maintainer**: runs the measurement of V11 on Windows and on Linux and reviews the evaluation of V4.

## Flow

1. The application opens a transaction and a normal tree (A1).
2. In a write it stores or adds entries with values of any size up to the maximum (V1).
3. In any transaction it reads or scans them, and receives each value as a single sequence of bytes without copying
   (V2).
4. In a write it overwrites or removes them, and their pages return to the free space (V5).
5. The commit applies all the changes, or none (G1).

## Validations and messages

Those of the ancestors and of 5.1 and 5.2 apply, including the failures of T11 and T12, with the precedence of A7 and
R8. This PRD only changes the maximum size of a value (V1) and adds one damage situation, in the library only (the
tool does not open stores until module 12):

| Situation | Rule | Error class |
|---|---|---|
| A value of more than 268,435,456 bytes, when storing or adding | V1, B6 | Invalid size, as a value |
| Read, or a scan, needs an entry whose off-leaf value has a damaged page | V7, G6 | Damage, with the number of a page of the value |

## Output

The module writes nothing to the screen. The store information (P12) keeps its data; the format version it reports
becomes `7` (V9). Read and the scans return the stored bytes, unchanged, in a single sequence, whatever their size.

Example of the data of an invalid size error, for a value of 268,435,457 bytes in a store on disk:

| Datum | Value |
|---|---|
| Folder | the store's |
| What violates its size | value |
| Size received | `268435457` |
| Minimum | `0` |
| Maximum | `268435456` |

## Reuse

- The common rules of the trees module (B1–B12) and those of 5.1 and 5.2: handles, key operations, scans, Remove
  range, errors, precedence, damage, identity and changes.
- The reservation of ranges of contiguous pages of the free space and their freeing (F5, F8).
- The transactions, their snapshot, the retained versions and the zero-copy reads (T2, T14, T16, D11).
- The journal and the recovery (G1, G5): off-leaf values travel like any other change.
- The integrity check and the damage class (G6).
- The simulation of crashes, power cuts and failures of the tests of modules 3 to 5.2.
- The store of format `6` kept as test data, which must now be rejected as an earlier format (V9).

## Out of scope

- Storing very large values in files separate from the store: it is evaluated with the streams (module 8).
- Optimizing values of more than a few MiB (V10): keeping them from going through the journal, or writing them
  without the commit holding them whole in memory; and latency limits for very large commits (D5).
- Reserving the room for a value and writing it afterwards in its place, without passing its bytes to Store (what
  Voron calls `DirectAdd`), and storing a value from several chunks.
- Reading the size of a value without reading it, reading only a part of it, or modifying or extending a part of a
  stored value.
- Knowing whether an entry has its value off its leaf, and statistics of the values of a tree (storage report,
  module 12).
- Compressing the values (dictionary value compression, module 7; leaf page compression, 5.6) and encrypting their
  pages (module 9, which fixes how they are returned without copying).
- Large values in multi-value and fixed-size trees (5.4 and 5.5 set their maximums).
- Joining free gaps to obtain contiguous room, or moving values to compact `krummi.data` (compaction, module 11).
- Migrating a store of format `6` (D7).
- Inspecting or verifying values from the command-line tool (module 12).

## Left to the implementation

- The leaf threshold, with the evaluation that V4 requires.
- How an off-leaf value is represented in its entry and in its pages, and how its integrity is checked, as long as it
  meets G6 and V7: whether it carries one check per page or one for the whole value, and whether it is checked on
  every read or once per transaction (that another program modifies the files of the open store is out of scope,
  P16).
- Which page of the value is reported in a damage error (V7), as long as it is one of its own, also when several are
  damaged or when the operation is repeated.
- How an off-leaf value is made contiguous in memory in temporary mode, in the write's copy and in the versions that
  are kept outside `krummi.data` (V2).
- Whether overwriting an off-leaf value reuses part of its pages or reserves others, with the same number of pages or
  another, within V5; near the maximum size, one way may fit and another fail with maximum size, because a write does
  not reuse the pages that it frees itself (F6).
- Which failure of T11 is reported when a single operation with a large value would cause several (out of memory,
  maximum size or disk full), according to what it tries first; in all cases it follows T11.
- In temporary mode and in the versions that are kept outside `krummi.data`, how much extra memory the engine
  reserves so that an off-leaf value is contiguous. That memory counts toward the maximum size (P10, T14), so near
  the limit a value may fail with maximum size in one mode and fit in the other with the same maximum. In temporary
  mode, the part of it that belongs to the store's memory counts in its information as pages, and as free while
  nothing uses them (F10).
- The command that runs the measurement of V11 and the shape of its report.

## Acceptance criteria

The test conditions of the earlier modules and the header of the parent's criteria apply: each criterion runs on a
store on disk and in temporary mode, except the parts that reopen the store or damage its files, which are on disk
only. This PRD checks the parent's criterion assigned to 5.3 (AC-B3 with 256 MiB, in AC-V1), and AC-B17 and
AC-B18 with off-leaf values (AC-V6 and AC-V8); it repeats AC10 and AC20 of the root with off-leaf values
(AC-V11 and AC-V3), and adds these. The tests that depend on where a value is stored, on where its reference is, on
damaged pages or on failures at specific points use knowledge of the implementation itself.

1. **AC-V1** — Given a write, when it stores with a key of 2,048 bytes an empty value and another of 268,435,456
   bytes, then they are read identical after committing and, on disk, also after reopening, and a scan of the tree
   returns them; when the one of 268,435,456 bytes is overwritten with one of 1 byte and then removed, then Read
   returns the new value and afterwards that the key is not there; and when it tries to store or add one of
   268,435,457 bytes, then it fails with the invalid size class without storing anything, as a value, with the size
   received, minimum `0` and maximum `268435456`, and the write stays usable (V1, AC-B3).
2. **AC-V2** — Given a tree, when values of 1,024 and 1,025 bytes, of 1 MiB and 8 MiB, and of one byte less, equal
   and one more than each of these limits, which the tests take from the implementation (the leaf threshold and the
   largest value that fits in 1, in 2 and in 3 pages off the leaf), are stored with keys of 1 and of 2,048 bytes,
   then Read returns them identical in the same write, in a later read and, on disk, after reopening; and when they
   are overwritten with larger, smaller and empty values, added on existing keys and removed, then Read always
   returns the last value stored or that the key is not there, and the number of entries matches the keys that Read
   finds (V1, V2, A4, A5).
3. **AC-V3** — Given an off-leaf value, when it is read, then the reference is inside the engine memory that T16
   fixes and not in a copy, in each of these cases: on disk, a version already written in place in `krummi.data`,
   inside the projection of `krummi.data`; on disk, a committed version that has not yet been written to
   `krummi.data`, and an earlier version that a read still sees after a later write overwrote the value, inside the
   versions area of `krummi.temp` where the engine keeps it (J7, T14, T16); in temporary mode, those same cases
   inside the store's memory or the memory where the engine keeps the versions; in temporary mode, a value stored
   when the store has to grow and whose range, with consecutive pages, would cross the boundary between the memory
   that the store already had and the one it adds when growing; and in a write that reads the value it has just
   stored, inside its own copy. In all cases the value is returned as a single contiguous sequence, with bytes
   identical to those stored, and using the reference after the transaction ends does not compile (V2, AC20).
4. **AC-V4** — Given a tree with values inside and off their leaf, when it is scanned forward, backward and with a
   prefix, then each entry comes out with its complete and identical value, as a single sequence without copying; and
   when intervals that contain off-leaf values are removed, then Remove range returns how many entries it removed and
   a scan of the whole tree returns the others with their values (V2, R1, R7).
5. **AC-V5** — Given a store with a tree already created, which stays within a single region of the free space (with
   values whose size does not force a jump of region, F5), taking the pages in use as in AC-A15: when an empty tree
   is created, committed, the value is taken, values inside and off their leaf are stored in it, overwritten with
   larger and smaller values, committed and then emptied, then the final value is the one taken with the empty tree;
   the same when all its entries are removed with Remove range; when the tree is deleted or overwritten with Rename,
   the final value is the one from before creating it when its entry did not make the catalog grow (the tests know it
   from the implementation), as in AC-A15; and when an off-leaf value of 1 MiB is stored, committed, overwritten with
   one of 64 KiB that is still off its leaf and committed, then each time the pages in use grow, with respect to the
   tree without that value, by at most the pages that its bytes would fill, rounded up, plus one, and those that grow
   because of the leaf entry (V5, A2, A3, R7, F10).
6. **AC-V6** — Given a read transaction open with references to off-leaf values, and a write that overwrites and
   removes those values, and splits and merges leaves with values inside and off them until the height of the tree
   changes, when it commits and another write reuses the freed pages (F6), then the bytes of the read's references do
   not change, the read keeps reading and scanning exactly the values of its snapshot, and a new read sees those of
   the write (V2, G3, T14, T16, AC-B17).
7. **AC-V7** — Given an off-leaf value with one byte altered in its first, in an intermediate and in its last page, in
   separate tests, when it is read or a scan needs its entry, then it fails with the damage class and the number of a
   page of the value, and fails the same way when repeated; the entries that the scan returned before are correct and
   the other keys are read without error; when a write stores on that key, removes it or removes an interval that
   contains it, then it does not fail, it commits and Read returns the new value or that the key is not there; when it
   adds on that key, then it returns that it did not add without failing, and Read keeps failing with the damage
   class; and when a write empties the tree, deletes it or overwrites it with Rename, then it does not fail and it
   commits (V7, A9, G6, R8).
8. **AC-V8** — Given a write that stores, overwrites and removes off-leaf values, when a process crash and a simulated
   power cut are caused at every point where its commit writes or carries data to disk, then on reopening each key has
   its value from before or the one from after, all at once, and the number of entries matches; when a T11 failure
   other than input and output is simulated when writing at every point where an operation reserves, reads or frees
   pages, also when reserving the range of a value, then committing fails with the class of the first failure and
   applies nothing, and after aborting the write the tree and its number of entries are as before and a new write
   works without error; and when disk full or out of memory is simulated during the commit itself, then it fails with
   that class, ends the transaction without applying anything and a new write works without error (V3, V8, G1, G5,
   T11, AC-B18).
9. **AC-V9** — Given a store with enough scattered free pages for a value but without a contiguous gap of its size,
   and whose end is at the boundary of the free space (the test prepares it knowing the implementation), when that
   value is stored and committed, then it is stored, the number of pages of the store (P12) grows by at least the ones
   the value takes up and the scattered free pages stay free; and with a maximum size that does not let it grow
   enough, then the operation, or at the latest the commit, fails with the maximum size class, applies nothing and a
   new write works without error (V6, F5, T11).
10. **AC-V10** — Given a write on disk that only does Add with a value of 8 MiB on an existing key, Remove on keys that
    are not there and operations that fail by invalid size with a value of 268,435,457 bytes, when it is committed,
    then it ends without error and the store's files are left identical byte for byte; Add with a value of
    268,435,456 bytes on an existing key, with a maximum size that leaves no room for it, returns that it did not add
    without failing; and a value of 268,435,457 bytes through an opening of a deleted tree, or in a write that already
    had a T11 failure, fails with invalid size (V1, A7, A10, B7, T10).
11. **AC-V11** — Given a small store with an off-leaf value, when each of its bytes is altered separately and the
    store is opened and the value is read, then each alteration is detected as damage or has no effect, with the rules
    of AC10 of the root and its annotations; the value is never returned with a different byte (V7, G6, AC10).
12. **AC-V12** — Given the store of format `6` kept as test data, when it is opened, then it fails with the different
    format class as an earlier format and it is not modified; a new store reports the format version `7`; and reading
    on disk the bytes of the entry of an off-leaf value, with knowledge of the implementation, its page and its
    length are in little endian (V3, V9, AC-B12).
13. **AC-V13** — Given the closing report, when it is reviewed, then it contains the measurement of V11 with its times
    and store sizes by value size and for the mixed load, and the machine data, on Windows and on Linux (V11).
14. **AC-V14** — Given the closing report, when it is reviewed, then it contains the evaluation of V4 with at least the
    two thresholds it names, the measurement data of each one, the complexity that each adds, the chosen one and why
    (V4).

## Annotations to other PRDs

This PRD changes or refines rules of earlier documents, which carry the corresponding annotations:

- Root parent, D11; trees module, B10: every value is returned as a single contiguous sequence without copying, also
  in temporary mode (V2).
- Transactions, T16; Journal, J7: a committed version of an off-leaf value that is kept outside `krummi.data` is kept
  contiguous, and the reference points to it without copying (V2).
- Trees module, B6: the maximum size of a value becomes 256 MiB (V1).
- Trees module, *Left to the implementation*: the leaf threshold remains left to the implementation, with the
  evaluation of V4.
- Trees and Key Operations, A11 and AC-A17: the provisional maximum is replaced by the one of V1.
- Trees and Key Operations, A9; In-Order Scan, R8: damage in the pages of an off-leaf value makes Read and the scans
  that need its entry fail, but never Store or Add on its key, Remove, Remove range, emptying, deleting or
  overwriting its tree with Rename (V7).
- In-Order Scan, *Out of scope*: values of more than 1,024 bytes in scans and in Remove range arrive with this PRD
  (V1, V2).
- Root parent, D7; Pager, P5; Journal, J3; grouped writing, E15; Free space, F11; Transactions, the whole document;
  Trees and Key Operations, A13; In-Order Scan, R10, *Output* and AC-R14: the current format version becomes `7`, and
  a store of format `6`, also the one created with 5.1 and kept as test data, is of an earlier format (V9).

## References

- Parent: [trees.prd.en.md](trees.prd.en.md) v1.0; root: [krummi.prd.en.md](krummi.prd.en.md) v1.1
- Siblings: [trees-keys.prd.en.md](trees-keys.prd.en.md) v1.0, [trees-scan.prd.en.md](trees-scan.prd.en.md) v1.0
- Related: [free-space.prd.en.md](free-space.prd.en.md), [transactions.prd.en.md](transactions.prd.en.md),
  [pager.prd.en.md](pager.prd.en.md)
- Voron, overflow pages: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron/Data/BTrees>
- LMDB, overflow pages: <https://github.com/LMDB/lmdb/blob/mdb.master/libraries/liblmdb/mdb.c>
- SQLite, overflow cells: <https://www.sqlite.org/fileformat2.html#b_tree_pages>

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| 1927218b | 2026-10-27 06:38 UTC+00:00 | 2 h 4 min | 46 min | 1 h 18 min | 30 min | 48 min | 14 |

<!-- prd-time: [{"session": "1927218b-4e42-490b-bf4b-16e755467a49", "start": "2026-10-27T06:38:47.350000+00:00", "wall": 7442, "discarded": 2733, "agent": 1812, "human": 2895, "interventions": 14, "idle_minutes": 10}] -->
