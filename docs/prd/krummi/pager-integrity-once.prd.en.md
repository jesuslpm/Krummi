# Remembered Page Integrity — PRD

**Version**: 1.0
**Date**: 2026-11-05
**Parent document**: [Krummi Pager — PRD](pager.prd.en.md) (v1.0)

> English translation of [pager-integrity-once.prd.md](pager-integrity-once.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Make the engine's reads cheaper by ceasing to repeat the **integrity check** (G6) of a page that has already passed it
while the store stays open. Today, each time an operation reads a page of the data file (or, in temporary mode, of the
store's memory), the engine computes the 64-bit integrity code of the whole page and compares it with the stored one; a
value stored off its leaf is checked whole on every read. A page that does not change is checked just the same on every
read.

This change makes the engine **remember the pages whose integrity it has already checked** in the current opening, as
Voron does: the first read of a page in each opening checks it; the following ones use it without computing its code
again, until the engine itself rewrites it or the store is closed. Only the read transactions use it: the write
transactions keep checking every page they read, so a damage is never sealed into a new version.

**Why**: the integrity check is the largest cost of reads. In the diagnosis of the performance plan of 2026-11-04,
prior to optimizations O1, O2 and O3a, it took 62% of the time of random reads with one thread (W7) and 76% of the
reading of fixed-size trees (W22); each read of a key checks about three pages. Krummi misses the threshold of those
workloads in the comparative benchmark (module 6), and Voron does not repeat the check.

**Why now**: O1, O2 and O3a removed waits between reads and made the computation of the code cheaper, but they do not
change how many times it is computed; it is the largest cost that remains in the read workloads.

This PRD is a change to the Pager (module 1) that arises from a missed threshold of the comparative benchmark (K13). It
inherits everything from the parent and its ancestry; only when the integrity check is repeated is added here. What
changes earlier rules is declared in its decisions and annotated in the affected documents (see
[Annotations to other PRDs](#annotations-to-other-prds)).

### Product decisions

| ID | Decision |
|---|---|
| I1 | **What is remembered**: that a page of the data file or, in temporary mode, of the store's memory passed the integrity check in the current opening. A value stored off its leaf, which is checked whole, is remembered and forgotten whole. Only passed checks are remembered: a page that does not pass it is never remembered, so it fails again on every read.<br><br>*Annotation of 2026-11-06: a value off its leaf is remembered with all its pages and with a mark of its length in the shared, bounded memory of checked contents. If another content takes the mark's slot, a read of another length from the value's first page is not checked. Only a reference with a wrong length makes that read, and that reference is in a leaf that passed its own check: it does not come from damage on disk. This residual is accepted, because closing it takes memory beyond I6 or checking the value again (against AC-I1). Closing it at no cost would need the length in the value's header, a format change with its own PRD (user's decision).* |
| I2 | **Reads that use it**: only those of the read transactions. If the page is remembered, it is used without computing its code; if not, it is checked: if it fails, the operation fails with the damage class and the page number; if it passes, from then on it is remembered. |
| I3 | **Reads that do not use it**: the write transactions check every page they read from the data file or, in temporary mode, from the store's memory, as today, even if it is remembered: when descending a tree, when copying a page to modify it (a node, a page of the free-space map, F9) and when reading the pages and values that move to other pages (splits, merges, values that move). If the check passes, the page is left remembered for the read transactions. Every other read that is not of a read transaction (that of the opening, the query of the store's information) also checks as today. Backup, compaction, migration and verification (modules 10 to 12) check every page they read, as G6 says; their PRDs set how, and whether what they check is left remembered. These do not change, and still do not check integrity: the committed versions that are still in the temporary area (J7, T16), also when a write copies them, and the copies that the write itself modifies. The header keeps its check on opening (P6), and the journal and the recovery theirs. |
| I4 | **Rewrites by the engine**: when the engine is about to write a page in its place in the data file or, in temporary mode, in the store's memory (the move, J5), that page ceases to be remembered before it is written, and with it the value stored off its leaf that contains it whole. The first later read checks it again. A write that fails or is left half done (G12) leaves the page unremembered. Forgetting a page never fails or needs memory. |
| I5 | **Scope and lifetime**: there is a single memory of remembered pages per open store, also in temporary mode, shared by all its transactions, trees and threads. It starts empty when the store finishes opening (after the recovery, G5); the pages that the file gains when it grows (P11) start unremembered. It is discarded when the store is closed and is not saved to disk: the format does not change and a store opens the same with or without this change. |
| I6 | **Memory**: what is remembered takes at most 16 KiB per GiB of the data file or, in temporary mode, of the store's memory, and grows with it. It does not count toward the maximum size (P10). Reserving it is part of opening and of growing: on opening, if the system does not give it, the opening fails with the out-of-memory class (P13); on growing, the growth fails with that class, the file keeps the size it had (P11) and the store remains usable. A growth that does not fit in the maximum size fails with its class, as today, before reserving anything. The size is internal: it is not a store option and it does not appear in its information (P12). |
| I7 | **Concurrent reads**: querying and adding remembered pages adds no blocking waits between reads or with the write. Several reads that read a non-remembered page at the same time may each check it; the result is the same. |
| I8 | **Resulting guarantee** (exception to G6 of the root parent and to C7 of the remembered layout check, annotated in them). What holds: every page of the data file or of the store's memory is checked the first time a read transaction reads it in each opening, the first time it reads it after the engine rewrites it, and every time a write transaction reads it. Thus, every damage done while the store is closed is still detected (AC10) and no write seals a damage into a new version. What changes: if the bytes of a remembered page change without the engine writing them (a hardware or memory failure, or a foreign program, which P16 leaves out of scope), the read transactions do not detect it until the store is opened again or the engine rewrites it. Since the layout check of a remembered node (C2) uses the code stored in the page as it is read, it is not repeated either if that code did not change. In that case, a read may return data that is not what was written or end in a panic instead of a damage error (and not with the error of G12). It is accepted as a residual risk: it affects what the read transactions read, never what the engine itself stores. |
| I9 | **Performance** (acceptance; refines the annotation of 2026-11-04 to K13, annotated there): the baseline is the full runs of the comparative benchmark on Windows and on Linux over the commit prior to the change. The change is accepted when, in the later full runs of each platform, no workload lowers its ratio with Voron by more than 0.05 with respect to that baseline and the workloads it attacks, W7 and W22, raise it by more than 0.05; 0.05 is the noise set by the performance plan, in absolute value of the ratio. The closing report shows whether W7 and W22 meet their threshold (K7) and includes single-workload runs of a read workload before and after the change on the two platforms. Not meeting the threshold does not prevent closing the change: what is missing follows the performance plan. |

### Starting state

Verified on 2026-11-05 in the code:

| Aspect | Situation |
|---|---|
| Integrity check | 64-bit code over the whole page (which includes its number), on every read of a page of the data file or of the temporary store's memory: nodes, pages of the free-space map and the metadata page that is read on opening. A value stored off its leaf carries a single code for all its pages and is checked whole on every read; one of 256 MiB is about 32,769 pages. |
| Reads with no check | The versions of the temporary area and the copies that the write modifies; the header has its own check, only on opening. |
| Writes with the store open | The move writes in their place the versions sealed at the commit, without checking them again; the recovery checks what it applies before writing it, on opening. Growth adds empty pages, which do not pass the check. |
| Memory per opening | The remembered layout check (C1–C9) takes a fixed size. There is no memory that grows with the number of pages. |
| Damage tests | Those of the sweep of every byte (AC10) alter the files with the store closed and reopen it. Two tests alter a page of the data file with the store open and expect damage on the next read: AC-C7 and the fourth part of AC-X19. |
| Voron (reference) | According to the performance plan (RavenDB 7.2.7), it marks in a bitmap each page the first time it passes its check after opening and does not check the pages beyond the size of the file on opening, because that same opening wrote them. |

The cost comes from the diagnosis of the performance plan of 2026-11-04: the integrity check was 62.4% of W7 and
18.5% of W9. O3a (2026-11-05) made the computation about 1.8 times faster; there is no later profile.

## Actors

- **Integrating developer**: reads and writes as until now; nothing they use changes.
- **Maintainer**: runs the tests, the full runs of the benchmark and the single-workload runs of I9 on Windows and on
  Linux.

## Flow

1. The application opens a store; after the recovery, the engine starts with no remembered pages (I5).
2. The first read of a page in the opening checks it and, if it passes, remembers it; the following read transactions,
   from any thread, use it without checking it (I1, I2, I7).
3. A write transaction checks every page it reads, as today (I3); the new version of a page it modifies is read from
   the temporary area as today. When the move writes it in its place, it ceases to be remembered and the first later
   read checks it (I4).
4. When the store is closed, what is remembered is discarded; the next opening checks every page again the first time
   it reads it.

## Validations and messages

There are no new situations, error classes or error data. A page whose integrity fails still fails with the damage
class and its page number in every operation that needs it, in the same order as today. Not being able to reserve the
memory of I6 is the out-of-memory class of P13.

## Output

Nothing that an operation returns changes in form: the same data, the same error classes and the same error data. The
store information (P12) adds no data and the format version stays `9`. For example, the second read of a damaged page
fails the same as the first, because a page that does not pass the check is never remembered:

| Error data | First read | Second read |
|---|---|---|
| Class | damage | damage |
| Folder | the store's | the store's |
| Page | `<number>` | `<number>` |

## Reuse

- The integrity check of the pages and of the values stored off their leaf, and the damage class (G6), unchanged in
  what they check.
- The remembered layout check (C1–C9), which keeps remembering nodes by their page number and the code stored in it,
  and its coherence rule (C8), on which I4 also depends.
- The rules of the move (J5, T14).
- The comparative benchmark (module 6) and the single-workload runs of the performance plan, for I9.

## Out of scope

- Checking the integrity once per transaction instead of once per opening.
- Checking the integrity of pages that are not read, in the background or on opening (as in [pager.prd.en.md](pager.prd.en.md)).
- Leaving unchecked, as Voron does, the pages that the file gains after opening: they are checked the first time they
  are read, like the others (I5).
- Using what is remembered in the write transactions, and in backup, compaction, migration and verification (I3); the
  criteria of those four operations belong to their PRDs (modules 10 to 12).
- Checking the integrity of the versions of the temporary area and of the copies that the write modifies (I3).
- Changing the check of the header (P6), that of the journal or that of the recovery.
- Detecting that another program, or a hardware or memory failure, changes a remembered page while the store is open
  (I8, P16).
- Saving what is remembered between openings.
- A public option to turn it off or limit its memory, and statistics of remembered pages in the store information (I6).
- Stronger or cryptographic integrity codes.
- Encryption (module 9): it decides whether it uses what is remembered and over which bytes.
- Requiring W7 or W22 to meet their threshold to close the change (I9).

## Left to the implementation

- The form of the memory of remembered pages (for example, a bitmap), how it grows with the file and at what moment of
  the growth it is reserved, within I6.
- How a value stored off its leaf is remembered and forgotten (for example, by its first page), within I1 and I4.
- How the tests count the integrity checks without a public option.
- The exact workloads and the repetitions of the single-workload runs of I9.

## Acceptance criteria

Those of the ancestors apply and must keep passing unchanged, except those this PRD replaces or refines (see
[Annotations to other PRDs](#annotations-to-other-prds)), including AC10 of the root parent. Each criterion is checked
on a store on disk and in temporary mode, except for the parts that reopen the store, which are on disk only. The tests
use knowledge of the implementation to count the integrity checks, to place pages in the data file or in the temporary
area, to alter bytes and to simulate the lack of memory. The counts of checks are made with single-thread reads (I7).

1. **AC-I1** — Given a tree and a value stored off its leaf with their pages in the data file, when a read transaction
   reads them several times and another read transaction reads them afterwards, then all return the same and the
   integrity of each page and of the value is checked only on its first read; and after closing and reopening the
   store, the first read checks it again (I1, I2, I5).
2. **AC-I2** — Given the nodes of a tree, remembered by an earlier read transaction, and a page of the free-space map,
   remembered by an earlier write (I3), all in the data file, when a write transaction reads a key twice and then
   stores one that modifies a leaf and reserves pages, then it checks the integrity of every page it reads from the
   data file on every read, including the leaf and the map page it copies; and a later read transaction does not check
   again the nodes that the write did not modify (I2, I3).
3. **AC-I3** — Given a committed version that is still in the temporary area, and the copies that a write modifies,
   when they are read, then their integrity is not checked, as today (I3).
4. **AC-I4** — Given a remembered page, when a write changes its content and commits while a read transaction opened
   earlier keeps reading it, then that transaction keeps seeing the earlier content; and when that transaction closes
   and the new version is moved to its place, then a new read transaction sees the new content and its integrity is
   checked once (I4, C8, G3).
5. **AC-I5** — Given a remembered value stored off its leaf, when it is deleted and a later write reuses its pages for
   other content that reaches the data file, then the read of the new content checks it once and returns what was
   written (I1, I4).
6. **AC-I6** — Given a page of the data file altered with the store closed, when it is opened and read twice in the
   same read transaction and once in another, then each read fails with the damage class and the number of that page
   (I1, I2, G6).
7. **AC-I7** — Given a page that has not yet been read in the current opening, when the test alters one of its bytes
   with the store open and then it is read, then the read fails with the damage class and the number of that page (I2,
   I8).
8. **AC-I8** — Given a remembered page with no pending versions to move, when the test alters one of its bytes with the
   store open, then a write transaction that reads it fails with the damage class and the number of that page, in that
   operation, and cannot commit (T11); and after closing and reopening the store, the first read of that page fails the
   same (I3, I5, I8). This criterion replaces AC-C7 and the fourth part of AC-X19.
9. **AC-I9** — Given correct pages and a damaged one as in AC-I6, when four threads with read transactions read them at
   the same time, at least a thousand times each, while a write changes the correct ones and they are moved, then each
   read returns the data of its snapshot and the damaged page fails on all its reads with the damage class (I4, I7).
10. **AC-I10** — Given a store with remembered pages, when it grows while being read, then the pages remembered before
    growing that the move did not rewrite are still not checked again, and the new pages are checked the first time
    they are read (I4, I5, I6).
11. **AC-I11** — Given a simulated lack of memory when reserving the memory of I6, when a store is opened, then the
    opening fails with the out-of-memory class; and when an open store grows, then the growth fails with that class,
    the store size in its information does not change and the store keeps accepting reads and writes (I6, P13).
12. **AC-I12** — Given the full runs of the benchmark on Windows and on Linux over the commit prior to the change and
    over the change, when they are compared per platform, then no workload lowers its ratio with Voron by more than
    0.05 and W7 and W22 raise it by more than 0.05; and the closing report includes those comparisons, whether W7 and
    W22 meet their threshold and the single-workload runs of a read workload before and after (I9).

    *Annotation of 2026-11-06: a workload whose ratio drops by more than 0.05 between two full runs counts as
    worse only when single-workload runs, before and after in the same session, confirm the drop. Full runs on
    different days move by more than 0.05, also for the engines that do not change. Read this way, AC-I12 is
    met after O3 and O4: none of the workloads that dropped in the full runs of 2026-11-06 drops in its
    single-workload runs against the commit before O3 (user's decision).*

## Annotations to other PRDs

This PRD changes or refines rules of earlier documents, which carry the corresponding annotations:

- Root parent, G6 and AC10: exception of I8. In the read transactions, the integrity of a page is checked the first
  time it is read in each opening and after each rewrite by the engine; a change of a remembered page made while the
  store is open by something that is not the engine may return data that is not what was written or end in a panic
  until reopening. The write transactions, backup, compaction, migration and verification still check everything they
  read. AC10 does not change, because its alterations are made with the store closed.
- Comparative benchmark, annotation of 2026-11-04 to K13: this change has a PRD and its acceptance is that of I9,
  which does not require W7 and W22 to meet their threshold.
- Pager, P10, P11 and P13: the memory of remembered pages grows with the store, does not count toward the maximum size
  and not being able to reserve it is out of memory; a growth that fails because of it leaves the file at the size it
  had (I6).
- Remembered layout check: the Objective, the Flow, C2, C3, C5, C7 and the first point of Out of scope assumed the
  integrity check on every read; it is now made as I2 and I3 set, and what is remembered of a node is looked up with
  the code stored in its page as it is read. C3 now says: the reads of nodes of the data file or, in temporary mode, of
  the store's memory use what is remembered. The residual risk of C7 is extended with that of I8. AC-C1 is refined: the
  integrity of each node is checked on its first read, not on all of them. AC-C7 is replaced by AC-I8.
- Fixed-size trees, AC-X19: its first part is refined as AC-C1, and its fourth part (a byte changed with the store open
  in a remembered leaf) is replaced by AC-I8.
- Trees and key operations (A9), multi-value trees (M11) and fixed-size trees (X10): a damaged page still fails in
  every operation that needs it, except for the residual risk of I8, which replaces that of C7.
- Large values, V7 and Left to the implementation: a damaged value stored off its leaf still fails every time it is
  needed (I1), except for the residual risk of I8; it ceases to be open whether its integrity is checked on every read
  or once per transaction, because I1 to I4 set it.
- B+ trees module, Reuse: the integrity check of the pages ceases to be made on every read of the data file and
  follows I2 and I3, with the residual risk of I8.

## References

- Parent: [pager.prd.en.md](pager.prd.en.md) v1.0
- Root: [krummi.prd.en.md](krummi.prd.en.md) v1.1 (G5, G6, G12, AC10)
- Related: [trees-node-check-cache.prd.en.md](trees-node-check-cache.prd.en.md) (C1–C9),
  [trees-fixed-size.prd.en.md](trees-fixed-size.prd.en.md) (X10, AC-X19), [trees-keys.prd.en.md](trees-keys.prd.en.md) (A9),
  [trees-multivalue.prd.en.md](trees-multivalue.prd.en.md) (M11), [journal.prd.en.md](journal.prd.en.md) (J5, J7),
  [transactions.prd.en.md](transactions.prd.en.md) (T14, T16), [free-space.prd.en.md](free-space.prd.en.md) (F9),
  [benchmark.prd.en.md](benchmark.prd.en.md) (K7, K13)
- Performance plan: [docs/performance/plan.md](../../performance/plan.md), O3
