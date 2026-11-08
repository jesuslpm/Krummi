# Remembered Check of Temporary-Area Versions — PRD

**Version**: 1.0
**Date**: 2026-11-08
**Parent document**: [Write Checks — PRD](pager-writer-checks.prd.en.md) (v1.0)

> English translation of [pager-version-checks.prd.md](pager-version-checks.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Make reads of the **committed versions that are still in the temporary area** (J7, T16) cheaper: the pages that a
write committed and that the move has not yet written to the data file. Today, every read of a node from one of those
versions checks its whole layout (C3, S1), because the versions have no integrity check (I3) with whose code to
remember it. A version does not change while it is in the temporary area, so the same check is repeated on the same
bytes in every read of every transaction.

With this change, each version in the temporary area is checked the first time a transaction reads it, and later reads,
by that transaction or by others, use it without checking it. In addition, reads of different transactions stop waiting
for each other to find the page they see.

**Why**: in the comparative benchmark (module 6), Krummi misses the threshold of `reads-with-writer` (W12) with 0.37
times Voron on Windows and 0.63 on Linux (reports `2026-11-07-windows-write-reuse` and `2026-11-07-linux-write-reuse`
in `docs/benchmark`). In that workload, four threads read random keys while a write commits without stopping. While
the temporary area has versions, every read of a page waits for the other reads to find it, whether or not it is a
version; and the versions they read, the root, the branches and the leaves that the write changed recently, are checked
in full on every read. Writes also read the versions that earlier ones committed: in the diagnosis of 2026-11-07 of the
performance plan, the full layout check takes 15% of W2 and 22% of W19 on Linux, and the part for versions is a share of
that which was not measured separately. According to the Voron code reviewed by the investigation of 2026-11-08, Voron
does not check the layout of any page.

**Why now**: it is candidate 2 of the writes diagnosis and the second lever of the investigation of 2026-11-08 into
what separates Krummi from Voron, together with writing only the changes of each page to the journal (O8, with its own
PRD).

This PRD changes when the versions of the temporary area are checked, for read and write transactions, and specifies
how reads find them. It inherits everything from the parent and its ancestry. What changes earlier rules is declared in
its decisions and annotated in the affected documents (see [Annotations to other PRDs](#annotations-to-other-prds)).

### Product decisions

| ID | Decision |
|---|---|
| U1 | **Layout of the temporary-area versions** (changes C3 of the remembered layout check and the last sentence of S1). The first read of a node from a committed version that is in the temporary area checks its layout, as today, whether it is done by a read or a write transaction. If it passes, later reads of that same version, by any transaction, do not check it again while the version stays in the temporary area. A check that fails is not remembered, as in C1 and S1: each read of that version fails again with the damage class and the page number, and a write that reads it cannot commit (T11). What is remembered of a version holds only for it: it is neither consulted in nor stored in the shared memory of remembered nodes (C2) or in that of a write's changes (S1), and it does not serve another version of the same page, newer or of another node that reuses the page. A write that obtains a copy of a remembered version to modify it makes a change (S1): the copy is checked on its first read. The other checks of the read of a node (that the page can hold data, its type, its level and the coherence of each entry that is used) are still made on every read (C1). Remembering does not reserve memory that could run out, so it makes no operation fail, and it does not count toward the maximum size (P10). C5 does not hold for this memory: each version is remembered while it is in the temporary area, with no discards. |
| U2 | **Resulting guarantee** (extends the residual risk of C7 and S3, annotated in them). The versions of the temporary area have no integrity check, so today the layout check, on every read, is the only thing that can detect that the bytes of a version change in the temporary area without the engine writing them (a hardware, disk or memory failure, or a foreign program, which P16 leaves out of scope), and only when the change breaks its layout. With this change, that is only detected if it happens before the first read of the version. After that, a read may return data that is not what was written or end in a panic instead of a damage error. If a write copies that version to modify it, it checks it on its first read, which may not detect it; if it commits it, the change is sealed with a valid code, as in S3. If the move writes the altered version to the data file, its code no longer matches and the first read from there fails with the damage class (G6). It is accepted as a residual risk, as in Voron: it affects what the engine reads and stores only in the face of failures that the engine does not cause. Every damage done while the store is closed is still detected (AC10), because the temporary area does not survive a close (J7). |
| U3 | **Reads that do not wait for each other**: finding the page that a transaction sees does not make a read wait for other reads, of the same transaction or of others. What G3 and J5 say about the waits between reads, write and move does not change. Opening and closing a read transaction does not change. |
| U4 | **No visible changes**: as S4. What is read and what is committed is the same as today. |
| U5 | **Performance**: as S5, on W12 (which must improve on both platforms) and with at least W2, W7, W10, W12, W13, W14 and W19 measured, none of which must worsen. The reference full runs are those of 2026-11-07 after the reuse of write resources (`docs/benchmark/2026-11-07-*-write-reuse.md`). |

*Annotation of 2026-11-08 from the journal of the changes of each page
([journal-page-diffs.prd.en.md](journal-page-diffs.prd.en.md)): U4, the current format version becomes `10`
(decision Y5).*

### Starting state

Verified on 2026-11-08 in the code, in the profiles of that day's investigation, in the benchmark reports and in the
performance plan:

| Aspect | Situation |
|---|---|
| Layout of the versions | A read that finds the version of a page in the temporary area delivers it with no integrity code, and the trees check its layout on every read (C3), also in writes (S1, AC-S7). |
| Life of the versions | A published version is not rewritten while it is in the temporary area. It leaves it when no open snapshot can see it; the move writes it to the data file without removing it while any snapshot can see it. |
| Search for versions | While the temporary area has versions, every read of a page, whether or not it is a version, goes through an exclusive wait that all reads, the write when publishing and the move share. |
| W12 | Four read threads and a write that commits batches of 100 entries. Ratio with Voron: 0.37 on Windows and 0.63 on Linux (`docs/benchmark/2026-11-07-*-write-reuse.md`). |
| Cost in the writes | The full layout check is 15% of W2 and 22% of W19 on Linux; the part that corresponds to the temporary-area versions was not measured (diagnosis of 2026-11-07, candidate 2). |

## Actors

- **Integrating developer**: reads and writes as until now; nothing they use changes.
- **Maintainer**: runs the tests and the single-workload and full runs of U5 on Windows and on Linux.

## Flow

1. A write commits: its pages become versions in the temporary area.
2. The first read of a node from one of those versions, by any transaction, checks its layout; later reads of that
   version do not repeat it (U1).
3. The reads of several transactions find their pages at the same time, without waiting for each other (U3).
4. When the version leaves the temporary area, its reads move to the data file as today (C2, C3).

## Validations and messages

| Situation | Result |
|---|---|
| Read of a version with a damaged layout | Fails with the damage class and its page number, on every read (U1). |
| Write that read a damaged version | Cannot commit (T11). |

There are no new situations, error classes or error data. What ceases to be detected is in U2.

## Output

Nothing that an operation returns changes in form, and the format version stays `9` (U4).

## Reuse

- The layout check of the nodes, unchanged in what it checks.
- The rules of the move and of the life of the versions (J5, J7, T14, T16), on which it depends that a version does not
  change while it is remembered.
- The single-workload and full runs of the performance plan, for U5.

## Out of scope

- Ceasing to check the layout of the temporary-area versions, as Voron does.
- Checking the integrity of the temporary-area versions (I3).
- Changing how the nodes that the write itself modifies (S1) or the pages of the data file (C2, I2, S2) are checked.
- Remembering the other checks of the read of a node (C1).
- Bounding the size of what U1 remembers, as C5 does.
- Making reads not wait for the write when publishing or for the move (G3, J5), and the waits when opening or closing a
  transaction (U3).
- The pages that are not nodes: the free-space map, the values stored off their leaf and the header (C3).
- Changing what the opening, the query of the store's information, backup, compaction, migration, verification, the
  journal or the recovery check.
- The compression of leaf pages (5.6) and encryption (module 9): each decides whether it uses what U1 remembers.
- Detecting that something that is not the engine changes a temporary-area version while the store is open (U2, P16).
- The other optimizations of the performance plan, among them writing only the changes of each page to the journal
  (O8).
- A public option to turn this change off, and statistics of what is checked in the store information.

## Left to the implementation

- Where and in what form what U1 remembers of each version is stored, within what U1 allows.
- How pages are found without reads waiting for each other (U3).
- How the tests count the layout checks and how they alter the bytes of a temporary-area version, without a public
  option.
- The workloads of U5 besides those it names, and the repetitions.

## Acceptance criteria

Those of the ancestors apply and must keep passing unchanged, except those this PRD replaces (see
[Annotations to other PRDs](#annotations-to-other-prds)), including AC10 of the root parent and AC-C5. Each criterion is
checked on a store on disk and in temporary mode; AC-U1, in a normal tree, in a fixed-size one and in the subtree of a
key of a multi-value tree, and the others in a normal tree, because the memory of U1 does not depend on the type of
tree. The tests use knowledge of the implementation to count the checks, to keep versions in the temporary area without
moving them and to alter bytes. The counts are made with a single thread.

1. **AC-U1** — Given a committed version of a node that is in the temporary area, when a read transaction reads it
   twice, another read transaction reads it afterwards and a write transaction reads it twice without changing it, then
   all return the same and its layout is checked only in the first of those reads, and its integrity in none (U1). This
   criterion replaces AC-S7.
2. **AC-U2** — Given a temporary-area version with a damaged layout (AC-C5), when a read transaction reads it twice and
   a write one reads it afterwards, then each read fails with the damage class and the number of that page, and the
   write cannot commit (U1, T11).
3. **AC-U3** — Given a remembered version, when a write commits a newer version of the same page while a read
   transaction opened earlier keeps reading the earlier one, then that transaction keeps seeing the earlier content and
   its layout is not checked again, and the first read of the new version checks it once (U1, G3).
4. **AC-U4** — Given a remembered version whose page is freed, when a later write reuses it for another node and commits
   it, then the first read of that node checks its layout (U1).
5. **AC-U5** — Given a remembered version, when a write copies it to modify it and reads it, then its layout is checked
   in that read (U1, S1).
6. **AC-U6** — Given a remembered version with room to remember in the shared memory and a content that this memory does
   not remember, when the version leaves the temporary area and a new read transaction reads the node from the data
   file twice, then its layout is checked in the first of those reads and not in the second (U1, C2).
7. **AC-U7** — Given four threads with read transactions that read, at least a thousand times each, nodes whose
   versions are in the temporary area, one of them with a damaged layout, while a write commits new versions of those
   pages and the move writes them, then each read returns the data of its snapshot and the damaged version fails in
   all its reads with the damage class; the number of checks is not counted (U1, U3).
8. **AC-U8** — Given the single-workload runs of U5, when they are compared, then W12 improves on both platforms and no
   other measured workload worsens; and the closing report includes those comparisons and those of the full runs with
   the reference ones (U5). U3 is checked with this criterion and with AC-U7.

## Annotations to other PRDs

This PRD changes or specifies rules of earlier documents, which carry the corresponding annotations, also in their
translations (`*.prd.en.md`):

- Remembered layout check: C3, step 4 of the Flow and the Out of scope item "remembering the layout" of the
  temporary-area versions now say that each version is checked on its first read and remembered while it stays in the
  temporary area (U1); C1 and C4 hold for the shared memory, and C5 does not hold for U1's; the residual risk of C7 is
  extended with U2. AC-C5 does not change.
- Write checks: the last sentence of S1 and the Out of scope item "Changing how the committed versions of the temporary
  area are checked" cease to hold (U1); the residual risk of S3 is extended with U2. AC-S7 is replaced by AC-U1.

## References

- Parent: [pager-writer-checks.prd.en.md](pager-writer-checks.prd.en.md) v1.0 (S1, S3, S4, S5, AC-S7)
- Remembered layout check: [trees-node-check-cache.prd.en.md](trees-node-check-cache.prd.en.md) (C1–C5, C7,
  AC-C5)
- Remembered integrity: [pager-integrity-once.prd.en.md](pager-integrity-once.prd.en.md) (I2, I3)
- Pager: [pager.prd.en.md](pager.prd.en.md) v1.0 (P10, P12, P16)
- Root: [krummi.prd.en.md](krummi.prd.en.md) v1.1 (G3, G6, AC10)
- Related: [transactions.prd.en.md](transactions.prd.en.md) (T11, T14, T16), [journal.prd.en.md](journal.prd.en.md) (J5, J7),
  [benchmark.prd.en.md](benchmark.prd.en.md) (K7, K13)
- Performance plan: [docs/performance/plan.md](../../performance/plan.md), writes diagnosis of 2026-11-07,
  candidate 2
