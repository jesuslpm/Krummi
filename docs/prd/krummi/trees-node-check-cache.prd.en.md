# Remembered Node Layout Check — PRD

**Version**: 1.0
**Date**: 2026-10-30
**Parent document**: [Krummi B+ Trees — PRD](trees.prd.en.md) (v1.0)

## Objective and context

Make tree reads cheaper without ceasing to check the integrity of every page read from the data file. Today, each time
an operation reads a **node** (a page of a normal tree, of the catalog, of a multi-value tree or of the subtree of one
of its keys), the engine makes two checks of the whole page:

- the **integrity check** (G6), a 64-bit code that detects altered bytes, only on the pages that come from the data
  file or, in temporary mode, from the store's memory;
- the node's **layout check**, which confirms that its internal arrangement (type, number of entries, position and
  size of each one, with no gaps or overlaps) is coherent before using it, wherever it comes from.

The second one walks all the entries of the node, so it costs more the more entries the node has, and it is repeated
on every read even if the node has not changed. Besides those two, a read makes other checks that this change does not
touch: that the page can hold data, the level expected of the node and the coherence of each entry it uses (the
content of an entry of a multi-value tree, that of a catalog entry, the references to values stored off their leaf).

This change makes the engine **remember the nodes whose layout it has already checked** while the store is open,
identified by their page number and by the integrity code that the same read has just verified. The integrity check is
still made on every read, as today.

**Why**: in dense leaves, such as those of the subtrees of a multi-value tree with 16-byte values (about 370 entries),
the layout check costs several times more than the integrity one. In the investigation that followed the close of 5.4,
Contains in a subtree went from 6.4–7.0 µs to 2.8–3.1 µs without the layout check and keeping the integrity one; in a
normal tree with composite keys, from 3.7–4.1 µs to 3.3–3.4 µs.

**Why now**: multi-value trees (5.4) are the first to fill leaves with hundreds of small entries, and the tables with
secondary indexes (module 7) will rest on them.

