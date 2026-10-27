# Krummi Free Space — PRD

**Version**: 1.0
**Date**: 2026-10-24
**Parent document**: [Krummi, an embedded storage engine inspired by Voron — PRD](krummi.prd.en.md) (v1.1)

## Objective and context

Build the **free space**, module 4 of the [parent's split](krummi.prd.en.md#split-into-child-prds): the reuse of
the pages that transactions free, one at a time or in ranges of contiguous pages, without breaking snapshot
isolation (G3) or atomicity and recovery (G1, G5).

When this module is closed, writes can free pages and reserve one page or a range of N contiguous pages, and a
reservation uses the freed pages before growing the store. The store information says how many free pages it has.
Applications still do not free anything by themselves: the data structures starting with the B+ trees (module 5)
will do it, reserving and freeing through this module.

**Why now**: until today the pages that a write stops using are not reused (T14 of
[transactions.prd.en.md](transactions.prd.en.md)), and the B+ trees of module 5 free and reserve pages
continuously. The free space manager has to exist beforehand and not depend on them: a manager stored in a B+ tree
needs to reserve pages for its own nodes through itself, and that circular dependency forces special reservations
and exceptions that this design avoids.

This PRD inherits everything from the parent, from the [Pager](pager.prd.en.md) (P1–P17), from
[Transactions](transactions.prd.en.md) (T1–T18), from the [Journal](journal.prd.en.md) (J1–J17) and from the
[group and direct journal writes](journal-group-commit.prd.en.md) (E1–E16); here only what is specific to the
module is added. What changes from earlier rules is declared in its decisions and annotated in the affected
documents (see [Annotations to other PRDs](#annotations-to-other-prds)).

In this document, the **map** is the persistent record of which pages are free; a **region** is a fixed stretch of
consecutive pages of `krummi.data` with its own map page; the **frontier** is the page number from which no page
has ever been used; and the **store-owned pages** are those of the header, the metadata page and the map pages.

### Product decisions

| ID | Decision |
|---|---|
| F1 | **Public surface**: the store information (P12) adds the number of free pages (F10). Reserving one page or a range of contiguous pages and freeing pages inside a write transaction is internal to the library, as in P1 and T1: the following modules and the tests use it, not the applications. |
| F2 | **Map by regions**: `krummi.data` is divided into fixed-size regions, each with a map page that records which pages of the region are free. The map page is the first of its region, except in the first region, where it goes right after the header and the metadata page. Map pages are store pages like the others: a transaction modifies them with a copy, they travel in its commit through the journal and are recovered with it (G1, G5, J8). Their fixed position means they are never reserved through the manager itself, so the map does not depend on any structure of module 5 nor needs special reservations. The format of the map is its own (D1). |
| F3 | **Frontier**: the pages from the frontier on have never been used and are free without being in the map; the map records the free pages below it. A reservation that finds no room below the frontier advances it, and the file grows if needed (P11). The frontier enters a new region when a reservation uses the first page of that region after its map page: the transaction that advances it then writes that map page, which becomes occupied; before that, the map position of that region is neither occupied nor free. Like every change, the advance of the frontier is applied only if the transaction is committed. The frontier never goes back, and on reopen, also after a recovery, `krummi.data` reaches at least up to it, unless the file was trimmed while the store was closed: then a map page that is missing is a damaged page (G6, missing pages row of the Pager) and a reservation of missing pages makes the file grow. |
| F4 | **Reserving one page**: if there is any free page below the frontier, in any region, the reservation uses one of them; only if there is none does it advance the frontier. |
| F5 | **Reserving a range**: a transaction can reserve exactly N contiguous pages, without rounding to a fixed size, from 1 up to the **cap**: the pages of a region minus its map page (about 511 MiB). In the first region, which also has the header and the metadata page, the longest possible range is somewhat smaller, so a range the size of the cap always goes in another region. A range never includes store-owned pages and never crosses from one region to another. The rules, in order: (1) if there is below the frontier a free range of at least N consecutive pages, the reservation uses it; (2) if not, even if there are N loose free pages, the range is taken at the frontier: it starts at it if the N pages fit in what remains of its region; if they do not fit, it starts at the first page after the map page of the next region, and the pages that are skipped stay free and available for other reservations; (3) the necessary growth, including the skipped pages, follows P10, P11 and G7: if it does not fit in the maximum size, the reservation or, at the latest, the commit fails with the maximum size class, and if it does not fit on the disk, with the disk full class, and in both cases T11 follows. The manager does not split a value into several ranges, does not move pages to join gaps and does not compact: that belongs to the structures that use it (modules 5 and 8) and to compaction (module 11). There is no fragmented space error. |
| F6 | **When a freed page is reused**: a page that a write transaction frees is available for the reservations of every write that gets the turn after the point of no return of the commit that frees it (E2), without waiting for the open reads to finish. The reads that could already see the page keep seeing its previous content (T14, J5, J7). Inside the transaction that frees it, the page is not available for its own reservations. If the transaction is aborted or its commit fails before the point of no return (T10, T11), the freeing is not applied, like the rest of its changes; if it fails after, the write that reused the page follows E8 like any other that saw that commit. Recovery never brings a reuse without the freeing that allowed it (J10, E9). It holds the same in both commit modes. There is no queue of freed pages waiting for the oldest reader. |
| F7 | **Long reads and growth** (what the parent leaves to this module): a long-running read does not prevent reusing freed pages (F6) and `krummi.data` does not grow because of it. What it retains are the previous versions of the pages that are modified or reused while it is open, as T14 and J7 say. |
| F8 | **Reserving and freeing inside a write**: a reserved page starts with all its bytes at zero for the transaction and is part of its commit even if the transaction does not modify it. A transaction can free pages in use according to what it sees (T2, E3), loose or in ranges, including those it reserved itself; a page reserved as part of a range can be freed alone or with others of the range, and the rest remain in use. Freeing a page discards the changes the transaction had made to it and does not change its content in `krummi.data`. Once freed, the transaction can no longer read or modify it. Reserving and freeing are changes: a transaction that does it is not a write without changes (T10, E11), and the references to its pages stop being valid as after any other change (T8). |
| F9 | **Reading the map on demand**: opening does not read the map pages; each region is read the first time a reservation or a freeing needs it, and its map page passes the integrity check of every page (G6). If it is damaged, the operation fails with the damage class and the number of that page, and T11 follows. Reads, writes that do not need that region and the store information keep working. A damaged map is never used as valid, not even to skip the region: every reservation or freeing that reads it fails again in the same way. |
| F10 | **Free pages in the information** (extends P12): the store information gives the sum of two parts. The first is the free pages of the map according to what is committed and durable, as a read opened at that moment would see it (E3). The second is the pages between that durable frontier and the end of `krummi.data` as it is (P12), or of the store memory in temporary mode, not counting the map positions of the regions the frontier has not yet reached (F3), and it is zero if the file ends before the frontier. Thus, what open transactions or commits that are not yet durable free or reserve does not change the value, and the pages by which an open, aborted or not yet durable write made the file grow count as free. Since that growth may not survive a crash (P11, J8), the value may be lower on reopen. The first part is a total that is saved in each commit together with the map changes and travels with it through the journal; it is never computed from the in-memory state of the regions read since opening (F9), so it does not depend on which regions have been read. The query works without reading the map, so it keeps working always (P12), also with a damaged map page (F9). |
| F11 | **Format `5`**: the map changes what an earlier engine can read and write safely, so the current format version becomes `5` (D7 of the parent). A store of format `4` is of an earlier format even if it was closed without error, and before 1.0 it is not migrated. It refines P5 and E15. |
| F12 | **Temporary mode**: free space works the same (G9): reservations reuse the freed pages before growing the store memory, and what is committed counts for F10 as soon as the commit returns (E14). |
| F13 | **Measurement**: the change includes a reproducible measurement, on Windows and on Linux, of the time to reserve a page and a range of 16, 256 and 4,096 pages in a store of at least 1 million pages with half of them freed in loose gaps, and of the time to open that store. It sets no threshold (D5 of the parent); its results go in the closing report. |
| F14 | **Error data** (D8 of the parent, T18): the errors of this module carry the same data as those of T18, the store folder or that it is temporary; the damage one also carries the number of the damaged map page. |

*Annotation of 2026-10-25 from trees and key operations ([trees-keys.prd.en.md](trees-keys.prd.en.md)): F11, the current format version becomes `6`; a store of format `5` is of an older format (decision A13).*

*Annotation of 2026-10-27 from large values ([trees-large-values.prd.en.md](trees-large-values.prd.en.md)): F11, the current format version becomes `7`; a store of format `6` is of an earlier format (decision V9).*

### Starting state

Verified on 2026-10-24 in the code and in the public code of Voron v6.0:

| Aspect | Situation |
|---|---|
| Page reservation | A write can only reserve one page at a time, which starts at zero. Pages come from a counter that only goes up, saved in a metadata page that goes right after the two copies of the header and that travels through the journal in the commits that reserve. There are no ranges of contiguous pages. |
| Freed pages | There is no way to free a page nor any record of free pages. No test or technical document deals with free space. |
| Versions | A page keeps its number when it is modified: the new version is saved in `krummi.temp` and written in its place in `krummi.data` when no open read needs the previous one and it is durable. Modifying a page never frees another. |
| Journal | Each commit saves in full the pages it changes, compressed when they take less space, and recovery applies them in order. |
| Growth | A new store takes 64 KiB; the file doubles its size at each jump up to jumps of 1 GiB. Each growth projects the whole file into memory again, so a contiguous range in `krummi.data` is also contiguous in the projection. In temporary mode memory grows in separate chunks. |
| Header | It has no free field for the free space manager; the current format is `4`. |
| Voron (reference) | It stores free space in a fixed-size tree with a bitmap for each section of 2,048 pages, and avoids recursion with a guard that, while modifying that tree, reserves its pages at the end of the file. A freed page can be reused in the next transaction: the wait for readers is in the transfer to the data file, not in the manager. |
| Other engines (reference) | LMDB stores free pages in a B+ tree indexed by transaction, reuses them when no reader can see them and looks for contiguous ranges by walking a sorted list. SQL Server uses map pages at fixed positions (GAM, SGAM, PFS) by blocks of 8 pages. redb and ext4 use bitmaps per region with an auxiliary in-memory index. |

## Actors

- **Integrating developer**: queries in the store information how many free pages it has. Does not reserve or free
  pages by themselves.
- **Maintainer**: runs the measurement of F13 on Windows and on Linux.

The following modules (B+ trees, tables, streams and the rest) are the ones that reserve and free pages through this
module.

## Flow

1. A write transaction frees pages it no longer uses (F8). For its own reservations they remain occupied (F6).
2. The transaction reserves a page or a range: the manager looks first among the free pages below the frontier,
   reading on demand the regions it needs (F4, F5, F9), and if it finds no room it advances the frontier, which
   makes the file grow if needed (F3).
3. The commit carries through the journal, with the rest of its pages, the changes of the map and of the frontier
   (F2, F3).
4. From its point of no return, the pages it freed are available for the following writes; the open reads keep
   seeing their previous content (F6).
5. When the commit is durable, the store information reflects the new number of free pages (F10).

## Validations and messages

Those of the ancestors apply, including the failures of T11 and T12 in any reservation or freeing. This module adds
these situations, library only (the tool does not open stores until module 12). All follow T11 without exceptions,
also misuse: the operation has no effect, the transaction can no longer be committed, committing it fails with the
class of the first failure without applying anything, and the store keeps accepting writes.

| Situation | Rule | Error class |
|---|---|---|
| A reservation or a freeing reads a region whose map page is damaged | The operation fails with the number of that page (F9). | Damage |
| The growth of a reservation does not fit in the maximum size or on the disk | The reservation or, at the latest, the commit fails (F5). | Maximum size or disk full |
| A range of 0 pages or of more pages than the cap is requested | The operation fails without effect (F5). | Misuse |
| A store-owned page, one from the frontier on (which was never in use), one that does not exist in the store or one already freed in the same transaction is freed | The operation fails without effect (F8). | Misuse |
| A page that the map has as free is freed | The operation fails without effect (F8). It is only recognized by reading its region, so with a damaged map page the damage wins. | Misuse |
| A page that the same transaction freed is read or modified, or a store-owned page is modified | The operation fails without effect (F8). | Misuse |

When an operation meets several situations at once, the misuse that is recognized without reading the map wins
(all those in the table except freeing a page that the map has as free); then, the first failure the operation
finds, when reading the map (damage, out of memory, input/output) or when growing (maximum size, disk full).

## Output

The module writes nothing to the screen. The store information (P12) adds one datum and keeps the others unchanged;
the format version it reports becomes `5` (F11). Example, for a newly created store on disk without maximum size:

| Datum | Value |
|---|---|
| Format version | `5` |
| Page size | `8192` |
| Store size in bytes | multiple of `8192`, at most `1048576` |
| Number of pages | the size divided by `8192` |
| Maximum size | none |
| Temporary | no |
| Free pages | the number of pages minus the store-owned pages (header, metadata and map of the first region) |

On disk, the store folder has the same names as before (J2); the map is inside `krummi.data`.

## Reuse

- The copy of the modified pages and their keeping in `krummi.temp` while some read needs the previous version (T14,
  J5, J7): it is what makes immediate reuse safe (F6), without duplicating that logic in the manager.
- The journal, the point of no return, the prefix rule and recovery (E2, E8, J8, J10): the map and the frontier
  travel in the commits like any other page.
- The integrity check of the pages and the damage class (G6).
- The growth of the file, the maximum size and out of memory (P10, P11, P13).
- The current counter of reserved pages, which becomes the frontier (F3).
- The simulation of crashes, power cuts and failures of the tests of the Journal and of group writes.

## Out of scope

- Reserving or freeing pages from the public API (F1).
- Reducing the size of `krummi.data`, also when the free pages are at the end (P11; compaction, module 11), and
  returning to the system the memory of a temporary store whose pages are free.
- Moving pages to join gaps, defragmenting or splitting a value into several ranges (F5).
- Ranges larger than the cap, that cross from one region to another or aligned to a given multiple (F5); the maximum
  size of a value is set by module 5.
- Reading a range as a continuous block in memory, also in temporary mode, where memory grows in separate chunks:
  module 5 sets it with its large values.
- Waiting for the oldest reader to reuse a page, and reusing it inside the transaction that frees it (F6).
- Detecting that a transaction reads or modifies a free page that it has neither reserved nor freed itself: it is an
  error of the structure that does it, which verification (module 12) can find.
- Fragmentation data in the store information: the longest free range, percentages or the state of each region
  (storage report, module 12).
- Checking that the total of free pages matches the map, or that the map matches the pages used by the structures:
  verification does it (module 12).
- Repairing or rebuilding a damaged map page (data rescue, module 12).
- Erasing the content of freed pages (secure erase, out of scope of the parent).
- Choosing where a page is reserved so that it is near others of the same structure.
- Encrypting the map: covered by the encryption of everything the engine writes (module 9).
- Performance thresholds (module 6).

## Left to the implementation

- The exact size of a region and, with it, the exact value of the cap, as long as the cap is at least 65,000 pages;
  the layout of the map inside its page.
- Which free range or page is chosen when several serve (the first, the tightest or another), and in what order the
  regions are read, as long as F4 and F5 hold. Therefore, with a damaged map page and free pages in other regions, a
  reservation may find room without reading the damaged region or fail with damage when reading it (F9).
- How the frontier and the total of free pages are saved, and how the state of the regions already read is
  summarized in memory, as long as that summary distinguishes the regions read from the unknown ones, never treats
  an unknown region as full and can be discarded and rebuilt from the map.
- The names of the internal operations and of the new datum of the information, which follow Rust conventions and
  are in English.
- The command that runs the measurement of F13 and the shape of its report.
- How the tests damage map pages, know which regions were read and simulate the lack of space only when growing
  `krummi.data`.

## Acceptance criteria

The test conditions of the Journal and of group writes apply: the criteria that reserve, free, read or write pages
are internal tests of the library; those that need to know which regions were read, hold commits in the journal
queue or simulate failures use the knowledge of the implementation itself. The tests check which set of pages a
reservation receives only when F4 and F5 determine it. The criteria that need several regions may use a reduced
region size only for the tests; the last part of AC-F9 and the measurement of F13 use the product size.

The criteria of the root parent, of Transactions, of the Journal and of group writes still apply, and AC10 of the
root parent is repeated with a small store that has freed pages and a map page. In addition:

1. **AC-F1** — Given a store whose only free pages below the frontier are 10 pages freed by a durable commit, when
   a later write reserves 10 pages one at a time, then it receives exactly those 10, `krummi.data` does not change
   size, the frontier does not advance and, after committing, the free pages of the information go down by 10; and
   the next reservation advances the frontier (F3, F4, F10).
2. **AC-F2** — Given a store without free pages below the frontier, when a write frees a page and then reserves
   another, then the reserved one is not the freed one; and after committing, a later write that reserves a page
   receives the freed one (F4, F6).
3. **AC-F3** — Given an open read that sees the content of a page, when a write frees it and commits, so that it is
   the only free one below the frontier, and another write reserves a page, writes new content and commits, then the
   reserved one is that same number, the open read keeps seeing the previous content, a new read sees the new one,
   and after the old read ends, closing and reopening, the new one is there; and the same result is obtained with
   the two commits in deferred mode, with the new read opened after they are durable. And repeating three times on
   the same page number the freeing and the reuse with a different content, with a read opened before each freeing
   and all open until the end, each read sees the content of its generation (F6, T14, E3).
4. **AC-F4** — Given a write that frees pages and reserves others among the free ones below the frontier, when it is
   aborted, or ceases to be used without committing, or its commit fails by disk full, then none of its freeings or
   reservations is applied: the pages it freed remain in use, the ones it reserved remain free, the frontier does
   not advance and the free pages of the information do not change (F6, T10, T11).
5. **AC-F5** — Given a commit in durable mode that frees a page, held in the journal queue, and a write that reuses
   that page and commits in durable mode afterwards, when the write of the group of the first fails, then both fail
   with the input/output class; and on reopening without the failure, the store has a prefix of the two and never
   the second without the first (F6, E8).
6. **AC-F6** — Given a sequence of commits, in both modes, that free pages and reuse them with new content, in which
   the same page is freed and reused at least three times, when a power cut is simulated at each point where the
   journal or `krummi.data` is written or flushed to disk, and the process is killed at each point of recovery,
   then on reopen the store has a prefix of the sequence, each reused page has the content of the last present
   commit that wrote it, no page is at once free in the map and in use according to the present commits,
   `krummi.data` reaches at least the frontier, and the first part of the free pages of the information matches
   the free ones of the map (F2, F3, F6, F10, G5).
7. **AC-F7** — Given a store with a free range of 20 consecutive pages below the frontier, when a write reserves 16
   contiguous pages, then it receives them inside that range without the frontier advancing; and given a store
   with 100 loose free pages, no free range of 16 and a frontier with more than 16 pages left in its region, the
   reservation of 16 contiguous pages receives the 16 pages that start at the frontier, which advances 16 (F5).
8. **AC-F8** — Given a frontier with 10 pages left in its region and no free page below it, when a write reserves 16
   contiguous pages, then the range starts at the first page after the map page of the next region, the free pages
   of the information, after committing durably, include the 10 skipped, and the next reservation of one page
   receives one of them (F3, F5, F10).
9. **AC-F9** — Given a write, when it requests a range of 0 pages or of one page more than the cap, frees the same
   page twice, frees a page that the map has as free, one of the header, of metadata or of the map, or one beyond
   the end of the store, reads or modifies a page that it itself freed, or modifies a map page, then each operation
   fails with the misuse class without effect; and in each case, in a write that before the incorrect operation
   modified a page, reserved another and freed a third, committing it fails with the misuse class, none of those
   changes appears in a new read or on reopening, and the next write commits without error. In a store without
   maximum size, a range the size of the cap is reserved without error in a region that is not the first (F5, F8,
   T11).
10. **AC-F10** — Given a write that modifies a page, reserves a range of 8 pages, frees 3 of them and the modified
    page, and frees a page that it itself reserved loose, when it commits and reopens, then the 5 remaining pages of
    the range remain in use and start at zero, the 3 of the range, the loose one and the modified one are free, and
    the modified page keeps in `krummi.data` its previous content; and if before that write there were no free
    pages below the frontier, a later write that reserves 5 pages one at a time receives exactly those 5 (F4, F8).
11. **AC-F11** — Given a store with a maximum size that leaves room for the versions of a write in `krummi.temp` but
    not for `krummi.data` to grow, and with free pages below the frontier and none between the frontier and the end
    of the file, when a write reserves one at a time as many pages as there are free in the map, then it commits
    without error; when it reserves one page more, it fails with the maximum size class, applies nothing and the
    store keeps accepting writes; and with a lack of space simulated only when growing `krummi.data`, a reservation
    that needs to grow fails with the disk full class and one that reuses free pages commits without error (F5,
    P10, G7).
12. **AC-F12** — Given a read open during 1,000 commits that free and reserve the same number of pages, when
    `krummi.data` is observed, then its size does not change from the second one on (F7).
13. **AC-F13** — Given a store closed without error whose only free pages are in a region far from the frontier,
    when it is opened, then the opening reads no map page; and when a write reserves a page, it receives one of
    those free pages and the frontier does not advance (F4, F9).
14. **AC-F14** — Given a closed store whose only free pages below the frontier are in a region whose map page has a
    byte altered, when it is opened, then it opens without error, the information gives the same free pages as
    before altering it, all the pages in use except that map page are read without error, a write that only
    modifies pages in use commits without error, and one that reserves a page below the frontier fails with the
    damage class and the number of that map page, cannot be committed, and a later reservation fails again in the
    same way (F9, F10).
15. **AC-F15** — Given a new store, when the information is queried, then the free pages are the number of pages
    minus the store-owned pages; and given a store with pages in use that a previous durable commit reserved, when
    an open write frees them, the free pages do not change; when a deferred commit frees them, they do not change
    until it is durable, and then they go up by that number; and after closing and reopening they give the same
    value (F10, E3).
16. **AC-F16** — Given a random sequence, reproducible by seed, of at least 100,000 operations on a store of up to 4
    regions, with reservations of loose pages and of ranges of up to 64 pages, freeings, commits in both modes and
    aborts, with reads opened and closed at random points and a simulated crash and reopening at others, when
    compared with a reference model that knows the pages in use and the real size of `krummi.data`, then a page in
    use is never reserved, no range includes a store-owned page, each read sees the content of its snapshot, and
    the free pages of the information and of the map match those of the model (F2, F4, F5, F6, F10).
17. **AC-F17** — Given a valid store whose header carries format `4`, when it is opened, then it fails with the
    different-format class as an earlier format and is left identical byte for byte; and the information of a new
    store gives format `5` (F11).
18. **AC-F18** — Given a store in temporary mode, when AC-F1, AC-F2, AC-F3 without the deferred part or the reopen
    part, AC-F4 without the disk full part, AC-F7, AC-F8, AC-F9 without its last part, AC-F10 without reopening,
    AC-F11 without the disk full part, AC-F12 and AC-F15 without the deferred part or the reopen part are repeated,
    then they give the same result, with the store memory instead of `krummi.data` and `krummi.temp` (F12).
19. **AC-F19** — Given a store closed without error, trimmed while the store is closed so that the file ends before
    the frontier and the map page of a region with free pages is missing, when it is opened, then the second part
    of the free pages of the information is zero, and a reservation that needs that region fails with the damage
    class and the number of that map page (F3, F10).
20. **AC-F20** — Given the closing report, when it is reviewed, then it contains the times of the measurement of F13
    on Windows and on Linux (F13).

## Annotations to other PRDs

This PRD changes or refines rules of earlier documents, which carry the corresponding annotations:

- Root parent, split into child PRDs, module 4: a long-running read does not prevent reusing pages; what it retains
  are versions in `krummi.temp` (F7).
- Root parent, D7 (format annotation): the current format version becomes `5` (F11).
- Pager, P5; group writes, E15: the current format version becomes `5`; a store of format `4` is of an earlier
  format (F11).
- Pager, P12; Transactions, T14 (last sentence); Journal, J15: the store information adds the number of free
  pages, which combines the durable part of the map with the size of `krummi.data` as it is (F10); the rest of the
  information keeps describing only `krummi.data`.
- Pager, row for missing pages because the file was trimmed while the store was closed: it also holds for a map
  page (F3).
- Transactions, T18: the errors of this module carry their data (F14).
- Transactions, T14: the "until module 4" ends: the pages that a write frees are reused according to F6 and F7.
- Transactions, T8 and T10: reserving and freeing are changes (F8).
- Journal, J8: when recovery ends, `krummi.data` reaches at least the frontier (F3).

## References

- Parent: [krummi.prd.en.md](krummi.prd.en.md) v1.1 (root)
- Related: [pager.prd.en.md](pager.prd.en.md), [transactions.prd.en.md](transactions.prd.en.md),
  [journal.prd.en.md](journal.prd.en.md), [journal-group-commit.prd.en.md](journal-group-commit.prd.en.md)
- Voron, free space management: <https://github.com/ravendb/ravendb/blob/v6.0/src/Voron/Impl/FreeSpace/FreeSpaceHandling.cs>
- Voron, page reservation: <https://github.com/ravendb/ravendb/blob/v6.0/src/Voron/Impl/LowLevelTransaction.cs>
- LMDB: <https://github.com/LMDB/lmdb/blob/mdb.master/libraries/liblmdb/mdb.c>
- SQL Server, pages and extents: <https://learn.microsoft.com/en-us/sql/relational-databases/pages-and-extents-architecture-guide>
- redb, design: <https://github.com/cberner/redb/blob/master/docs/design.md>

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| 73d9cfa6 | 2026-10-24 07:32 UTC+01:00 | 46 min | 0 min | 46 min | 24 min | 23 min | 8 |

<!-- prd-time: [{"session": "73d9cfa6-d168-4b93-b59e-ff2b19ef0a0c", "start": "2026-10-24T06:32:42.026000+00:00", "wall": 2782, "discarded": 0, "agent": 1431, "human": 1350, "interventions": 8, "idle_minutes": 10}] -->
