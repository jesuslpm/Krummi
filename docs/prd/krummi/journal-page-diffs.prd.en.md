# Journal of the changes of each page — PRD

**Version**: 1.0
**Date**: 2026-11-08
**Parent document**: [Krummi Journal and Recovery — PRD](journal.prd.en.md) (v1.0)

## Objective and context

Make commits cheaper by keeping in the journal only **the bytes that change** in each page, instead of the whole page.
Today, each commit writes to the journal a full copy of every page it modified, compressed when compression pays
(J16). Storing a 100-byte entry in a leaf changes a few hundred bytes of an 8 KiB page, yet the journal receives,
compresses and carries to disk the whole page, and the same for each branch on the path that changes.

With this change, for each page that the commit modified starting from an earlier version, the journal keeps the new
value of the bytes that changed, and for each page that the commit created, the whole page without its parts that are
zeros. Recovery applies that in order and arrives at the same pages as today. So that deleting an entry or changing its
size changes few bytes, the engine stops reorganizing the node on every delete: the place the entry leaves stays as a
**hole** until it is needed for a new entry.

**Why**: in the comparative benchmark (module 6), Krummi misses the threshold of the random-write workloads:
`carga-al-azar` (W2) 0.40 on Windows and 0.48 on Linux, `borrado` (W14) 0.47 and 0.55, `reutilizacion` (W15) 0.42 and
0.52 and `multivalor-carga` (W19) 0.37 and 0.55 (reports `2026-11-07-*-write-reuse` in `docs/benchmark`). According to
O8 of the performance plan, journal compression is 36–40 % of W2, W14, W15 and W19 and 23 % of W21. The Linux profiles
of 2026-11-08 put 61 % of `tamano-fijo-carga` (W21, 0.65 and 0.71) in the commit (copying, compressing and computing
the check code of whole pages); W21 gains only with Y1, because its nodes do not change (Y4). Voron keeps in its
journal the differences of each page against its earlier version.

**Why now**: it is O8 of the performance plan, the largest lever left for random writes, and the first of the
2026-11-08 investigation into what separates Krummi from Voron. It reverses the Out of scope point of group commit
that left for later "keeping only the differences of each page in the journal".

