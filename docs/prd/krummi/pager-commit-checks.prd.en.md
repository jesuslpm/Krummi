# Check of the Write's Nodes after Commit — PRD

**Version**: 1.0
**Date**: 2026-11-09
**Parent document**: [Write Checks — PRD](pager-writer-checks.prd.en.md) (v1.0)

> English translation of [pager-commit-checks.prd.md](pager-commit-checks.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Make write transactions that change the same nodes many times cheaper. Today, the first read of a node after each
change checks its whole layout (S1 of the write checks): a leaf that receives thirty insertions in one transaction is
checked thirty times. With this change, the write does not check the layout of the nodes that it changed itself, neither
when it reads them nor at commit: it commits them as they are, and the following transactions check each one on their
first read, as they already do today with the nodes that a write changes and does not read again.

**Why**: in the comparative benchmark (module 6), `carga-masiva` (W3) is the only threshold that Krummi misses on Linux,
at 0.78 times Voron, and on Windows it stands at 0.82 (benchmark reports of 2026-11-08). In the margin diagnosis of
2026-11-09 of the performance plan, that check takes 33% of W3 on Windows and 39% on Linux: W3 stores a million random
keys in a single write, about thirty per leaf, and each insertion checks its leaf again. The in-order loads (W1) repeat
the same pattern. According to the Voron code reviewed by the research of 2026-11-09, Voron does not check the layout
of any page in production (its check of the tree pages, `TreePage.DebugValidate`, exists only in debug builds).

Checking each changed node only once, at commit, was also measured: W3 improved, but `carga-al-azar` (W2) dropped
between 5% and 8% on Windows, because each of its commits changes about a thousand leaves that nobody reads again and
that today are not checked.

**Why now**: it is proposal W3 b) of the margin diagnosis of 2026-11-09, which the user authorized with the agent's
recommendations.

