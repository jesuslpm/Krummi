# Write Checks — PRD

**Version**: 1.0
**Date**: 2026-11-06
**Parent document**: [Remembered Page Integrity — PRD](pager-integrity-once.prd.en.md) (v1.0)

> English translation of [pager-writer-checks.prd.md](pager-writer-checks.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Make write transactions cheaper by ceasing to repeat two checks that they do today on every read:

- the **layout check** (C1 of the remembered layout check) of the nodes that the write itself modified, which is
  repeated in full every time the write reads them again;
- the **integrity check** (G6) of the pages of the data file or, in temporary mode, of the store's memory, which the
  write computes on every read even if the page is already remembered (I3).

With this change, a node that the write modified is checked the first time it is read after each change, and write
transactions use the remembered pages as read transactions already do (I2), as Voron does.

**Why**: in the diagnosis of the performance plan of 2026-11-06, the layout check of the modified nodes takes 57–63% of
the time of the in-order workloads (W1, W3). In those workloads, each insertion reads all the nodes of its path, and the
root and the branches at the end are modified from the first insertions of the transaction, so they are checked in full
on every insertion even though they only change when a node below them splits. The leaf where the insertion lands
changes on every insertion and will still be checked once per insertion. The integrity check in the write takes
5.7–10.2% of the random workloads (W2, W14, W15, W21), because the branches of the path are computed again on every
insertion of the transaction. Krummi misses the threshold of all those workloads in the comparative benchmark (module
6).

**Why now**: O1 to O4 sorted out the reads. The writes are still between 0.24 and 0.76 times Voron, and these two
checks are the largest costs that can be removed without changing the format.

This PRD is a change to the remembered integrity (O6 and O7 of the performance plan). It inherits everything from the
parent and its ancestry; only when the write checks is added here. What changes earlier rules is declared in its
decisions and annotated in the affected documents (see [Annotations to other PRDs](#annotations-to-other-prds)).

### Product decisions

| ID | Decision |
|---|---|
| S1 | **Layout of the nodes that the write modifies** (changes C3 and refines C1 and C4 of the remembered layout check). A write transaction changes a node when it creates it (a split, a new root) or when it obtains a copy of it to modify it, and each later write of its bytes in the transaction is another change. The first read of a node after a change checks its layout, as today. The following reads of the node in the same transaction use it without checking it, until the next change. A check that fails is not remembered: the operation fails with the damage class and the page number, the transaction cannot commit (T11) and each later read of that node fails again. What is remembered belongs only to that transaction: it starts empty, no other transaction sees it and it is discarded when the transaction commits or is abandoned. A node that the transaction changed never consults or feeds the shared memory of remembered nodes (C2), even if it keeps the integrity code of the page it was copied from, and what S1 remembers of it does not pass to that memory on commit. Remembering does not reserve memory that could run out, so it makes no operation fail. The committed versions that are still in the temporary area (J7, T16), which a write reads without having changed them, keep checking their layout on every read, as C3 says. |
| S2 | **Integrity of what the write reads** (changes I3 of the remembered integrity). Write transactions use the remembered pages as read transactions do (I2) in every read of a node, of a page of the free-space map (F9) or of a value stored off its leaf, when the version they read is in the data file or, in temporary mode, in the store's memory: when descending a tree, when copying a page to modify it and when reading the pages and values that move to other pages (splits, merges, values that move). If it is remembered, it is used without computing its code. If not, it is checked: if it fails, the operation fails with the damage class and the page number, the transaction cannot commit (T11), nothing is remembered and each later read fails again; if it passes, from then on it is remembered for all transactions (I1), also if the write is abandoned afterwards. The pages that the write already changed are read from their copy, without an integrity check, as today. The versions of the temporary area still have no integrity check. The opening and the query of the store's information keep checking as today (I3). The version that a write reads from the data file is the last committed one, so the move does not rewrite it while the transaction is open (J5, T14); I4 still forgets each page before the move rewrites it. |
| S3 | **Resulting guarantee** (extends the exception of I8 to G6 of the root parent and the residual risk of C7, annotated in them). It still holds that every page of the data file or of the store's memory is checked the first time any transaction reads it in each opening and the first time after the engine rewrites it (I4), so every damage done while the store is closed is still detected (AC10), also by a write. What changes is the following. If the bytes of a remembered page change without the engine writing them (a hardware or memory failure, or a foreign program, which P16 leaves out of scope), a write transaction that reads it may return data that is not what was written or end in a panic instead of a damage error, like a read one (I8). If in addition it modifies it and commits, it seals it with a valid code in a new version and the damage is stored: no later read detects it as damage, not even after reopening the store. The residual of the values stored off their leaf in the annotation to I1 applies to the writes as well. And if the bytes of a node that the write changed and already checked change in memory before its next change, the write does not detect it. As today, a node that the write changes and does not read again before committing is sealed without checking its layout. It is accepted as a residual risk, as in Voron: it affects what the engine stores only in the face of failures that the engine does not cause. |
| S4 | **No visible changes**: there are no new situations, error classes, error data or options, the format does not change (version `9`) and the store information (P12) adds no data. What S1 remembers lasts as long as the transaction and does not count toward the maximum size (P10). |
| S5 | **Performance** (acceptance as an intermediate step, according to the annotation of 2026-11-06 to K13). The single-workload runs are made before and after the change, in the same session, in two rounds that alternate the order, on Windows and on Linux. A workload improves when its median after exceeds that of before in both rounds of a platform, and worsens when it falls below in both. The change is accepted when W1 and W3 (S1) and W2, W14, W15 and W21 (S2) improve on both platforms and no other measured write workload worsens. The full runs of the benchmark are made at the end of the run that implements this PRD together with O5, and measure the two changes together against those of 2026-11-06. A workload whose ratio with Voron drops by more than 0.05 between them counts as worse only if its single-workload runs against the commit prior to that run confirm the drop (annotation of 2026-11-06 to AC-I12). Not meeting the threshold (K7) does not prevent closing the change; what is missing follows the performance plan. |

### Starting state

Verified on 2026-11-06 in the code and in the profile of the performance plan:

| Aspect | Situation |
|---|---|
| Layout of the modified nodes | A node that the write modified has no checked integrity code, so its layout is checked in full on every read and it does not use the shared memory (C3). An insertion reads once each node of its path, from the root to the leaf, with that check. It is 57–63% of W1 and W3 on Linux. |
| Layout of the store's nodes | It is remembered by page number and code (C2) and is not repeated. |
| Integrity in the write | Each read of the data file computes the code, even if the page is remembered (I3). An unmodified page of the path is computed on every insertion, and a leaf several times in the insertion that modifies it. It is 5.7–10.2% of W2, W14, W15 and W21 on Linux. |
| Integrity in the read | Read transactions use the remembered pages (I2) since O3. |
| Voron (reference) | According to the RavenDB 7.2.7 code cited by the performance plan, it checks each page of the file once per process, also when a write reads it, and does not check the layout of the pages it modifies. |

## Actors

- **Integrating developer**: reads and writes as until now; nothing they use changes.
- **Maintainer**: runs the tests, the single-workload runs and the full runs of S5 on Windows and on Linux.

## Flow

1. A write transaction reads pages of the store using what is remembered (S2).
2. Each node that changes is checked on its first read after the change (S1).
3. When it commits or is abandoned, what it remembered of its nodes is discarded; the store pages it remembered stay
   remembered (I1).

## Validations and messages

There are no new situations, error classes or error data. What a write ceases to detect is in S3.

## Output

Nothing that an operation returns changes in form, and the format version stays `9` (S4).

## Reuse

- The memory of remembered pages of the remembered integrity (I1, I4–I7), unchanged in what it remembers and when it
  forgets.
- The layout check and the shared memory of remembered nodes (C1, C2, C4–C6), unchanged for the store's nodes.
- The single-workload runs and the full runs of the performance plan, for S5.

## Out of scope

- Ceasing to check the layout of the nodes that the write modifies, as Voron does.
- Checking the layout when sealing or committing: a node that is changed and not read again is sealed without checking
  it, as today (S3).
- Passing to the shared memory (C2) what S1 remembers of a changed node.
- Checking the integrity once per write transaction instead of using what is remembered.
- Changing how the committed versions of the temporary area are checked: layout on every read and no integrity (C3,
  I3).
- Checking the layout of the pages that are not nodes: the free-space map, the values stored off their leaf and the
  header (C3).
- Changing what the opening, the query of the store's information, backup, compaction, migration, verification, the
  journal or the recovery check.
- Detecting that another program, or a hardware or memory failure, changes a remembered page while the store is open,
  or preventing a write from sealing that change (S3, P16).
- The other optimizations of the performance plan (O5, O8).
- A public option to turn this change off, and statistics of what is checked in the store information.

## Left to the implementation

- The form of what S1 remembers and how it is forgotten on each change.
- How the tests count the layout and integrity checks, and how they alter the bytes of a modified node or of a
  remembered page, without a public option.
- The exact workloads of the single-workload runs of S5 besides those it names, and their repetitions.

## Acceptance criteria

Those of the ancestors apply and must keep passing unchanged, except those this PRD replaces or refines (see
[Annotations to other PRDs](#annotations-to-other-prds)), including AC10 of the root parent. Each criterion is checked
on a store on disk and in temporary mode, except for the parts that reopen the store, which are on disk only. The tests
use knowledge of the implementation to count the checks, to place pages in the data file or in the temporary area and to
alter bytes. The counts are made with a single thread.

1. **AC-S1** — Given a write transaction that stores keys in order in a normal tree, in a fixed-size one and in the
   subtree of a key of a multi-value tree, with splits of leaves and branches, when each store reads the path, then the
   layout of each changed node is checked once after each change, including the nodes that a split creates, and no read
   without a previous change checks it again; and what is read and committed is the same as today (S1).
2. **AC-S2** — Given a node that a write transaction has just changed, whose code from before the change is in the
   shared memory of remembered nodes, when the test alters its layout before its next read, then that read fails with
   the damage class and its page number, a second read fails the same and the transaction cannot commit (S1, T11).
3. **AC-S3** — Given a node that a write transaction changed and checked, when the transaction commits, and otherwise is
   abandoned, and another write transaction changes it and reads it, then its layout is checked again in that read (S1).
4. **AC-S4** — Given the nodes of a tree, remembered by an earlier read transaction, a page of the free-space map
   remembered by an earlier write and a value stored off its leaf remembered, all in the data file, when a write
   transaction reads a key twice, stores keys that split and merge nodes, reserves pages and moves the value, then it
   does not compute the integrity of any of those pages or of the value; and a page of the data file that nobody read
   on opening is checked on its first read by the write and not on the following ones, of that transaction or of
   others (S2). This criterion replaces AC-I2.
5. **AC-S5** — Given a page of the data file altered with the store closed, when it is opened and a write transaction
   reads it twice, then each read fails with the damage class and the number of that page and the transaction cannot
   commit (S2, T11).
6. **AC-S6** — Given a remembered leaf with no pending versions to move, when the test alters with the store open a byte
   of the value of one of its entries without breaking its layout, then a write transaction that reads it without
   modifying it returns the altered byte without failing, and after closing and reopening the store the first read of
   that leaf fails with the damage class and the number of its page; and when, before reopening, another write modifies
   another entry of that leaf and commits, then after the move and a new opening the leaf is read with no damage error
   and returns the altered byte (S2, S3, I5). This criterion replaces AC-I8.
7. **AC-S7** — Given a committed version of a node that is still in the temporary area, when a write transaction reads
   it twice without changing it, then its layout is checked on every read and its integrity is not checked, as today
   (S1, S2).
8. **AC-S8** — Given the single-workload runs of S5, when they are compared, then W1, W3, W2, W14, W15 and W21 improve
   on both platforms and no other measured write workload worsens; and the closing report includes those comparisons
   and those of the full runs with those of 2026-11-06, with the single-workload ones that confirm or discard each drop
   of more than 0.05 (S5).

## Annotations to other PRDs

This PRD changes or refines rules of earlier documents, which carry the corresponding annotations:

- Root parent, G6: the annotation of the remembered integrity said that write transactions keep checking everything
  they read. It now says that they use what is remembered (S2), and the exception of I8 is extended with S3: a write
  may seal in a new version a change of a remembered page made while the store is open by something that is not the
  engine, and the damage is stored. AC10 does not change.
- Remembered integrity: the Objective ("only the read transactions use it"), I3, I8, step 3 of the Flow and the point
  of Out of scope "Using what is remembered in the write transactions" cease to hold for the writes, which follow S2
  and S3. AC-I2 is replaced by AC-S4 and AC-I8 by AC-S6.
- Remembered layout check: C3, step 4 of the Flow and the point of Out of scope about not remembering the layout of the
  pages that the write modifies now say that those pages check their layout the first time they are read after each
  change, with a memory of each transaction's own (S1); C1 and C4 hold for the shared memory, which those pages do not
  use; the residual risk of C7 is extended with that of S3.
- Fixed-size trees, AC-X19: its fourth part, which AC-I8 had replaced, becomes AC-S6.

## References

- Parent: [pager-integrity-once.prd.en.md](pager-integrity-once.prd.en.md) v1.0 (I1–I9, AC-I2, AC-I8)
- Pager: [pager.prd.en.md](pager.prd.en.md) v1.0 (P10, P12, P16)
- Root: [krummi.prd.en.md](krummi.prd.en.md) v1.1 (G6, AC10)
- Related: [trees-node-check-cache.prd.en.md](trees-node-check-cache.prd.en.md) (C1–C7),
  [transactions.prd.en.md](transactions.prd.en.md) (T11, T14, T16), [journal.prd.en.md](journal.prd.en.md) (J5, J7),
  [free-space.prd.en.md](free-space.prd.en.md) (F9), [trees-fixed-size.prd.en.md](trees-fixed-size.prd.en.md) (AC-X19),
  [benchmark.prd.en.md](benchmark.prd.en.md) (K7, K13)
- Performance plan: [docs/performance/plan.md](../../performance/plan.md), O6 and O7, diagnosis of 2026-11-06