This PRD is a change to the trees module outside its split into six child PRDs (B2): it is not one of them and does
not count toward the close of the module. It inherits everything from the parent and its ancestry; only when the
layout check is repeated is added here. What changes earlier rules is declared in its decisions and annotated in the
affected documents (see [Annotations to other PRDs](#annotations-to-other-prds)).

### Product decisions

| ID | Decision |
|---|---|
| C1 | **What is remembered**: that the content of a node passed the layout check, identified by its page number and its integrity code. Only passed checks are remembered: a node that does not pass it is never remembered. The other checks of the read (that the page can hold data, the level of the node, the coherence of each entry that is used) are not remembered and are made as today, on every use. |
| C2 | **Order on every read of a node**: first the integrity of the bytes being read is checked; if it fails, the operation fails with the damage class, as today (G6). If it passes and the engine remembers that page number with that same code, the layout of the node is accepted without checking it. If it does not remember it, it is checked: if it fails, the operation fails with the damage class and the page number, as today; if it passes, from then on it is remembered. The integrity code of an earlier read is never used. |
| C3 | **Which reads use it**: only the reads of nodes whose integrity is checked in that same read, that is, those that come from the data file or, in temporary mode, from the store's memory. These do not use it, and check the layout on every read as today: the committed versions that are still in the temporary area (J7, T16) and the pages that the write itself modifies, which today do not pass the integrity check. The pages that are not nodes (those of the free-space map, F9; the header; those of the values stored off their leaf) have no layout check and do not change. Reading from the data file a content that was earlier read from the temporary area does not find it remembered: it is checked and remembered. |
| C4 | **Scope and lifetime**: there is a single memory of remembered nodes per open store, also in temporary mode, shared by all its transactions, trees and threads. It starts empty when the store is opened, is discarded when it is closed and is not saved to disk: the format does not change (B12) and a store opens the same with or without this change. |
| C5 | **Bounded memory**: what is remembered takes a fixed size that does not depend on the size of the store or on the number of reads. When there is no room, a remembered node may stop being remembered; it only costs checking its layout again. A lookup only finds a remembered node if it was remembered with exactly that page number and that code: it never combines the number of one node with the code of another. The size is internal: it is not a store option, it does not appear in its information (P12) and it makes no operation fail. |
| C6 | **Concurrent reads**: querying and adding remembered nodes adds no blocking waits between reads or with the write. Several reads that read a non-remembered node at the same time may each check its layout; the result is the same. |
| C7 | **Resulting guarantee** (exception to G6 of the root parent, annotated there): every page read from the data file still passes the integrity check on every read, so everything that check detects today is still detected. What changes is the second defence. Today, if the bytes of a node change without the engine writing them (a hardware or memory failure, or a foreign program, which P16 leaves out of scope) and the integrity check does not detect it because of a coincidence in its 64-bit code, the layout check can still detect it. With this change, if that page number with that code was already remembered in the current opening, the node is accepted without checking its layout. Thus, an alteration that matches the code no longer also needs to leave a coherent layout to go unnoticed. In that case, the operation may return data that is not what was written or end in a panic instead of a damage error (and not with the error of G12). Using a badly laid-out node does not by itself cause undefined behavior, because the trees use no unsafe code (D12); the exception of P16 stays the same: if a foreign program modifies the files while the store is open, the behavior may be undefined, as today. It is accepted as a residual risk. A store that is opened starts with nothing remembered, so alterations made while the store is closed are detected as today (AC10). |
| C8 | **Coherence of the pages**: it relies on the current rules of the move (J5, T14): the engine only rewrites a page in its place in the data file when no open read transaction needs the earlier version, so the bytes of a node that a read checks do not change while it checks them or while it remembers them in use. |
| C9 | **Measurement** (B12, no threshold by D5): the change includes a reproducible measurement that is run by hand, on Windows and on Linux, and that is not part of continuous integration or of the normal tests. It measures, without remembering nodes and remembering them, the cost of reading leaf nodes of 1, 100 and 370 entries, and Contains in a multi-value tree with the values in subtrees and in a normal tree with composite keys, on stores on disk. The closing report includes its results, also if any case is slower with the change. |

### Starting state

Verified on 2026-10-30 in the code:

| Aspect | Situation |
|---|---|
| Integrity check | 64-bit code over the whole page, verified on every read of a page from the data file or from the temporary store's memory. It is not verified on the versions of the temporary area or on the pages that the write modifies. |
| Layout check | It is made on every read of a node, wherever it comes from, and walks all its entries. Alongside it, it is checked that the page can hold data and the expected level; the coherence of each entry is checked when it is used. |
| Page versions | The header of a page stores its integrity code and its number; it carries no version. Committed versions live in the temporary area until the move writes them in their place (J5, T14). Freed pages are reused with other content, and when the file grows a page may change its address in memory. |
| Check memory | It does not exist: no check is remembered between reads. |
| Damage tests | Those that alter the files do it with the store closed and reopen it. A test of the trees commits, through a write, a node with a damaged layout and a correct integrity and expects damage in two consecutive reads; that page is read from the temporary area. |
| Unsafe code | The trees use no unsafe code (D12): a badly laid-out node that was used without checking would cause a panic or wrong results, not an access outside memory. |
| Precedent | 5.3 left to the implementation whether the integrity of a value stored off its leaf is checked on every read or once per transaction, as long as G6 is met. |

An isolated measurement of 2026-10-30 (Windows, optimized build, in a temporary copy that is not kept; indicative, it
is replaced by that of C9) gave for the integrity check about 0.3 µs per page in cache and 0.5–0.6 µs outside it,
whatever its content, and for the layout check about 0.03 µs with 1 entry, 0.1 µs with 10, 0.8–1.0 µs with 100,
1.4–2.0 µs with 200 and 2.5–2.9 µs with 370: between 4 and 10 times the integrity one in the densest leaves, and less
than it in nodes with few entries.

## Actors

- **Integrating developer**: reads and writes trees as until now; nothing they use changes.
- **Maintainer**: runs the tests and the measurement of C9 on Windows and on Linux.

## Flow

1. The application opens a store; the engine starts with no remembered nodes (C4).
2. An operation reads a node from the data file: the engine checks its integrity and, if it does not remember that node
   with that code, its layout, and remembers it if it passes (C2).
3. The following reads of that same content, from any transaction, check its integrity and do not repeat the layout
   check while it is still remembered (C4, C5).
4. If a write changes the node, its new version is read from the temporary area, with the layout check on every read
   (C3); when the move writes it in the data file, its code is different and the first read that sees it there checks
   it (C1, C2).
5. When the store is closed, what is remembered is discarded.

## Validations and messages

There are no new situations, error classes or error data. A node whose integrity or layout fails still fails with the
damage class and the number of its page (D8), in every operation that needs it (A9, M11).

## Output

Nothing that an operation returns changes in form: the same data, the same error classes and the same error data. The
store information (P12) adds no data and the format version stays `8`. For example, the second read of a node with a
damaged layout fails the same as the first:

| Error data | First read | Second read |
|---|---|---|
| Class | damage | damage |
| Folder | the store's | the store's |
| Page | `<number>` | `<number>` |

## Reuse

- The integrity check of the pages and the damage class (G6), unchanged.
- The layout check of the nodes, unchanged in what it checks.
- The rules of the move (J5, T14), on which C8 depends.
- The measurement of the multi-value trees (M15), as the basis of that of C9.

## Out of scope

- Ceasing to check the integrity on every read, or remembering it (C7).
- Checking the integrity of the versions of the temporary area and of the pages that the write modifies, and
  remembering their layout (C3).
- Remembering the other checks of a read: the level of the node and the coherence of the entries (C1).
- The pages that are not nodes: free-space map, header and values stored off their leaf (C3).
- Detecting that another program modifies the files while the store is open (P16).
- Stronger or cryptographic integrity codes.
- Making the layout check itself cheaper, or checking only the entries that the operation uses.
- Remembering already decoded nodes or complete paths.
- Saving what is remembered between openings.
- A public option for the size or to turn it off, and statistics or hit counters in the store information (C5).
- Fixed-size trees (5.5), leaf page compression (5.6) and encryption (module 9): each decides whether it uses what is
  remembered and over which bytes its code is computed.
- Performance thresholds (module 6, D5).

## Left to the implementation

- The size of the memory of remembered nodes, as an internal constant, how it is distributed among the page numbers,
  which memory is discarded when there is no room and when that memory is reserved (C5). The tests may use their own
  size.
- Whether it is used for all nodes or only from a certain number of entries, where the measurement of C9 shows it pays
  off.
- How the read of a node knows that its integrity was checked in that same read (C2, C3).
- How the measurement compares without remembering and remembering without a public option (C5, C9), and the exact
  loads, units and repetitions of the measurement, within C9.

## Acceptance criteria

Those of the ancestors apply and must keep passing unchanged, including AC10 of the root parent, AC-M15 and the rest of
the damage tests of the trees. Each criterion is checked on a store on disk and in temporary mode, except for the
parts that reopen the store, which are on disk only. The tests use knowledge of the implementation to count the layout
checks, to place pages in the data file or in the temporary area, to alter bytes and to choose the size of the memory of
remembered nodes.

1. **AC-C1** — Given a normal tree, the catalog and the subtree of a key of a multi-value tree, with their pages in the
   data file and room to remember all the nodes that the test reads, when a read transaction reads the same key or the
   same value several times and another read transaction reads it afterwards, then all return the same, the layout of
   each node of the path is checked only on the first read of that node and its integrity on all of them (C1, C2, C4).
2. **AC-C2** — Given a remembered node, when a write changes its content and commits while a read transaction opened
   earlier keeps reading it, then that transaction keeps seeing the earlier content; and when that transaction closes
   and the new version is moved to the data file at the same page number, then a new read transaction sees the new
   content and checks its layout once (C1, C2, C8, G3).
3. **AC-C3** — Given a page of a remembered node that is freed and that a later write reuses for another node with
   other content, when that new node reaches the data file and is read, then its content is read and its layout is
   checked once (C1, C2).
4. **AC-C4** — Given a page of a node in the data file with a damaged layout and the integrity code recalculated so
   that it is correct, when it is read twice in the same read transaction and once in another, then each read fails
   with the damage class and the number of that page, and a later read checks it again; also when another correct
   version of that same page had been remembered earlier (C1, C2, A9).
5. **AC-C5** — Given the same damaged page of AC-C4, but in the temporary area (committed and not moved), when it is
   read twice, then it fails the same both times (C3).
6. **AC-C6** — Given a leaf of a multi-value tree in the data file with a correct layout that is remembered, and an
   entry whose content is incoherent, when an operation uses it twice, then it fails both times with the damage class
   and the number of that page (C1, M11).
7. **AC-C7** — Given a remembered node, when the test alters a byte of its page in the data file without recalculating
   its integrity code, then the next read fails with the damage class and the number of that page (C2, C7).
8. **AC-C8** — Given a memory of remembered nodes of a size reduced for the test, and correct nodes and a damaged one
   as in AC-C4 whose page numbers force it to discard memories, when four threads with read transactions read them at
   the same time, at least a thousand times each, while a write changes them and they are moved, then each read returns
   the data of its snapshot and the damaged node fails on all its reads with the damage class (C5, C6).
9. **AC-C9** — Given a store on disk with remembered nodes, when it is closed and opened again, then the first read of
   each node checks its layout again (C4).
10. **AC-C10** — Given the measurement of C9, when it is run on Windows and on Linux, then it reports the cost per read
    of leaf nodes of 1, 100 and 370 entries and that of Contains in the two loads, without remembering nodes and
    remembering them, and the results are left in the closing report (C9).

## Annotations to other PRDs

This PRD changes or refines rules of earlier documents, which carry the corresponding annotations:

- Root parent, G6 and AC10: exception of C7. The layout check of a node whose integrity has just been verified is
  skipped if that same content (page number and integrity code) already passed it in the current opening; an alteration
  that matches the integrity code of a remembered content may return data that is not what was written or end in a
  panic. Alterations made while the store is closed are detected as today.
- B+ Trees, B2: this change is not one of the six child PRDs of the split and does not count toward the close of the
  module.
- Trees and Key Operations, A9, and multi-value trees, M11: a damaged node still fails with damage in every operation
  that needs it, except for the residual risk of C7.

## References

- Parent: [trees.prd.en.md](trees.prd.en.md) v1.0
- Root: [krummi.prd.en.md](krummi.prd.en.md) v1.1
- Related: [pager.prd.en.md](pager.prd.en.md) (P12, P15–P17), [transactions.prd.en.md](transactions.prd.en.md) (T14,
  T16), [journal.prd.en.md](journal.prd.en.md) (J5, J7), [free-space.prd.en.md](free-space.prd.en.md) (F9),
  [trees-keys.prd.en.md](trees-keys.prd.en.md) (A9), [trees-large-values.prd.en.md](trees-large-values.prd.en.md) (V7
  and its *Left to the implementation*), [trees-multivalue.prd.en.md](trees-multivalue.prd.en.md) (M11, M15)
- Previous investigation: execution log of the multi-value trees, entry L22