This PRD is a change to the write checks. It inherits everything from the parent and its ancestors; only when the layout
of a node that the write changed is checked changes here. What changes earlier rules is declared in its decisions and
annotated in the affected documents (see [Annotations to other PRDs](#annotations-to-other-prds)).

### Product decisions

| ID | Decision |
|---|---|
| H1 | **The nodes that the write changes** (changes S1 of the write checks and the sentence of U1 about the copy of a version). It applies only to the nodes of the trees. A write transaction changes a node when it creates it or when it obtains a copy of it to modify it, as S1 defines. The transaction does not check the layout of the nodes that it changed, neither when it reads them nor at commit: it uses them and commits them exactly as their changes left them. The other checks of each read of a node (that the page can contain data, its type, its level and the coherence of each entry that is used, C1) stay on every read. The page the copy is obtained from is checked as today when it is read as a node (C1–C3, U1). |
| H2 | **Check of what is committed**. Each later transaction checks the layout of a committed node on its first read of it, from the temporary area (U1) or from the data file or the store's memory (C1, C2). The version that the commit publishes is new: nothing that was checked of the page it was copied from holds for it (U1, C1). A node that does not pass makes that read fail, and each later read, with the damage class and the number of its page. |
| H3 | **Resulting guarantee** (extends the exception of G6 of the root parent and the residual risk of S3 of the parent). The nodes that a write changes are new or copies of nodes that the tree read and checked to reach them, so only a failure of the engine itself, or of the hardware or the memory (P16), can leave them inconsistent. In that case, a later operation of the same write that reads that node may return data that is not what was written, fail with another class or end in a panic instead of a damage error; the commit stores the inconsistent node, and from then on each read of it by another transaction fails with the damage class (H2). Today the same happens with the nodes that the write changes and does not read again (S3). It is accepted as a residual risk, as in Voron, which does not check the layout of any page. |
| H4 | **Performance**. The single-workload runs are made before and after the change, in the same session, in two rounds that alternate the order, on Windows and on Linux, with the benchmark power plan described by the measurement method of the performance plan. A workload improves when its median after exceeds that of before in both rounds of a platform. A workload that falls below in both rounds is measured in two new rounds, and worsens if it also falls below in both. A workload that is above in one round and below in the other neither improves nor worsens. The change is accepted when W1 and W3 improve on both platforms and none of the write workloads W1–W6, W13–W15, W17, W19 and W21 worsens. If this is not met, the closing report says so and the user decides; not meeting the threshold (K7) does not prevent closing the change. |

### Starting state

Verified on 2026-11-09 in the code and in the profiles of the margin diagnosis:

| Aspect | Situation |
|---|---|
| Check of the changed nodes | The first read of a node after each change checks it in full (S1). |
| Commit | It does not check the layout: a changed node that is not read again is sealed without checking (S3). |
| Cost | The layout check is 33% of W3 on Windows and 39% on Linux, almost all of it on the leaves that change in each insertion. |
| Voron (reference) | It checks the integrity code of each page of the file once per process and does not check the layout of any page in production. |

## Actors

- **Integrating developer**: reads and writes as until now; nothing they use changes.
- **Maintainer**: runs the tests and the single-workload runs of H4 on Windows and on Linux.

## Flow

1. A write transaction reads the nodes it did not change, checking them as today, and the ones it changed without
   checking their layout (H1).
2. When it commits, it stores the nodes it changed as they are (H1).
3. The first read of each committed node by another transaction checks its layout (H2).

## Validations and messages

There are no new situations or messages. What a write ceases to detect is in H3.

## Output

Nothing that an operation returns changes in form, and the format version stays `10`.

## Reuse

- The remembered layout check of the temporary-area versions (U1) and that of the store's nodes (C1, C2), unchanged.
- The single-workload runs of the performance plan, for H4.

## Out of scope

- Checking at commit the nodes that the write changed (see Objective and context).
- Changing the other checks of the read of a node (C1): that the page can contain data, its type, its level and the
  coherence of its entries.
- Changing how the nodes of the data file, of the store's memory or of the temporary area are checked.
- Detecting inside the write a node that it left inconsistent, or preventing it from committing it (H3).
- A public option to check again on every read.

## Left to the implementation

- How the tests count the checks and how they alter the bytes of a changed node, without a public option.
- The repetitions of the runs of H4.

## Acceptance criteria

Those of the ancestors apply and must keep passing unchanged, except those this PRD replaces (see
[Annotations to other PRDs](#annotations-to-other-prds)). AC-H1 to AC-H3 are checked on a store on disk and in
temporary mode. The tests use knowledge of the implementation to count the checks, to alter bytes and to keep versions
in the temporary area without moving them. AC-H4 is a measurement, not a test.

1. **AC-H1** — Given a write transaction that creates a normal tree, a fixed-size one and a multi-value one, and stores
   keys in order in each of them and in the subtree of a key of the multi-value tree, with splits of leaves and
   branches, and another that removes most of the keys of a tree and merges its nodes, when each operation reads the
   path, then no read checks the layout of a node that the transaction changed; what is read after committing is what a
   model of the stored keys and values has, and that read checks at least one committed node (H1, H2). This criterion
   replaces AC-S1.
2. **AC-H2** — Given a node whose content from before the change is in the shared memory of remembered nodes, which a
   write transaction changes, when the test alters its layout and the transaction commits without any later operation
   reading it, in durable mode and in deferred mode, then the commit passes, and two reads of that node by other
   transactions, from the temporary area and, after the move, from the data file or the store's memory, fail with the
   damage class and the number of its page (H1, H2, H3). This criterion replaces AC-S2.
3. **AC-H3** — Given a node that a write transaction changed, when the test changes its level, and in another case its
   type, without breaking its layout, then the next read of that node in the transaction fails with the damage class and
   the number of its page, and the transaction cannot commit (H1, C1, T11).
4. **AC-H4** — Given the single-workload runs of H4, when they are compared, then W1 and W3 improve on both platforms
   and none of the write workloads of H4 worsens; and the closing report includes those comparisons (H4).

## Annotations to other PRDs

This PRD changes rules of earlier documents, which carry the corresponding annotations:

- Root parent, G6: the exception of I8, extended by S3, is extended with H3: inside a write transaction, a read of a
  node that it changed may return data that is not what was written, fail with another class or end in a panic if a
  failure of the engine, the hardware or the memory left it inconsistent; the commit stores it, and each read of it by
  another transaction fails with the damage class.
- Write checks: S1 ceases to hold for the nodes that the write changed, which it does not check (H1), also its
  sentences about a check that fails and about what is remembered; steps 2 and 3 of the Flow and the Out of scope item
  "Ceasing to check the layout of the nodes that the write modifies, as Voron does" cease to hold; the residual risk of
  S3 is extended with that of H3. AC-S1 is replaced by AC-H1 and AC-S2 by AC-H2; AC-S3 no longer applies, because the
  write does not check its nodes.
- Remembered check of temporary-area versions: in U1 and U2, the copy of a version that a write obtains to modify it is
  not checked by that write; the version it commits is checked on its first read by another transaction (U1). AC-U5
  now says that the write's read of the copy does not check it, and that the first read of the committed version by
  another transaction does.
- Remembered layout check: C3, in what its annotation of 2026-11-06 says about the nodes that the write modifies, now
  says that the write does not check them (H1).

## References

- Parent: [pager-writer-checks.prd.en.md](pager-writer-checks.prd.en.md) v1.0 (S1, S3, AC-S1–AC-S3)
- Root: [krummi.prd.en.md](krummi.prd.en.md) (G6)
- Related: [trees-node-check-cache.prd.en.md](trees-node-check-cache.prd.en.md) (C1–C3),
  [pager-version-checks.prd.en.md](pager-version-checks.prd.en.md) (U1, U2),
  [benchmark.prd.en.md](benchmark.prd.en.md) (K7)
- Performance plan: [docs/performance/plan.md](../../performance/plan.md), margin diagnosis of 2026-11-09
