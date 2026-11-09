# Check of the Write's Nodes at Commit — PRD

**Version**: 1.0
**Date**: 2026-11-09
**Parent document**: [Write Checks — PRD](pager-writer-checks.prd.en.md) (v1.0)

> English translation of [pager-commit-checks.prd.md](pager-commit-checks.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Make write transactions that change the same nodes many times cheaper. Today, the first read of a node after each
change checks its whole layout (S1 of the write checks): a leaf that receives thirty insertions in one transaction is
checked thirty times. With this change, the write uses the nodes it changed itself without checking them, and checks
each one only once, at commit, before its point of no return.

**Why**: in the comparative benchmark (module 6), `carga-masiva` (W3) is the only threshold that Krummi fails on Linux,
at 0.78 times Voron, and on Windows it stands at 0.82 (benchmark reports of 2026-11-08). In the margin diagnosis of
2026-11-09 in the performance plan, that check takes 33% of W3 on Windows and 39% on Linux: W3 stores a million random
keys in a single write, about thirty per leaf, and each insertion checks its leaf again. The loads in order (W1) repeat
the same pattern. According to the Voron code reviewed by the research of 2026-11-09, Voron does not check the layout of
any page in production (its check of the tree pages, `TreePage.DebugValidate`, exists only in debug builds).

**Why now**: it is proposal W3 b) of the margin diagnosis of 2026-11-09, which the user authorized with the agent's
recommendations.