This PRD is a change of the journal and of the organization of nodes. It inherits everything from the parent and its
ancestors; here we only add what the journal keeps of each page, how recovery applies it and when a node is
reorganized. What changes from earlier rules is declared in its decisions and annotated in the affected documents (see
[Annotations to other PRDs](#annotations-to-other-prds)).

### Product decisions

| ID | Decision |
|---|---|
| Y1 | **What the journal keeps of each page** (refines J16). Of a page that the commit obtained from its last committed version in order to modify it, the journal keeps the new value of each byte that differs from that version, never its change with respect to the old value. Such a page that changed no byte is not kept. Of a page that the commit created (a page it reserves, new or previously freed, and each page of an off-leaf value, also when it reuses pages), the journal always keeps the whole page, without writing its parts that are zeros; a created page that is all zeros is also recorded. What is kept of a page takes up at most the whole page plus a few bytes that say where each part goes. It is then compressed as today when it takes less space compressed. |
| Y2 | **Recovery** (changes J8). The open applies to each page of `krummi.data`, in the order in which they were committed, what the journal keeps of it since what is recorded as transferred: a created page is written whole, with zeros where nothing was kept; for the others, the kept bytes are written over the content the page has in `krummi.data`, or over zeros if the page is beyond its end. Since each kept byte is its final value in that commit, the result is the last committed version of each page even if a crash cut a transfer and left in `krummi.data` pages with bytes from several versions later than what was transferred, even if an earlier recovery is cut, and even if recovery starts from the oldest copy of the header (J8). When it finishes applying, the open checks the integrity (G6) of each page it applied whose last application was not a page of an off-leaf value, which has none as a page. What J9 calls damage of the journal is still detected before touching the store, as today. |
| Y3 | **Damaged page when recovering** (changes J8 and G5 in this). If an applied page fails the check of Y2, the open fails with the damage class and the number of the lowest of those pages, after having applied everything else: the store is left modified, and `krummi.data` may have grown, as in a cut recovery. The next open repeats the recovery and fails the same way. It can happen when its content in `krummi.data` was damaged by something that is not a cut transfer (a disk failure or a foreign program, P16), or when J9 discards as damaged a commit that had already been partly transferred. Today, in those cases, the pages the journal keeps are rewritten whole without error. It is accepted: recovery cannot tell those damages apart from an engine failure, and it does not hide them. If something else fails first (lack of space or memory, an input/output failure), the open fails with that class as today (J8), and the next one repeats. |
| Y4 | **Holes in nodes**. In all the nodes of the trees with variable-size entries (normal, the catalog, multi-value trees and the subtrees of their keys), deleting an entry or replacing it with another of a different size leaves its place as a hole, without moving the other entries, also in temporary mode; replacing an entry with another of the same size changes it in place, as today. The bytes of a hole are not erased, just as today those of the free space are not. A node without entries has no holes. A node is reorganized to use its holes only when a new entry does not fit without them and does fit with them. What a node takes up in order to split, merge or be considered underfull is what its entries take up, as today, without counting its holes: no tree has more or fewer pages than today after the same operations. The layout check of a node accepts holes and still rejects entries that overlap, that go outside the page or that exceed their maximum sizes, and whatever the node says about its holes that does not match its entries. Fixed-size trees do not change. |
| Y5 | **Format `10`**: the journal records and the organization of nodes change, so the current format version becomes `10` (J3) and a store of format `9` is of an earlier format (D7 of the root parent), with the same error as today. |
| Y6 | **No other visible changes**: there are no new operations, options, situations, error classes or error data. What operations return, what is durable and when (G2, J4, J10), what is discarded after a crash (J9) and the store information (P12) do not change, except the format version. The journal files follow the rules of J2 and E7; they fill up more slowly. |
| Y7 | **Performance**: as S5 of the write checks, with at least W1, W2, W3, W4, W7, W10, W12, W13, W14, W15, W17, W19 and W21 measured. The change is accepted when W2, W14, W15 and W19 improve on both platforms and no measured workload gets worse. The full reference runs are those of 2026-11-07 after the reuse of write resources (`docs/benchmark/2026-11-07-*-write-reuse.md`). |

### Starting state

Verified on 2026-11-08 in the code, in the profiles of that day and in the performance plan:

| Aspect | Situation |
|---|---|
| Journal records | Each commit keeps, of each page it modified, its number and the whole sealed page; the set is compressed when it takes less space, and is kept uncompressed if not. The pages of an off-leaf value go whole, marked so they are not checked one by one. |
| Recovery | It checks each page of a record before writing any, and writes it whole in its place, except those of off-leaf values, which it does not check. A page of `krummi.data` cut by a transfer is overwritten whole. |
| Deleting in a node | It moves the entries so that they take up the end of the page without holes; a node with holes does not pass the layout check. Replacing an entry with another of a different size, in any tree with variable-size entries, deletes it and inserts it again; with the same size, it changes it in place. |
| Measured cost | According to O8 of the plan, journal compression: 36–40 % of W2, W14, W15 and W19 and 23 % of W21. Linux profiles of 2026-11-08: the commit is 61 % of W21, and in W17 preparing the record is 30 %. |
| Format | Version `9`. |
| Voron (reference) | According to the RavenDB 7.2.7 code reviewed by the 2026-11-08 investigation: it keeps in its journal the differences of each page against its earlier version, or its non-zero parts if it is new, and compresses them. |

## Actors

- **Integrating developer**: reads and writes as until now; a store of format `9` must be recreated (Y5).
- **Maintainer**: runs the tests, the crash tests and the single-workload and full runs of Y7 on Windows and on Linux.

## Flow

1. A write modifies pages: each one starts from its last committed version or is created.
2. On commit, the journal receives from each page what Y1 says.
3. The transfer writes whole pages to `krummi.data`, as today.
4. After a crash, the open applies what the journal keeps since what was transferred, in order, and checks each page
   it applied (Y2, Y3).
5. Deleting an entry or changing its size leaves a hole; the node is reorganized only when needed (Y4).

## Validations and messages

There are no new messages, error classes or error data.

| Situation | Result |
|---|---|
| Opening a store of format `9` | It fails as today with a store of an earlier format (Y5). |
| A page applied in recovery fails its integrity check | The open fails with the damage class and the number of the lowest of those pages (Y3). |
| A node with holes that do not overlap its entries | It passes the layout check (Y4). |
| A node with entries that overlap, that go outside the page or that exceed their maximum sizes | It fails with the damage class and the number of its page, as today (Y4). |

## Output

Nothing that an operation returns changes in shape. The store information (P12) reports the format version `10` (Y5).

## Reuse

- Journal compression, the integrity check of each record and the detection of damage of the journal (J9, J16).
- The order of the records and group writes (E1–E15).
- The integrity check of pages (G6), to check the applied pages.
- The single-workload and full runs of the performance plan, for Y7.

## Out of scope

- Keeping differences in the temporary area or in `krummi.data`: the versions of the temporary area and the transfer
  stay with whole pages.
- Holes in fixed-size trees.
- Erasing the bytes of deleted entries or of holes.
- Reorganizing nodes with holes in the background or on transfer.
- Migrating stores of format `9` (D7).
- Repairing in recovery a damaged page of `krummi.data` (Y3), or checking and rebuilding the pages in memory before
  writing them.
- Compressing with another algorithm, or choosing compression per record or per page.
- Changing when it transfers, when the header is rewritten or when the journal files are deleted (J5, J6).
- Leaf page compression (5.6), encryption (module 9) and what backup, compaction and verification (modules 10 to 12)
  do with holes: each decides it.
- The copy into the temporary area of the pages of an off-leaf value and the other optimizations of the performance
  plan.

## Left to the implementation

- How the kept bytes are computed and encoded: their granularity, how they are grouped and the organization of each
  record, within Y1.
- How the commit finds the last committed version of each page.
- How recovery applies what is kept and how it remembers which pages to check at the end, within Y2 and Y3.
- How a node knows how much its holes take up and how it is reorganized.
- How the tests simulate a page cut by a transfer, measure what is kept in the journal and force a reorganization on
  every delete in order to compare.

## Acceptance criteria

Those of the ancestors apply, and must keep passing unchanged except those that name the format version, which becomes
`10`, including those of crashes and recovery of the journal, AC10 of the root parent and those of tree damage. Each
criterion is checked on a store on disk; those that are not about the journal, also in temporary mode. The tests use
knowledge of the implementation to measure what is kept in the journal, to simulate crashes and cut pages, to alter
bytes and to force a reorganization on every delete.

1. **AC-Y1** — Given a normal tree with a half-full leaf of entries with a 16-byte key and a 100-byte value, when three
   separate commits change the value of an entry to another of the same size, store a new entry without splitting the
   leaf and delete an entry from the middle, then what the journal keeps of that leaf in each one, before compressing,
   takes up less than a quarter of a page; and so does what it keeps of a page that a commit creates with a single
   entry (Y1).
2. **AC-Y2** — Given a fixed sequence of commits that modify the same pages several times, create new pages, free and
   reuse others, store and replace off-leaf values and create a page that is all zeros, when the process crashes
   without transferring and is reopened, then each key has its last committed value and each page of `krummi.data`,
   except those of the header, is equal to the one the same sequence leaves without a crash, with the transfer stopped
   until the close (Y1, Y2).
3. **AC-Y3** — Given the sequence of AC-Y2 with at least three versions of several pages, when the test leaves in
   `krummi.data` those pages with randomly chosen bytes, with a fixed seed, between the transferred version and any
   later version, also skipping versions, and the open recovers, then each page is its last committed version; also
   when recovery starts from the oldest copy of the header and when a first recovery is cut halfway (Y2).
4. **AC-Y4** — Given a page with a pending commit in the journal, when the test alters in `krummi.data` a byte that
   what is kept of that page does not include and the open recovers, then it fails with the damage class and the number
   of that page, and a second open fails the same way (Y3).
5. **AC-Y5** — Given a fixed sequence of stores, deletes in the middle and replacements with entries of a different
   size in a normal tree and in a multi-value tree, when it is run and read, then each read returns what was committed,
   each node passes the layout check and some node was reorganized to use its holes; and the number of pages of each
   tree is the same as with the reorganization forced on every delete (Y4).
6. **AC-Y6** — Given a node with two entries that overlap, or one that goes outside the page, or one that exceeds its
   maximum size, or whose record of holes does not match its entries, when it is read, then it fails with the damage
   class and the number of its page (Y4).
7. **AC-Y7** — Given a store of format `9`, when it is opened, then it fails as a store of an earlier format; and a new
   store reports the format version `10` (Y5).
8. **AC-Y8** — Given the single-workload runs of Y7, when they are compared, then W2, W14, W15 and W19 improve on both
   platforms and no measured workload gets worse; and the closing report includes those comparisons and those of the
   full runs with the reference ones (Y7).

## Annotations to other PRDs

This PRD changes or refines rules of earlier documents, which carry the corresponding annotations, also in their
translations (`*.prd.en.md`):

- Journal and recovery: J16 refines what is kept of each page (Y1); J8 changes in how it is applied, in that each
  applied page is checked at the end and in that a damaged page is reported after applying everything else (Y2, Y3);
  J3, the current format version becomes `10` and a store of format `9` is of an earlier format (Y5).
- Group writes: the Out of scope point "Keeping only the differences of each page in the journal" comes with this
  change; E15, the current format version becomes `10`.
- Pager, P5: the current format version becomes `10`.
- Root parent: D7, the current format version is `10` (Y5); G5, a recovery that finds a damaged page fails after
  modifying the store (Y3).
- B+ trees and key trees and operations (A9): the organization of a node admits holes; layout damage is an overlap, an
  entry outside the page or its sizes, or a record of holes that does not match (Y4).
- Remembered layout check: the Objective describes the organization of a node as "without holes or overlaps"; it
  becomes "without overlaps", with holes allowed (Y4).
- Remembered check of the versions of the temporary area, U4: the current format version is `10`.

## References

- Parent: [journal.prd.en.md](journal.prd.en.md) v1.0 (J2, J3, J8, J9, J16, J17)
- Group writes: [journal-group-commit.prd.en.md](journal-group-commit.prd.en.md) (E7, E15, Out of scope)
- Root: [krummi.prd.en.md](krummi.prd.en.md) v1.1 (D7, G2, G5, G6, AC10)
- Related: [pager.prd.en.md](pager.prd.en.md) (P5, P12, P16), [trees.prd.en.md](trees.prd.en.md),
  [trees-keys.prd.en.md](trees-keys.prd.en.md) (A9),
  [trees-node-check-cache.prd.en.md](trees-node-check-cache.prd.en.md),
  [pager-version-checks.prd.en.md](pager-version-checks.prd.en.md) (U4),
  [pager-writer-checks.prd.en.md](pager-writer-checks.prd.en.md) (S5),
  [trees-multivalue.prd.en.md](trees-multivalue.prd.en.md), [trees-fixed-size.prd.en.md](trees-fixed-size.prd.en.md),
  [benchmark.prd.en.md](benchmark.prd.en.md) (K7, K13)
- Performance plan: [docs/performance/plan.md](../../performance/plan.md), O8
