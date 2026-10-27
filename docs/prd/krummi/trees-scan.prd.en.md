# Krummi In-Order Scan and Prefix Search — PRD

**Version**: 1.0
**Date**: 2026-10-26
**Parent document**: [Krummi B+ Trees — PRD](trees.prd.en.md) (v1.0)

## Objective and context

Build child PRD **5.2** of the [split of the trees module](trees.prd.en.md#split-into-child-prds): scan in order the
entries of a normal tree between two bounds, forward and backward, limit the scan to the keys that start with a
prefix, and remove at once all the entries of an interval.

When this change closes, an application can read a tree by intervals and not only key by key: list all its entries,
those of an interval of keys or those of a prefix, in ascending or descending order, and empty an interval without
removing the keys one by one. The scans follow the semantics of `BTreeMap::range` of the Rust standard library, as
5.1 followed those of `FileMode` and `Dictionary` of .NET, with the difference of R3. The parent's split speaks of
scanning "from a given key"; the two bounds of this PRD generalize it: that key is one of the bounds and the other
can be left open.

**Why now**: it is the second submodule of the split; the indexes of the tables (module 7) and the values of a
multi-value key (5.4) are read by intervals and by prefix.

This PRD inherits everything from its ancestry, the [trees module](trees.prd.en.md) (B1–B12) and the
[root](krummi.prd.en.md), and what the parent inherits from the earlier modules: the [Pager](pager.prd.en.md)
(P1–P17), the [Transactions](transactions.prd.en.md) (T1–T18), the [Journal](journal.prd.en.md) (J1–J17), the
[grouped and direct journal writing](journal-group-commit.prd.en.md) (E1–E16) and the
[Free space](free-space.prd.en.md) (F1–F14). It also reuses what its sibling
[Trees and Key Operations](trees-keys.prd.en.md) (A1–A15 and its annotation) set. Only what is specific to 5.2 is
added here. What changes or refines earlier rules is declared in its decisions and annotated in the affected
documents (see [Annotations to other PRDs](#annotations-to-other-prds)).

In this document, an **interval** is the set of keys between a lower bound and an upper bound; each **bound** is an
included key, an excluded key or **open** (no bound on that side). A **scan** is what results from requesting the
entries of an interval, with or without a prefix, of a tree, and from which entries are taken from either of its two
ends. A **handle** is the one of 5.1.

### Product decisions

| ID | Decision |
|---|---|
| R1 | **Scan** (like `BTreeMap::range`): it is requested through a handle of a tree, with a lower bound and an upper bound. It returns the entries of the interval, each as its key and its value, in the key order of B5. It is consumed from both ends: the **front end** takes the next entry in ascending order and the **back end** the next one in descending order, in any alternation; each entry comes out only once, and the scan ends when the two ends meet. It is in both kinds of transaction. |
| R2 | **Where it starts**: the bound keys need not exist in the tree. The front end starts at the first key greater than or equal to the lower bound if it is included, at the first greater key if it is excluded, or at the first of the tree if it is open. The back end starts at the last key less than or equal to the upper bound if it is included, at the last lesser key if it is excluded, or at the last of the tree if it is open. |
| R3 | **Empty interval** (difference from `BTreeMap::range`, which halts the program when the lower bound comes after the upper one, or when both have the same key and both are excluded): if the lower bound comes after the upper one, or both have the same key and either is excluded, or no key that starts with the prefix (R4) can fall between the bounds (for example, the prefix `a` with the lower bound `c`), the interval is empty and it is not an error: the scan returns no entries and Remove range (R7) removes `0`. Since it is decided from the arguments alone, it reads no page of the tree and cannot fail by damage. An interval that is not empty by this rule but has no keys in the tree does read pages (R8). |
| R4 | **Prefix**: a scan can also request a prefix, and then it returns only the keys of the interval that start with those bytes, in both directions. A key equal to the prefix starts with it. The empty prefix limits nothing. With a prefix that ends in `0xFF` bytes, all its keys are returned; the application does not have to compute the key that follows the prefix. |
| R5 | **Sizes** (refines B5, B7 and B8): each bound key is validated as a key (B5), from 1 to 2,048 bytes, and outside that size it fails with invalid size as a key, with minimum `1` and maximum `2048`; an open bound has no key and is not validated. The prefix is from 0 to 2,048 bytes; outside that size it fails with invalid size, whose datum for what violates its size is the new value **prefix**, with minimum `0` and maximum `2048`. It is always validated, even if the interval is empty (R3) or the tree has no entries. If several arguments are outside their size, the first one in this order is reported: lower bound, upper bound, prefix. |
| R6 | **Scans and changes** (B9, T8): a scan sees what its transaction sees at the moment it is requested: in a read, its snapshot (G3); in a write, with all its changes made up to that moment. A scan of a write stops being valid at the next operation that can modify the transaction (A10), in that tree or in another, even if it fails without effect: continuing to use it afterwards does not compile. Requesting a scan and taking entries from it does not modify: it does not end the references or other scans. The keys and values it returns are zero-copy references with the lifetime of those of Read (D11, T8): in a read they are valid while the transaction is open, also after the scan is dropped; in a write, until the next operation that can modify it. |
| R7 | **Remove range**: in a write, it removes from a tree all the entries of an interval, with or without a prefix, with the rules of R2 to R5, and returns how many it removed. The number of entries of the tree (A5) goes down by that amount. The tree keeps its name, its type and its identity (A8), and its other handles in the transaction stay valid and see the tree without the removed entries. The pages it stops using return to the free space (F8), and a tree left with no entries, by one or by several Remove range, takes up the same as an emptied one (A2). It ends the earlier references and scans even if it removes nothing or fails (A10). For T10 it is a change only if it removes at least one entry; unlike Empty, removing everything from an already empty tree is not a change. It is not in read transactions: using it in one does not compile. |
| R8 | **Errors**: there are no new classes. **Precedence** (extends A7): first invalid size, in the order of R5; then a handle of another transaction (misuse) and a handle of a tree that the same write deleted (tree not found, with the tree's last name, A6), both without reading any page; then, the failures reading the pages of the tree. A scan fails with the damage class and the page number when it needs a damaged page, like Read (A9). It needs the pages that contain keys of the interval, those that lead from the root to where the interval starts and where it ends, also when it has no keys, and the neighbouring ones of those that it reads to know that the interval ended. Remove range needs those same pages and also the neighbouring ones that it reads to reorganize the tree. A page that is none of those makes neither a scan nor Remove range fail. The other failures of T11 also apply: a scan can fail with out of memory or with input and output, and Remove range also with disk full or maximum size. In a write, all follow T11. **After a failure**, the scan ends: both ends stop returning entries, without repeating the error; the entries it returned before are correct. In a read, the transaction stays usable and a new scan works, although it fails again the same way if it needs the same damaged page (F9). |
| R9 | **Threads**: a scan follows the rule of its handle (B1): one of a read can be moved to another thread, and moving one of a write to another thread does not compile. |
| R10 | **Format**: the change does not modify what is stored on disk (B12): the current format version remains `6`, the stores created with 5.1 are opened and scanned without conversion, and the pages that Remove range leaves pass the same consistency checks as those left by the operations of 5.1, so that a 5.1 engine reads and modifies them without risk. |
| R11 | **Measurement** (B12): the change includes a reproducible measurement, on Windows and on Linux, on a store on disk with durable commit, on the randomly loaded store of A15 of 5.1 (1 million random keys of 16 bytes with their fixed seed and values of 100 bytes), after closing and reopening it. It measures, in a single read transaction and reading the bytes of each key and each value returned: scanning all the entries forward and then backward; and 10,000 forward scans of 100 entries from random bound keys. Then, removing all the entries with 1,000 contiguous intervals that divide the keys into random slices, the first with the lower bound open and the last with the upper bound open, one per write (A15 used 1,000 operations per write; here each write is a single Remove range). Each part of the measurement starts from a copy of the loaded store. The report includes the operating system, the processor and the disk. Its results go in the closing report. |

*Annotations of 2026-10-27 from large values ([trees-large-values.prd.en.md](trees-large-values.prd.en.md)):*

- *R8: damage in the pages of a value stored off its leaf makes a scan that needs its entry fail, but never Remove range, which does not read those pages (decision V7).*
- *R10, Output and AC-R14: the current format version becomes `7`; a store of format `6`, also the one created with 5.1 and kept as test data, is of an earlier format (decision V9).*
- *Out of scope: values of more than 1,024 bytes in scans and in Remove range arrive with large values (decisions V1 and V2).*

### Starting state

Verified on 2026-10-26 in the code:

| Aspect | Situation |
|---|---|
| Public API | A normal tree is opened, emptied, renamed, listed and deleted; its entries are stored, added, read and removed one by one by their key, and its number of entries is queried (A1–A5). There is no public way to scan a tree or to search by interval or by prefix. |
| Internal scan | The library only scans internally a whole tree, from start to end: the catalog, which is also a tree, for List; a tree that is deleted or emptied, to free its pages; and the trees, in the tests. It does not scan part of a tree or in descending direction. |
| Errors | Sixteen classes: the twelve of the parent's starting state plus invalid size, different tree type, tree not found and tree already exists. Invalid size reports name, key or value as what violates its size. A handle of another transaction is checked before that of a deleted tree. |
| Format | The current format version is `6` (A13). |

## Actors

- **Integrating developer**: scans trees by intervals and by prefix in both directions, removes intervals and handles
  the errors by their class.
- **Maintainer**: runs the measurement of R11 on Windows and on Linux.

## Flow

1. The application opens a transaction and a normal tree (A1).
2. It requests a scan with two bounds and, if it wants, a prefix (R1–R5).
3. It takes entries from the front end, from the back end or from both, until the scan ends or it drops it.
4. In a write, it removes intervals (R7); a modification ends the open scans (R6).
5. The commit applies all the changes, or none (G1).

## Validations and messages

Those of the ancestors and of 5.1 apply, including the failures of T11 and T12. This PRD adds these situations, in
the library only (the tool does not open stores until module 12); those of invalid size, tree not found and misuse
by a handle of another transaction are of public validation (B7). The precedence is that of R8.

| Situation | Rule | Error class |
|---|---|---|
| A bound key that is empty or of more than 2,048 bytes, when scanning or removing an interval | R5 | Invalid size, as a key |
| A prefix of more than 2,048 bytes, when scanning or removing an interval | R5 | Invalid size, as a prefix |
| Inverted bounds, or equal with either excluded | R3 | None: empty interval |
| An interval is scanned or removed through a handle of another transaction | R8 | Misuse |
| An interval is scanned or removed through a handle of a tree that the same write deleted | R8, A6 | Tree not found |
| A scan or Remove range needs a damaged page of the tree | R8, A9 | Damage, with the page number |

## Output

The module writes nothing to the screen. The store information (P12) keeps its data and the format version remains
`6` (R10). A scan returns pairs of key and value; Remove range, a number. Example, in a tree with the keys `a`, `ab`,
`b`, `ba` and `c`:

| Request | End | Keys returned |
|---|---|---|
| From `ab` included to `b` included | front | `ab`, `b` |
| From `ab` included to `b` included | back | `b`, `ab` |
| From `a` excluded to `ba` included | front | `ab`, `b`, `ba` |
| From `aa` excluded to open | front | `ab`, `b`, `ba`, `c` |
| Open to `b` excluded | back | `ab`, `a` |
| Prefix `b`, open bounds | front | `b`, `ba` |
| Prefix `b`, open bounds | back | `ba`, `b` |
| Prefix `a`, from `aa` included to open | front | `ab` |
| Empty prefix, open bounds | front | `a`, `ab`, `b`, `ba`, `c` |
| From `c` included to `a` included | either | none |
| Prefix `d`, open bounds | either | none |

Taking alternately from the two ends of a scan of the whole tree, starting with the front end, `a`, `c`, `ab`, `ba`
and `b` come out, and then no more from either end. Removing the interval with prefix `b` and open bounds in that
tree returns `2`; removing from `ab` included to `ba` excluded with prefix `b` returns `1` (only `b`).

Example of the data of an invalid size error, for a prefix of 2,049 bytes in a store on disk:

| Datum | Value |
|---|---|
| Folder | the store's |
| What violates its size | prefix |
| Size received | `2049` |
| Minimum | `0` |
| Maximum | `2048` |

## Reuse

- The common rules of the trees module (B1–B12) and those of 5.1: handles, errors, precedence, damage, identity and
  changes (A5–A10).
- The key order of B5, the same that Read and List already use.
- The transactions, their snapshot and the zero-copy reads (T2, T8, T16, D11).
- The free space (F4–F8) for the pages that Remove range frees, and the page freeing of Empty (A2).
- The simulation of crashes, power cuts and failures of the tests of modules 3 to 5.1.

## Out of scope

- Cursors that are repositioned or change direction without requesting another scan (the Voron and LMDB model), and
  standalone searches for the key before or after a given one, or for the first or the last (a scan with a single
  step from one end gives them).
- Scans that remain valid after the transaction is modified, removing or storing entries through a scan, and
  removing an interval while returning its entries.
- Counting the entries of an interval without scanning it.
- Scanning or removing several intervals in a single operation, or intervals of several trees at once.
- Listing the trees of the catalog by interval or by name prefix.
- Values of more than 1,024 bytes in scans and in Remove range: they arrive with large values (5.3), which extends
  these operations.
- Scans of multi-value, fixed-size and compressed trees (5.4, 5.5 and 5.6, which set their own scans).
- Copying an interval to another tree.

## Left to the implementation

- How an interval is scanned and removed internally, and the organization of the pages, as long as the format does
  not change (R10).
- At which step a failure appears: when requesting the scan, when taking the first entry of the page that fails or
  before, when taking the last ones of the previous page.
- Which damaged page is reported when an operation of R8 finds several, and which neighbouring pages a scan reads to
  know that the interval ended and Remove range to reorganize the tree (R8).
- How many pages a Remove range that leaves entries in the tree frees, as long as R7 is met when it leaves it empty.
- What the write itself sees of a tree whose Remove range failed with a T11 failure (the write can no longer
  commit), as in 5.1.
- The names of the operations and of the representation of the bounds, which follow Rust conventions and are in
  English.
- The command that runs the measurement of R11 and the shape of its report.

## Acceptance criteria

The test conditions of the earlier modules and the header of the parent's criteria apply: each criterion runs on a
store on disk and in temporary mode, except the parts that reopen the store or damage its files, which are on disk
only. This PRD checks the parent's criterion assigned to 5.2 (AC-B7 in its 5.2 part), and AC-B17 and AC-B18 with
Remove range, which changes the structure of a tree (AC-R7 and AC-R12); it repeats AC10 and AC20 of the root with
the scans, and adds these. The tests that cause splits, merges, damaged pages or failures at specific points use
knowledge of the implementation itself.

1. **AC-R1** — Given the tree of the example in *Output*, when the scans of its table are requested, then each
   returns exactly the keys indicated, in that order, with their values; taking alternately from the two ends of a
   scan of the whole tree, starting with the front end, `a`, `c`, `ab`, `ba` and `b` come out, and then no more from
   either end; and the two Remove range of the example, each on the original tree, return `2` and `1` and remove
   those keys (R1–R4, R7).
2. **AC-R2** — Given a tree with enough entries to have several levels, when it is scanned whole forward, backward
   and alternating ends at random, then all its keys come out, in order, in reverse order and once each, and their
   number matches the number of entries (A5); and when scans are requested whose bounds are an existing key, a key
   between two neighbouring keys (also where they change page) or a key before the first or after the last, each
   included and excluded, then each end starts where R2 says (R1, R2).
3. **AC-R3** — Given keys with bytes from `0x00` to `0xFF`, including `FF`, `FF FF`, `FF FF 00` and `FF FF FF`, when
   the prefix `FF FF` is scanned, then `FF FF`, `FF FF 00` and `FF FF FF` come out forward, and in reverse order
   backward; with the empty prefix, all the keys; with a prefix of 2,048 bytes equal to a key, that key; and with a
   prefix that has no key, none (R4).
4. **AC-R4** — Given inverted bounds, bounds with the same key and either excluded, and the prefix `a` with the lower
   bound `c`, when the interval is scanned and removed, then no entry comes out, Remove range returns `0` and they
   do not fail, also with the root of the tree damaged; through a handle of a deleted tree, they fail the same way
   with tree not found; with the same key included in both bounds, that entry comes out if it exists, only once;
   given a write on disk that only requests scans, removes empty intervals or intervals without keys in the tree and
   makes operations that fail by invalid size, when it is committed, then it ends without error and the store's
   files are left identical byte for byte; and a write on disk that removes a single entry with Remove range does
   write to the store (R3, R7, T10).
5. **AC-R5** — Given a read transaction and a write one, when they scan with a lower or upper bound that is empty or
   of 2,049 bytes, and the write also removes an interval with them, then it fails with invalid size without effect,
   as a key, with the size received, minimum `1` and maximum `2048`; with a prefix of 2,049 bytes, as a prefix, with
   minimum `0` and maximum `2048`; with bounds of 1 and of 2,048 bytes and prefixes of 0 and of 2,048 bytes, it does
   not fail; with both bounds outside their size, the error is the lower one's, and with the upper one and the
   prefix, the upper one's; with a bound outside its size in a tree with no entries or in an inverted interval, it
   fails the same way; and the transaction stays usable (R5, B7).
6. **AC-R6** — Given a write that stores and removes entries and then requests a scan, when it consumes it, then it
   sees those changes; and given a write with a tree opened twice, when it removes an interval through one handle,
   then the other sees the tree without the removed entries and the new number of entries (R6, R7).
7. **AC-R7** — Given a read transaction with a scan half consumed and another not started, when a write removes
   intervals and stores entries until the height of the tree changes and commits, then the two scans return exactly
   the entries of their snapshot, and a new read sees those of the write (R6, G3, AC-B17).
8. **AC-R8** — Given the code of a test application with a scan of a write, when it uses it after storing, adding,
   removing, removing a range, opening a tree in a mode other than Open, deleting or renaming a tree, even if they
   fail or change nothing, then it does not compile; when it uses it after Read, after opening a tree in Open mode,
   after Exists, after List, after the number of entries or after requesting another scan, then it compiles; a key
   returned by a scan of a read is used after dropping the scan, and one of a write is used after dropping it but
   not after the next operation that can modify; Remove range in a read transaction does not compile; and moving a
   scan of a read to another thread compiles, and one of a write does not (R6, R7, R9, AC-B11).
9. **AC-R9** — Given a tree with several levels, when intervals are removed that fall within a page, that cross
   several pages and levels, with and without a prefix, then each returns how many entries it removed, only those
   keys disappear, the number of entries goes down by that amount and a scan of the whole tree returns the others;
   and after committing and reopening, it is kept (R7, A5).
10. **AC-R10** — Given a store with a tree already created, which stays within a single region of the free space,
    taking the pages in use as in AC-A15, when an empty tree is created, committed, the value is taken, it is filled
    with enough entries to take up several pages and several levels, committed, all its entries are removed with an
    interval with open bounds and committed, then the final value is the one taken with the empty tree; and the same
    when they are removed with ten contiguous intervals, in different writes (R7, A2, F10).
11. **AC-R11** — Given a write with a tree opened twice, when it deletes it through one handle and then requests a
    scan or removes an interval with the other, then it fails with tree not found, with the tree's last name,
    without effect; given a handle of an already ended transaction, when it is used in another to scan or remove an
    interval, then it fails with misuse without effect; in each of those two cases, with a bound of 2,049 bytes as
    well, invalid size wins, and with the root of the tree damaged as well, its class (misuse or tree not found) wins
    over damage; and in all cases the transaction stays usable (R8).
12. **AC-R12** — Given a write that removes intervals with page merges that change the height of the tree, when a
    process crash and a simulated power cut are caused at every point where its commit writes or carries data to
    disk, then on reopening the tree has all its entries from before or all those from after and the number of
    entries matches; and when a T11 failure is simulated at every point where Remove range reserves, reads or frees a
    page, then committing fails with the class of the first failure and applies nothing, and after aborting the tree
    and its number of entries are as before (R7, G1, G5, AC-B18).
13. **AC-R13** — Given a tree with a damaged leaf page, and another with a damaged intermediate page, when a read
    scans them from both ends until it needs that page, then it fails with the damage class and the page number, the
    entries returned before are the correct ones, afterwards it returns no more entries from either end, the
    transaction stays usable and a new scan that needs it fails the same way; a scan and a Remove range whose keys
    are in another part of the tree, neither in that page nor in its neighbours nor on the path to them, do not fail;
    when a write scans or removes an interval that needs it and fails with damage, then committing it fails with the
    damage class and applies nothing; and when an out of memory or input and output failure is simulated in the
    middle of a scan, then it fails with that class and ends the same way (R8, A9, T11).
14. **AC-R14** — Given a store created and filled with the 5.1 version, kept as test data, when it is opened with
    the 5.2 one, then its format version is `6` and it is scanned without error; and when intervals are removed from
    it with merges that change the height of a tree, committed and reopened, then a scan of the whole tree and the
    read of each remaining key do not fail with damage; and when that store is opened with the 5.1 version, then the
    list of trees, each remaining key and the number of entries are read without error, and storing and removing
    entries until causing splits and merges, committing and reopening work without error. The 5.1 version part is a
    check of the execution, with that version compiled separately and its result in the closing report, not a test
    kept in the repository (R10).
15. **AC-R15** — Given the measurement of R11, when the closing report is reviewed, then it contains its times and
    the machine data on Windows and on Linux (R11).

## Annotations to other PRDs

This PRD refines rules of earlier documents, which carry the corresponding annotations:

- Trees module, B5: the bound keys of a scan are validated as keys, from 1 to 2,048 bytes, and the prefix is from 0
  to 2,048 bytes (R5).
- Trees module, B7: the order of the arguments outside their size continues with lower bound, upper bound and prefix
  (R5).
- Trees module, B8: invalid size can report **prefix** as what violates its size, with minimum `0` and maximum
  `2048` (R5).
- Trees and Key Operations, A7: the precedence holds for scans and Remove range, with misuse before tree not found
  (R8).
- Trees and Key Operations, A9: a scan fails with damage in the pages it needs, like Read, and Remove range in those
  and in the neighbouring ones it reads to reorganize the tree; a page they do not need never makes them fail (R8).
- Trees and Key Operations, A10: Remove range ends the references, and is a change for T10 only if it removes some
  entry; requesting a scan and taking entries does not end them (R6, R7).

## References

- Parent: [trees.prd.en.md](trees.prd.en.md) v1.0; root: [krummi.prd.en.md](krummi.prd.en.md) v1.1
- Sibling: [trees-keys.prd.en.md](trees-keys.prd.en.md) v1.0
- Related: [transactions.prd.en.md](transactions.prd.en.md), [free-space.prd.en.md](free-space.prd.en.md)
- Rust, `BTreeMap::range`: <https://doc.rust-lang.org/std/collections/struct.BTreeMap.html#method.range>
- Voron, trees: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron/Data/BTrees>

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| 65ee1db9 | 2026-10-26 06:27 UTC+00:00 | 45 min | 1 min | 44 min | 18 min | 26 min | 9 |

<!-- prd-time: [{"session": "65ee1db9-fa7b-4f9e-a6a0-63629e012fb8", "start": "2026-10-26T06:27:50.067000+00:00", "wall": 2686, "discarded": 42, "agent": 1087, "human": 1556, "interventions": 9, "idle_minutes": 10}] -->