This PRD is a change to the write checks. It inherits everything from the parent and its ancestors; here only the
moment at which the layout of a node changed by the write is checked changes. What changes earlier rules is declared in
its decisions and annotated in the affected documents (see [Annotations to other PRDs](#annotations-to-other-prds)).

### Product decisions

| ID | Decision |
|---|---|
| H1 | **Read of a node that the write changed** (changes S1 of the write checks and the sentence of U1 about the copy of a version). A write transaction changes a node when it creates it or when it obtains a copy of it to modify it, as S1 defines. Its reads of a node that it changed do not check its layout: it uses the node as its changes left it. The other checks of each read of a node (that the page can contain data, its type, its level and the coherence of each entry that is used, C1) remain on every read. The page from which the copy is obtained is checked as today when it is read as a node (C1–C3, U1); the copy, at commit (H2). |
| H2 | **Check at commit** (changes the second-to-last sentence of S3 and the Out of scope item "Checking the layout when sealing or committing" of the parent, and adds a failure to those that E2 produces before the point of no return). When committing a write with changes, in both commit modes (E5) and also in temporary mode, the transaction checks once the layout of each node that it changed and that it keeps at commit, that is, of each node page that the commit writes; a node occupies one page. One that it changed and then freed is not checked, because freeing discards its changes (F8). The pages that it changed without being nodes (the free-space map, the values stored off their leaf and the header) are not checked. The check goes after the failures of T11 and before everything else that E2 does before the point of no return, also before saving the pages in the temporary area in temporary mode, so an inconsistent node takes precedence over any other failure of the commit: if an earlier operation of the transaction already failed, the commit fails with the class of that failure, as today (T11), without checking anything. If a node does not pass, the commit fails with the damage class and the data of a damage error (D8, G6), with the number of the lowest-numbered page that does not pass; it applies nothing of the transaction, the data file keeps what it has grown (T10, P11), the store keeps accepting writes, the earlier commits that were already in the journal queue are not affected and the turn passes to the next write request, as in any commit that fails before its point of no return (T11, E2). What is checked at commit is neither consulted in nor saved to the shared memory of remembered nodes (C2), as S1 said of the nodes that the write changes. An abandon, and a commit with no changed nodes, check nothing. |
| H3 | **Resulting guarantee** (extends the exception of G6 of the root parent and the residual risk of S3 of the parent). No node whose layout is inconsistent gets committed by a write, including those it changed and did not read again, which today are sealed without being checked. The following changes: the nodes that a write changes come from nodes that the tree read and checked in order to reach them, so only a failure of the engine itself, or of the hardware or the memory (P16), can leave them inconsistent; in that case, a later operation of the same transaction that reads that node may return data that is not what was written, fail with another class or end in a panic instead of a damage error, and the commit, if it gets that far, fails without applying anything. A change of bytes that does not break the layout is still sealed, as S3 says. It is accepted as a residual risk, as in Voron, which does not check what it commits either. |
| H4 | **Performance**. The single-workload runs are made before and after the change, in the same session, in two rounds that alternate the order, on Windows and on Linux, with the benchmark's power plan described by the measurement method of the performance plan. A workload improves when its median after exceeds that of before in both rounds of a platform. A workload that falls below in both rounds is measured in two new rounds, and worsens if it also falls below in both. The change is accepted when W1 and W3 improve on both platforms and none of the write workloads W1–W6, W13–W15, W17, W19 and W21 worsens. A workload that is above in one round and below in the other neither improves nor worsens. If W1 or W3 do not improve on some platform, or any workload worsens, the closing report says so and the user decides; not meeting the threshold (K7) does not prevent closing the change. |

### Starting state

Verified on 2026-11-09 in the code and in the profiles of the margin diagnosis:

| Aspect | Situation |
|---|---|
| Check of the changed nodes | The first read of a node after each change checks it in full (S1). |
| Commit | It does not check the layout: a changed node that is not read again is sealed without being checked (S3). |
| Cost | The layout check is 33% of W3 on Windows and 39% on Linux, almost all of it from the leaves that change on every insertion. |
| Voron (reference) | It checks the integrity code of each page of the file once per process and does not check the layout of any page in production. |

## Actors

- **Integrating developer**: reads and writes as until now; nothing they use changes.
- **Maintainer**: runs the tests and the single-workload runs of H4 on Windows and on Linux.

## Flow

1. A write transaction reads the nodes it did not change by checking them as today, and those it changed without
   checking their layout (H1).
2. At commit, after the failures of T11, it checks each node that it changed and keeps; if all pass, it commits as
   today (H2).
3. If any does not pass, the commit fails with the damage class and the number of the lowest-numbered page, without
   applying anything (H2).

## Validations and messages

The commit of a write with an inconsistent node fails with the damage class, with the data of a damage error and the
number of the lowest-numbered page that does not pass (H2). There are no new messages.

## Output

Nothing that an operation returns changes shape, and the format version is still `10`.

## Reuse

- The layout check of each node format, that of the nodes with variable-size entries and that of the fixed-size ones,
  with no change in what it accepts.
- The single-workload runs of the performance plan, for H4.

## Out of scope

- Ceasing to check the nodes that the write changes, also at commit, as Voron does.
- Repeating at commit the other checks of the read of a node (C1): that the page can contain data, its type, its level
  and the coherence of its entries. A changed node that is not read again passes them only if its layout includes them.
- Checking at commit the pages that are not nodes, or the integrity of what is sealed, which is computed over the bytes
  of each page as today.
- Checking on an abandon, on a commit with no changes, on making durable, on closing, on the backup, on the compaction
  or on the verification.
- Retrying a commit that fails (T11).
- Changing how the nodes of the data file, of the store's memory or of the temporary area are checked.
- Time or memory limits of the check of a very large transaction.
- A public option to check again on every read.

## Left to the implementation

- How the commit knows which changed pages are nodes and of which format.
- How the tests count and trigger the checks at commit, without a public option.
- The repetitions of the H4 runs.

## Acceptance criteria

Those of the ancestors hold, and must keep passing unchanged except those that this PRD replaces (see
[Annotations to other PRDs](#annotations-to-other-prds)). AC-H1 to AC-H6 are checked in an on-disk store and in
temporary mode. The tests use knowledge of the implementation to count the checks, to alter bytes and to free pages.

1. **AC-H1** — Given a write transaction that creates a normal tree, a fixed-size one and a multi-value one, and
   stores keys in order in each one and in the subtree of a key of the multi-value one, with splits of leaves and
   branches, when each store reads the path, then no read checks the layout of a node that the transaction changed,
   and at commit each node page that the commit writes is checked exactly once, of those trees and of the catalog,
   even if it changed several times, including the nodes that a split creates and that are not read again; and what is
   read after committing is what a model of the stored keys and values has (H1, H2). This criterion replaces AC-S1.
2. **AC-H2** — Given a node that a write transaction changed, when the test alters its layout and commits without any
   later operation reading it, in durable mode and in deferred mode, then the commit fails with the damage class and
   the number of its page; another transaction reads the keys and values from before; and the next write commits (H2).
   With two altered nodes, the error gives the lowest-numbered one. This criterion replaces AC-S2.
3. **AC-H3** — Given a node that a write transaction changed in a page that no tree references, when the test alters
   its layout and the transaction frees the page and commits, then the commit does not check it and commits (H2, F8).
4. **AC-H4** — Given a write transaction with an altered node as in AC-H2, with no operation reading it, and a later
   operation that fails with another class, when it commits, then the commit fails with the class of that operation and
   checks no node (T11, H2).
5. **AC-H5** — Given a write transaction that changes nodes and is abandoned, and another that only reserves and frees
   pages and commits, then neither checks a node (H2).
6. **AC-H6** — Given a node that a write transaction changed, when the test changes its level, and in another case its
   type, without breaking its layout, then the next read of that node in the transaction fails with the damage class
   and the number of its page (H1, C1).
7. **AC-H7** — Given the single-workload runs of H4, when they are compared, then W1 and W3 improve on both platforms
   and none of the write workloads of H4 worsens; and the closing report includes those comparisons (H4).

## Annotations to other PRDs

This PRD changes rules of earlier documents, which carry the corresponding annotations:

- Root parent, G6: the exception of I8, extended by S3, is extended with H3: within a write transaction, a read of a
  node that it changed may return data that is not what was written, fail with another class or end in a panic if a
  failure of the engine, of the hardware or of the memory left it inconsistent; the commit applies nothing in that
  case.
- Write checks: S1 ceases to hold for the reads of the nodes that the write changed (H1) and its check moves to commit
  (H2), as do its sentences about a check that fails and about what is remembered; steps 2 and 3 of the Flow, the
  second-to-last sentence of S3 and the Out of scope item "Checking the layout when sealing or committing" cease to
  hold (H2); the residual risk of S3 is extended with that of H3. AC-S1 is replaced by AC-H1 and AC-S2 by AC-H2; AC-S3
  ceases to apply, because there is no check on the read. S4 and its Validations and messages section: the commit that
  fails with the damage class is a new situation of that class, which G6 covers.
- Remembered check of temporary-area versions: in U1, the copy of a version to modify it is checked at commit (H2), not
  on its first read; in U2, the same for the copy of an altered version.
- Remembered layout check: C3, in what its annotation of 2026-11-06 says about the nodes that the write modifies,
  now says that they are checked once at commit (H2).
- Journal group commit: E2 adds to what the commit does before the point of no return the check of H2, after the
  failures of T11.

## References

- Parent: [pager-writer-checks.prd.en.md](pager-writer-checks.prd.en.md) v1.0 (S1, S3, AC-S1–AC-S3)
- Root: [krummi.prd.en.md](krummi.prd.en.md) (G6, D8)
- Related: [trees-node-check-cache.prd.en.md](trees-node-check-cache.prd.en.md) (C1–C3),
  [pager-version-checks.prd.en.md](pager-version-checks.prd.en.md) (U1, U2),
  [transactions.prd.en.md](transactions.prd.en.md) (T10, T11, T18),
  [journal-group-commit.prd.en.md](journal-group-commit.prd.en.md) (E2, E5),
  [free-space.prd.en.md](free-space.prd.en.md) (F8), [benchmark.prd.en.md](benchmark.prd.en.md) (K7)
- Performance plan: [docs/performance/plan.md](../../performance/plan.md), margin diagnosis of 2026-11-09
