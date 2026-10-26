# Krummi Trees and Key Operations — PRD

**Version**: 1.0
**Date**: 2026-10-25
**Parent document**: [Krummi B+ Trees — PRD](trees.prd.en.md) (v1.0)

## Objective and context

Build child PRD **5.1** of the [split of the trees module](trees.prd.en.md#split-into-child-prds): the catalog of
named trees and the normal trees, with the key operations to store, add, read and remove entries, and values of up
to 1,024 bytes.

When this change closes, an application can create, open, empty, rename, list and delete trees inside a
transaction, and store and read data in them: it is the first time the product stores application data. The catalog
operations follow the semantics of the .NET file opening modes (`File` and `FileMode`), and the key operations
those of a .NET dictionary (`Dictionary`).

**Why now**: it is the first submodule of the split; the others (traversal, large values, multi-value, fixed-size
and compression) build on the catalog and on the normal trees.

This PRD inherits everything from its ancestry, the [trees module](trees.prd.en.md) (B1–B12) and the
[root](krummi.prd.en.md), and what the parent inherits from the earlier modules: the [Pager](pager.prd.en.md)
(P1–P17), the [Transactions](transactions.prd.en.md) (T1–T18), the [Journal](journal.prd.en.md) (J1–J17), the
[grouped and direct journal writing](journal-group-commit.prd.en.md) (E1–E16) and the
[Free space](free-space.prd.en.md) (F1–F14). Only what is specific to 5.1 is added here. What changes or refines
earlier rules is declared in its decisions and annotated in the affected documents (see
[Annotations to other PRDs](#annotations-to-other-prds)).

In this document, a **handle** is what opening or creating a tree inside a transaction returns, and through which
the tree is operated on (B9). **Delete** is used only for trees and **remove** for entries.

### Product decisions

| ID | Decision |
|---|---|
| A1 | **Opening modes** (like .NET `FileMode`): a tree is opened by its name and its type in one of these modes. **Open**: opens the tree; if it does not exist, it fails with the new **tree not found** class. **Create new**: creates an empty tree; if it already exists, it fails with the new **tree already exists** class. **Open or create**: opens the tree if it exists and, if not, creates it empty. **Create or empty** (`Create`): creates the tree empty if it does not exist and, if it exists, empties it. **Empty** (`Truncate`): empties the tree; if it does not exist, it fails with tree not found. In a read transaction only the Open mode exists; the others do not compile in it. An existing tree of a different type follows B3 in every mode. In 5.1 the only type is the normal one. |
| A2 | **Emptying a tree** (Create or empty and Empty modes): the tree keeps its name, its type and its identity (B9), is left with no entries and its number of entries becomes `0`. The other handles of the same tree in the transaction stay valid and see it empty. Emptying frees its pages (F8): an emptied tree takes up the same as a newly created empty one. It is a change, even if the tree was already empty or was just created in the same transaction. |
| A3 | **Other catalog operations** (like .NET `File`): **Exists** says whether a tree with that name exists; it does not fail because it does not exist. **Delete** deletes the tree with all its entries and frees its pages (F8); if it does not exist, it does nothing and does not fail for that reason, like `File.Delete`. **Rename** changes the name of a tree: if the source does not exist, it fails with tree not found, also when the destination is the same name; if the destination already exists, it fails with tree already exists, unless overwriting is requested, in which case the destination tree is deleted as with Delete, whatever its type (verifiable from 5.4, when there is another type). Renaming an existing tree to its own name does nothing, also with overwriting. The renamed tree keeps its type, its entries and its identity: its handles stay valid under the new name. **List** returns the name and the type of each tree, in the name order of B5, as a list owned by the application: it is a copy that stays valid after the catalog is modified or the transaction ends (an exception to D11 of the root parent and to B10 for the catalog). Exists and List are in both kinds of transaction; Delete and Rename, only in write ones. In a write transaction, Exists and List see its own catalog changes; in a read transaction, its snapshot. |
| A4 | **Key operations** (like .NET `Dictionary`): **Store** creates the entry or overwrites its value if the key is already there, and returns nothing, like the indexer. **Add** creates the entry only if the key is not there and returns whether it added it; if it was already there, it changes nothing, like `TryAdd`. **Read** returns the value of the key or that it is not there, and tells a key with an empty value from a key that is not there, like `TryGetValue`. **Remove** removes the entry and returns whether it was there, like `Remove`. None fails because the key is present or absent, but all validate their sizes first (B7): Add with a value outside its size fails even if the key is already there. Read is in both kinds of transaction; Store, Add and Remove, only in write ones. |
| A5 | **Number of entries** (like `Count`): a handle reports the number of entries of the tree as its transaction sees it (T2), with its own changes in a write transaction, instantly, without traversing the tree. Querying it can only fail with tree not found, through a handle of a deleted tree (A6). |
| A6 | **New errors** (D8 of the root parent): **tree not found** and **tree already exists**, public validation errors with the rule of B7. Tree not found is also the B9 error for an operation through a handle of a tree that the same write transaction deleted, directly or by overwriting it with Rename. Both carry the data of T18 and a tree name: the requested one; in Rename, the source one for not found and the destination one for already exists; and for a handle of a deleted tree, the last name the tree had. |
| A7 | **Precedence** (extends B7): invalid size beats everything and, in Rename, the source name is checked before the destination one. Then, failures reading the catalog (damage, out of memory, input and output), different tree type, tree not found, tree already exists and, last, failures reading the pages of the tree itself. In Rename, a source that does not exist fails with tree not found without looking at the destination. An operation through a handle of a deleted tree fails with tree not found without reading the catalog or the tree's pages, so, after invalid size, it beats everything else. |
| A8 | **Identity of trees** (B9): deleting a tree, directly or by overwriting it with Rename, ends its identity. A tree created afterwards with that name is new, also in the same transaction and however many times it is repeated: no handle ever reaches a tree other than the one it opened. Emptying and renaming keep the identity. If the possible identities were ever exhausted, creating a tree would follow B11. |
| A9 | **Damage in a tree** (G6): the Open, Create new and Open or create modes, Exists, List, Rename without overwriting or to its own name and the number of entries never fail with damage in the pages of the tree itself. Read, Store, Add and Remove fail with damage in a page of the tree that they need, with the damage class and the page number. Delete, emptying (Create or empty and Empty modes) and Rename with overwriting to another name (on the destination tree) can fail with damage in any page of the affected tree. A damaged page fails the same way in every later operation that needs it, as in F9. In a write transaction, every failure by damage follows T11. |
| A10 | **Changes and references** (T8, T10, B9): the earlier references end with every operation that can modify: the modes other than Open, Delete, Rename, Store, Add and Remove, even if in the end they change nothing or fail. Open, Exists, List, Read and the number of entries do not end them. For T10 the following are changes: creating a tree, emptying it, deleting a tree that exists, renaming to another name, Store, and Add or Remove when they add or remove. The following are not: Open or create on an existing tree, Delete of a tree that does not exist, renaming to its own name, an Add that does not add, a Remove that does not remove and every operation that fails with a public validation error. |
| A11 | **Provisional maximum size of a value** (B6): 1,024 bytes. |
| A12 | **On-demand catalog**: the catalog takes up no pages until the first tree is created, so a store with no trees meets unchanged the size and free-page rules of the earlier modules (P11, F10, AC-F15). |
| A13 | **Format `6`**: the change is incompatible (B12): an engine of format `5` does not know the catalog or the trees and does not guarantee to preserve them when writing to the store. The current format version becomes `6`, and a store of format `5` is of an earlier format even if it is empty. |
| A14 | **Prior compilation test**: the one the parent requires before implementing 5.1 applies ([split](trees.prd.en.md#split-into-child-prds)). |
| A15 | **Measurement** (B12): the change includes a reproducible measurement, on Windows and on Linux, on a store on disk with durable commit. With 1 million random keys of 16 bytes, the same for all loads through a fixed seed, and values of 100 bytes, it measures: storing them in key order in one store and, in another, at random, in write transactions of 1,000 entries; in the store loaded at random, after closing and reopening it, reading them all at random in a single read transaction and then removing them all at random in write transactions of 1,000; and the size of the store in bytes that its information gives (P12) after each of the two loads. The report includes the operating system, the processor and the disk. Its results go in the closing report. |

*Annotation of 2026-10-25, before executing this PRD: an operation through a handle of another transaction of the same class, also of one that has ended, fails without effect with the misuse class and the transaction stays usable (exception to B1, annotated in the parent). Its precedence is that of a handle of a deleted tree (A7): after invalid size, it wins over everything else. This criterion checks it: given a handle of a write transaction already committed, when it is used in a new write transaction of the same store to read, store and query the number of entries, and a handle of a read transaction, when it is used in another read transaction to read and query the number of entries, then each operation fails with the misuse class without effect and the new write commits with its other changes. Using a read transaction's handle in a write transaction, or the other way round, does not compile.*

*Annotations of 2026-10-26 from in-order scan and prefix search ([trees-scan.prd.en.md](trees-scan.prd.en.md)):*

- *A7: the precedence also holds for scans and Remove range, with misuse before tree not found (decision R8).*
- *A9: a scan fails with damage in the pages it needs, like Read, and Remove range in those and in the neighbouring
  ones it reads to reorganize the tree; a page they do not need never makes them fail (decision R8).*
- *A10: Remove range ends the references and the scans, and is a change for T10 only if it removes some entry;
  requesting a scan and taking entries from it does not end them (decisions R6 and R7).*

### Starting state

Verified on 2026-10-25 in the code:

| Aspect | Situation |
|---|---|
| Public API | The library opens, queries and closes stores, and opens, commits and aborts transactions. There is no public way to store or read data (T1, F1). |
| Catalog and trees | They do not exist. The metadata page, after the header, holds the frontier and the total of free pages (F3, F10). |
| Free space | A write transaction reserves and frees single pages or ranges (F4–F8). |
| Errors | The twelve classes of the parent's starting state; those of invalid size and different tree type (B7) are not implemented. |
| Format | The current format version is `5` (F11). |

## Actors

- **Integrating developer**: creates, opens, empties, renames, lists and deletes trees; stores, adds, reads and
  removes entries; queries the number of entries; and handles the new errors by their class.
- **Maintainer**: runs the measurement of A15 on Windows and on Linux.

## Flow

1. The application opens a transaction and, inside it, a normal tree in one of the modes of A1.
2. In a write transaction it stores, adds and removes entries (A4); in any transaction it reads them and queries the
   number of entries (A5).
3. It checks whether a tree exists and lists the trees; in a write transaction it also empties, renames and deletes
   them (A2, A3).
4. The commit applies all the changes of the trees and of the catalog, or none (G1).

## Validations and messages

Those of the ancestors apply, including those of B3–B7, and the failures of T11 and T12 in any operation. This PRD
adds these situations, in the library only (the tool does not open stores until module 12), all of public
validation (B7, A6). The precedence is that of A7.

| Situation | Rule | Error class |
|---|---|---|
| A tree that does not exist is opened in Open mode, in any transaction, or in Empty mode, in a write transaction | A1 | Tree not found (new) |
| A tree that already exists with the same type is opened in Create new mode | A1 | Tree already exists (new) |
| A tree that does not exist is renamed, also to its own name | A3 | Tree not found (new) |
| A tree is renamed to a name that already exists, without overwriting | A3 | Tree already exists (new) |
| An operation is made through a handle of a tree that the same write transaction deleted, directly or by overwriting it | A6, B9 | Tree not found (new) |

## Output

The module writes nothing to the screen. The store information (P12) keeps its data (B11); the format version it
reports becomes `6` (A13). List returns pairs of name and type; in a store with no trees, an empty list. Example,
after creating `pedidos`, `Clientes` and `clientes` as normal trees:

| Name | Type |
|---|---|
| `Clientes` | normal |
| `clientes` | normal |
| `pedidos` | normal |

Example of the data of a tree already exists error, when renaming `clientes` to `pedidos` without overwriting in a
store on disk:

| Data | Value |
|---|---|
| Folder | the store's |
| Name | `pedidos` |

## Reuse

- The common rules of the trees module (B1–B12).
- The transactions, the journal and recovery (T2, T14, T16, G1, G5): the changes of the trees and of the catalog
  travel like any other change.
- The free space (F4–F8): the trees reserve and free pages through it.
- The page integrity check and the damage class (G6).
- The simulation of crashes, power cuts and failures of the tests of modules 3 and 4.

## Out of scope

- Traversing a tree in order and searching by range or by prefix (5.2).
- Values of more than 1,024 bytes (5.3).
- Multi-value, fixed-size and compressed trees (5.4, 5.5 and 5.6).
- Copying a tree, or moving entries from one tree to another in a single operation.
- The `Append` mode of `FileMode`, which makes no sense in a sorted tree.
- Having Delete report whether the tree existed (Exists is asked first, as in .NET), and an Exists that asks for a
  type (List gives the type).
- Storing or removing several entries in a single operation, and the combined operations of `Dictionary` such as
  `GetOrAdd` or `AddOrUpdate` (combined operations, out of scope of the parent).

## Left to the implementation

- How the catalog is stored, whether it returns its pages when the last tree is deleted, whether an empty tree
  keeps some page of its own, how the identity of a tree is told apart from its name (A8) and the organization of
  the pages of the trees, as long as the guarantees of the ancestors are met.
- Whether emptying and deleting a tree read its pages or a record of its own of the tree's pages, to know which
  ones to free (A2, A3, A9).
- Which damaged page is reported when an operation of A9 finds several, and what the write transaction itself sees
  of a tree whose operation failed with a T11 failure (the write transaction can no longer commit).
- The names of the operations, of the modes and of the new classes, which follow Rust conventions and are in
  English, and the English text of the errors.
- The shape of the list that List returns, as long as it gives each name with its type in the order of A3.
- The command that runs the measurement of A15 and the shape of its report.

## Acceptance criteria

The test conditions of the earlier modules and the header of the parent's criteria apply: each criterion runs on a
store on disk and in temporary mode, except the parts that reopen the store or damage its files, which are on disk
only. This PRD checks the parent's criteria assigned to 5.1 (AC-B1, AC-B2, AC-B3 with its provisional maximum,
AC-B4 and AC-B6 in their 5.1 parts, AC-B5, AC-B9 to AC-B13, AC-B14 in its same-type part, AC-B17 and AC-B18),
repeats AC10 and AC20 of the root with the trees and adds these. The tests that cause page splits and merges, or
failures at specific points of an operation, use knowledge of the implementation itself.

1. **AC-A1** — Given a write transaction, when it opens in Open mode a tree that does not exist, then it fails with
   the tree not found class with its name; in Create new mode, it creates it empty, and again in that mode it fails
   with tree already exists with its name; in Open or create mode, it creates it if it does not exist and, if it
   exists, opens it with its entries; and given a read transaction, when it opens a tree that does not exist, then
   it fails with tree not found (A1, A6).
2. **AC-A2** — Given a tree with entries, when it is opened in Create or empty or Empty mode, then it is left with
   no entries, with number of entries `0`, and an earlier handle of the same tree sees it empty and keeps working;
   given a tree that does not exist, Create or empty creates it and Empty fails with tree not found; and given a
   tree created and filled in the same write transaction, Empty leaves it empty and the commit stores it empty (A1,
   A2).
3. **AC-A3** — Given a read transaction, when it opens a tree, checks whether it exists, lists, reads entries and
   queries the number of entries, then it works; and opening it in any other mode, deleting it, renaming it or
   storing, adding or removing entries does not compile (A1, A3, A4).
4. **AC-A4** — Given an existing tree, when it is asked whether it exists, then yes, and after deleting it, no;
   given a write transaction on disk that only deletes a tree that does not exist, when it is committed, then the
   store's files are left identical byte for byte; and Exists and Delete with an empty name or one of 1,025 bytes
   fail with invalid size (A3, A10, B4).
5. **AC-A5** — Given trees `a` and `b`, when `a` is renamed to `c`, then `c` has the entries of `a`, `a` does not
   exist and an earlier handle of `a` keeps working; renaming `c` to `b` fails with tree already exists with the
   name `b`, and with overwriting it leaves `b` with the entries of `c`, the handle of `c` stays valid and an
   earlier handle of the original `b` fails with tree not found, with the name `b`; renaming a tree that does not
   exist fails with tree not found with its name, also to its own name; renaming `b` to `b` changes nothing; and
   given a tree `t` opened, renamed to `d` and deleted, when the handle of `t` is used, then it fails with tree not
   found with the name `d` (A3, A6).
6. **AC-A6** — Given a tree `a` renamed to `c` and again to `a`, when its original handle is used, then it keeps
   working; and given `a` renamed to `c`, a new tree created as `a` and `c` renamed to `a` with overwriting, when the
   handle of the new `a` is used, then it fails with tree not found (A3, A8).
7. **AC-A7** — Given trees created out of order, including names that differ only in case, when they are listed in a
   read transaction, then they come out with their type in byte order; in a store with no trees, the list is empty;
   in a write transaction, the list includes those created, renamed and deleted by it; and the list obtained is
   still available after deleting a tree and after the transaction ends (A3).
8. **AC-A8** — Given a tree, when a new key is stored and then the same key with another value, then Read returns
   the second value; Add with that key returns that it did not add and the value does not change; Add with a new key
   returns that it added; Read of a key that is not there returns that it is not there, and Read of a key stored
   with an empty value returns an empty value; Remove returns that it was there the first time and that it was not
   there the second; and Add with a key that is already there and a value of 1,025 bytes fails with invalid size
   (A4).
9. **AC-A9** — Given a write transaction on disk that only does Add on existing keys, Remove on keys that are not
   there, Open or create on an existing tree and operations that fail with tree not found or tree already exists,
   when it is committed, then it ends without error and the store's files are left identical byte for byte; and a
   write transaction on disk that only empties an already empty tree does write to the store (A10, T10).
10. **AC-A10** — Given a tree with enough entries for pages to split and merge, so that the tree gains and loses
    levels, when new keys are stored, overwritten, new and existing keys are added, present and absent keys are
    removed, all are removed down to the last one and it is emptied, then after every operation the number of entries
    matches the keys that Read finds among all those the test used; a read transaction opened earlier sees the number
    of its snapshot; after aborting a write transaction, the number is the one from before; after committing and
    reopening, the number is kept; and with a page of the tree itself damaged, querying it does not fail (A5, A9).
11. **AC-A11** — Given a write transaction with a tree `t`, when it repeats ten times, alternating, these two steps:
    (a) open `t`, delete it and create it again in Create new mode; (b) open `t`, create a tree `u` in Create new
    mode and rename `u` to `t` with overwriting; then every handle of `t` prior to the last replacement fails with
    tree not found, and only a handle of `t` made after the last replacement sees the current tree (A6, A8).
12. **AC-A12** — Given a tree with one of its pages damaged, when it is opened, asked whether it exists, listed or
    renamed without overwriting, then it does not fail; when an entry from that page is read, then it fails with the
    damage class and the page number, and fails the same way again when repeated; and when a write transaction
    deletes it, empties it, overwrites it with Rename or stores in that page and fails with damage, then committing
    it fails with the damage class and applies nothing (A9, T11).
13. **AC-A13** — Given operations that meet several situations at once, when they are run, then they fail with the
    one that wins according to A7: renaming with a source of 1,025 bytes to an existing destination gives invalid
    size; renaming a source that does not exist to a destination of 1,025 bytes gives invalid size; renaming a
    source that does not exist to an existing destination gives tree not found, also in a multi-page catalog with
    the destination's page damaged and the page where the source would be intact;
    storing a key of 2,049 bytes through a handle of a deleted tree gives invalid size; and a handle of a deleted tree
    whose page is damaged gives tree not found (A7).
14. **AC-A14** — Given a new store, when its information is queried before creating any tree, then its size is at
    most 1 MiB, the free pages are the pages minus the store's own (AC-F15) and the list of trees is empty (A12).
15. **AC-A15** — Given a store with a tree already created, which stays within a single region of the free space,
    and taking the pages in use (the pages minus the free pages that the information gives) when each commit is
    durable (in temporary mode, when it returns): when twice in a row a tree `x` is created, filled with enough
    entries to take up several pages and several levels, committed, deleted and committed, then the pages in use
    after the second deletion are the same as after the first, and those from before the first cycle when the entry
    of `x` did not make the catalog grow (the tests know this from the implementation); and when an empty tree is
    created, committed, the value is taken, it is filled the same way, committed, emptied and committed, then the
    final value is the one taken with the empty tree (A2, A3, F10, F12).
16. **AC-A16** — Given a write transaction that creates, renames, empties and deletes trees, and stores and removes
    entries with page splits and merges, when a process crash and a simulated power cut are caused at every point
    where its commit writes or carries data to disk, then on reopening the list of trees, every Read of the keys the
    test used and the number of entries of each tree are all as before the transaction or all as after, never
    halfway, and the number of entries matches the keys that Read finds (G1, G5, A5).
17. **AC-A17** — Given a write transaction, when it stores a value of 1,024 bytes with a key of 2,048 bytes, then it
    is stored; and one of 1,025 bytes fails with invalid size, with minimum `0` and maximum `1024` (A11, B6).
18. **AC-A18** — Given a store of format `5`, even if it is empty, when it is opened, then it fails with the
    different format class as an earlier format (A13, AC-B12).
19. **AC-A19** — Given the measurement of A15, when the closing report is reviewed, then it contains its times, the
    store sizes and the machine data on Windows and on Linux (A15).
20. **AC-A20** — Given the code of a test application with a reference read from a tree, when it uses it after
    opening another tree in Open mode, after Exists, after List or after Read, then it compiles; when it uses it
    after opening a tree in any other mode, after Delete or after Rename, even if they fail or change nothing, then
    it does not compile; and a handle of a read transaction can be passed to another thread, while passing one of a
    write transaction to another thread does not compile (A10, B1).

## Annotations to other PRDs

This PRD refines rules of earlier documents, which carry the corresponding annotations:

- Trees module, B7: the public validation errors of the module also include tree not found and tree already
  exists, and their precedence is that of A7 (A6, A7).
- Trees module, B9: the error of a handle of a deleted tree is tree not found, with the tree's last name (A6).
- Trees module, B9: opening a tree in Open or create mode ends the earlier references even if the tree exists,
  because the compiler does not know whether it will create it (A10).
- Trees module, AC-B18: the part on failures during a structure change simulates a T11 failure at every point where
  the operation reserves, reads or frees a page, also after having modified several, in splits and merges that
  propagate through several levels. Committing then fails with the class of the first failure and applies nothing,
  and after aborting, or after that failed commit, the tree, the catalog and the number of entries are as before and
  a new write transaction reads and modifies them without error. The tests use knowledge of the implementation to
  reach each point.
- Root parent, D11; trees module, B10: List returns a copy owned by the application, not references (A3).
- Transactions, T10 and T11: operations that fail with tree not found or tree already exists are not changes and do
  not prevent committing (A6, A10).
- Root parent, D7; Pager, P5; Journal, J3; grouped writing, E15; Free space, F11; Transactions, the whole document:
  the current format version becomes `6`, and a store of format `5` is of an earlier format (A13).

## References

- Parent: [trees.prd.en.md](trees.prd.en.md) v1.0; root: [krummi.prd.en.md](krummi.prd.en.md) v1.1
- Related: [transactions.prd.en.md](transactions.prd.en.md), [free-space.prd.en.md](free-space.prd.en.md)
- .NET, `File` and `FileMode`: <https://learn.microsoft.com/en-us/dotnet/api/system.io.file?view=net-10.0>,
  <https://learn.microsoft.com/en-us/dotnet/api/system.io.filemode?view=net-10.0>
- .NET, `Dictionary<TKey,TValue>`:
  <https://learn.microsoft.com/en-us/dotnet/api/system.collections.generic.dictionary-2?view=net-10.0>
- Voron, trees: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron/Data/BTrees>

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| 31c36231 | 2026-10-24 10:22 UTC+01:00 | 20 h 59 min | 19 h 3 min | 1 h 56 min | 45 min | 1 h 11 min | 32 |

<!-- prd-time: [{"session": "31c36231-b691-4c62-8559-6f87e09084cf", "start": "2026-10-24T09:22:37.952000+00:00", "wall": 75547, "discarded": 68592, "agent": 2676, "human": 4278, "interventions": 32, "idle_minutes": 10}] -->
