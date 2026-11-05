# Krummi Fixed-Size Trees — PRD

**Version**: 1.0
**Date**: 2026-11-02
**Parent document**: [Krummi B+ Trees — PRD](trees.prd.en.md) (v1.0)

## Objective and context

Build child PRD **5.5** of the [split of the trees module](trees.prd.en.md#split-into-child-prds): fixed-size
trees, whose keys are unsigned 64-bit integers in numeric order (B5) and whose values all have the same length, fixed
when the tree is created (B3), with the key operations of 5.1 and the scans of 5.2.

When this change closes, an application that indexes by a number (an identifier, a timestamp, a page number) can store
those entries without converting the number into sortable bytes and without paying, for each entry, the lengths of a
key and of a value that do not change. As in Voron, the reason is density and CPU cost, not a new function: a normal
tree with the key stored as 8 big-endian bytes already sorts the same way. That is why the change includes a
measurement that compares the two (X13).

**Why now**: it is the fifth submodule of the split. The later structures that store integers by number, such as the
tables (module 7), can build on it.

This PRD inherits everything from its ancestry, the [trees module](trees.prd.en.md) (B1–B12) and the
[root](krummi.prd.en.md), and what the parent inherits from the earlier modules: the [Pager](pager.prd.en.md)
(P1–P17), the [Transactions](transactions.prd.en.md) (T1–T18), the [Journal](journal.prd.en.md) (J1–J17), the
[grouped and direct journal writing](journal-group-commit.prd.en.md) (E1–E16) and the
[Free space](free-space.prd.en.md) (F1–F14). It also reuses what its siblings
[Trees and Key Operations](trees-keys.prd.en.md) (A1–A15 and its annotations),
[In-Order Scan and Prefix Search](trees-scan.prd.en.md) (R1–R11),
[Large Values](trees-large-values.prd.en.md) (V1–V11) and [Multi-Value Trees](trees-multivalue.prd.en.md) (M1–M15)
set, and the change [Remembered Node Layout Check](trees-node-check-cache.prd.en.md) (C1–C9). Only what is specific to
5.5 is added here. What changes or refines earlier rules is declared in its decisions and annotated in the affected
documents (see [Annotations to other PRDs](#annotations-to-other-prds)).

In this document, the **length** of a fixed-size tree is the number of bytes that all its values have.

### Product decisions

| ID | Decision |
|---|---|
| X1 | **Length of the values** (refines B3 and B6): the length of a fixed-size tree is fixed when it is created and is from 0 to 2,048 bytes, the same maximum as a key (B5) and as a multi-value value (M1). It does not change while the tree exists: emptying it (A2) and renaming it (A3) preserve it. Every value that an operation receives has exactly that length; with another, it fails with invalid size, as a value, with the size received and the length of the tree as minimum and maximum (B8). With length `0`, the tree stores only keys and each value is empty; Read still distinguishes a key with its empty value from one that is not there (A4). In fixed-size trees there are no off-leaf values (5.3). |
| X2 | **Invalid requested length**: every opening of a fixed-size tree requests its length, in all the modes of A1, also in Open and in a read transaction. A requested length of more than 2,048 bytes fails with invalid size, as a value, with the requested number as the size received, minimum `0` and maximum `2048`, without creating or opening anything. Like a .NET `ArgumentException`, it is an argument that is invalid by itself, decided only with the arguments without looking at the store, so it takes precedence over everything else except the invalid size of the name (B7, X11): over tree not found, tree already exists, different tree type and any failure reading the catalog. |
| X3 | **The type includes the length** (refines B3 and B8): the type of a fixed-size tree is “fixed-size of N bytes”, and two different lengths are two different types. Opening or creating, in any mode of A1, as fixed-size of N bytes an existing fixed-size tree with another length, or a normal or multi-value tree, fails with different tree type; and opening or creating as normal or multi-value a fixed-size tree does too. The data of the error (B8) carry the type the tree has and the type requested, each with its length when it is fixed-size. List (A3) reports the type of each fixed-size tree with its length, and Type (X15) that of a tree by its name. The operations that name a tree without requesting a type (Exists, Type, Delete, Rename) never fail with different tree type (B3). |
| X4 | **Opening by type** (like M4): a fixed-size tree is opened with the same modes of A1, with its own handle, which has only the operations of its type. Using an operation of the normal trees (5.1, 5.2) or of the multi-value ones (5.4) with its handle does not compile, and neither does using the operations of this PRD with the handle of a normal or multi-value tree. The number of entries (A5) is in the handles of the three types. The type requested when opening is still checked against the catalog (X3). The catalog operations (Exists, Delete, Rename and List, A3, and Type, X15) are valid for the three types. The rules of the handles of B1, B9, A6, A7, A8 and of the exception to B1 annotated in the parent (misuse with a handle of another transaction) apply the same to those of a fixed-size tree. |
| X5 | **Keys** (refines B5): a key is an unsigned 64-bit integer, from `0` to `2^64−1`, and keys are ordered by their numeric value. Every integer is a valid key, so no key and no bound of a scan ever fails with invalid size. The operations receive and return keys as integers, not as bytes. |
| X6 | **Key operations** (like A4 and A5): **Store**, **Add**, **Read** and **Remove**, with the rules of A4, and the **number of entries**, with those of A5. Read returns the value as a zero-copy reference (D11, T16), with the lifetime of those of Read in 5.1 (B9, A10, T8); with length `0`, the value is empty. Read is in both kinds of transaction; Store, Add and Remove, only in write ones. The only size validation is the length of the value in Store and Add (X1); Add with a value of another length fails even if the key is already there. |
| X7 | **Scans and Remove range** (like R1–R3 and R6–R9): a scan receives a lower and an upper bound, each an included or excluded integer, or open, and is consumed from both ends with the rules of R1 and R2. It returns each entry as its key, an integer, and its value, a zero-copy reference with the lifetime of R6. The interval is empty when no integer meets both bounds (extends R3 to integer keys): when the lower bound is greater than the upper; when both are the same integer and either is excluded; when both are excluded and are contiguous integers, such as `5` and `6`; when the lower is `2^64−1` excluded; or when the upper is `0` excluded. Those are exactly the cases of the rule. As in R3, it is decided only with the arguments: the scan returns nothing and Remove range removes `0` without reading any page, so they cannot fail by damage. **Remove range** removes the entries of an interval with those same rules and those of R7, and returns how many it removed. There is no prefix (R4): a byte prefix makes no sense over integers, and the bounds express any interval of keys. |
| X8 | **Changes and references** (like A10, R6 and R7): all the operations that can modify end the earlier references and scans: the modes other than Open, Delete, Rename, Store, Add, Remove and Remove range, even if they change nothing or fail. Open, Exists, Type, List, Read, the number of entries and requesting a scan or taking entries from it do not end them. For T10 the changes are the same as in A10 and R7: Store always is, also with the same value the key already had; Add, Remove and Remove range, when they add or remove something. Those operations are not changes when they change nothing, nor are those that fail with a public validation error. Emptying is always a change (A2). |
| X9 | **Space** (F8): the pages that a fixed-size tree stops using return to the free space, and all its pages return on emptying it (A2), deleting it and overwriting it with Rename (A3). An emptied fixed-size tree takes up the same as a newly created empty one, and one left with no entries, by any combination of Remove and Remove range, the same as an emptied one. |
| X10 | **Damage** (G6, like A9 and R8): the modes Open, Create new and Open or create, Exists, Type, List, Rename without overwrite or to its own name and the number of entries never fail with damage in the pages of the tree itself. Read, Store, Add, Remove, the scans and Remove range fail with the damage class and the page number when they need a damaged page of the tree, also when the result would have been not adding or not removing anything; the pages they need are those of A9 and R8. Delete, emptying and Rename with overwrite can fail with damage in any page of the affected tree. A damaged page fails the same way in every later operation that needs it (F9), except for the residual risk of the exception to G6 of the root parent (X14, C7). In a write, every failure by damage follows T11. |
| X11 | **Errors**: there are no new classes. The precedence is that of A7 and R8: first invalid size, that of the name before that of the length requested when opening (X2); in Store and Add, that of the value (X1); then, a handle of another transaction (misuse) and, after it, a handle of a tree that the same write deleted (tree not found, with the last name of the tree), both without reading any page; then, the failures reading the catalog, different tree type, tree not found and tree already exists; and, last, the failures reading the pages of the tree. |
| X12 | **Format `9`**: the change is incompatible (B12): an engine of format `8` does not know the fixed-size type and takes its entry in the catalog as damaged, so it would misread a healthy store: List, and opening, deleting or renaming that tree, would fail with the damage class. The change does not change the size or the free pages of a store without trees (A12). The current format version becomes `9`, and a store of format `8` is of an earlier format even if it is empty; before 1.0 there is no migration and the data is recreated (D7). The numbers that the format stores for a fixed-size tree, such as its length and its keys, are stored in little endian, like the rest of the format (V3); the order of the keys is the numeric one (X5), not that of their bytes. |
| X13 | **Measurement** (B12, no threshold by D5): the change includes a reproducible measurement, on Windows and on Linux, on a store on disk with durable commit, that compares a fixed-size tree with a normal tree that stores the same key as 8 big-endian bytes, with values of 0, 8 and 100 bytes of random content. The keys are 1 million distinct integers chosen at random between `0` and `2^64−1`, and the keys and the values are the same in all the loads thanks to a fixed seed. In the normal tree, converting each key into its 8 bytes when storing it and back into an integer when reading it is part of the measured time. For each tree type and each value length it measures: storing the entries in key order in one store and, in another, at random, both loads in writes of 1,000 entries; the size of the store in bytes that its information gives (P12) after each of the two loads; and, in the store loaded at random, after closing and reopening it without emptying the system cache, reading all the entries at random in a single read transaction and scanning them all forward in another, reading in both cases the key and the bytes of each value returned, and removing them all at random in writes of 1,000. The report includes the operating system, the processor and the disk. Its results go in the closing report. |
| X14 | **Remembered layout check** (refines C1): the pages of a fixed-size tree are nodes for the purposes of C1–C8 and use the remembered check like the other nodes and over the same bytes: it is remembered that the content of a node passed the layout check, identified by its page number and by its integrity code, in the store's memory of remembered nodes, shared with the other trees. The guarantee and the residual risk of C7 apply to them. Whether what is remembered is used for all their nodes or only from a certain number of entries is left to the implementation, as for the other nodes (left to the implementation of the remembered check). |
| X15 | **Type of a tree** (extends A3): a new catalog operation, **Type**, receives a name and returns the type of the tree that has it, with its length when it is fixed-size, or that it does not exist; it does not fail because it does not exist, like Exists. It is valid for the three types and is in both kinds of transaction. It validates the size of the name (B4), never fails with different tree type (B3) and only reads the catalog: it can fail with the failures reading the catalog (damage, out of memory, input and output), but never with damage in the pages of the tree itself. Its precedence is that of Exists: first the invalid size of the name and then the failures reading the catalog. It modifies nothing: it does not end the references or the scans (X8) and is not a change for T10. It sees what its transaction sees, like Exists and List (A3): in a write, with its own changes to the catalog (trees created, deleted, renamed or recreated with another type), and in a read, its snapshot; it returns the same as List reports for that name. |

*Annotations of 2026-11-05 from the remembered page integrity ([pager-integrity-once.prd.en.md](pager-integrity-once.prd.en.md)):*

- *X10: a damaged page still fails in every operation that needs it, except for the residual risk of decision I8, which replaces that of C7.*
- *AC-X19: its first part is refined as AC-C1 (the integrity of each node is checked on its first read, not on all of them), and its fourth part (a byte changed with the store open in a remembered leaf) is replaced by AC-I8.*

### Starting state

Verified on 2026-11-02 in the code:

| Aspect | Situation |
|---|---|
| Public API | Normal trees have the operations of 5.1, the scans and Remove range of 5.2 and the values of up to 256 MiB of 5.3; multi-value ones, those of 5.4. Each type has its own handle, and using an operation of one type with the handle of another does not compile (M4). The number of entries is common to the two types. |
| Types | There are two types, normal and multi-value; the list of types admits new types without breaking the code of the applications that use it, and has no value reserved for fixed-size. List returns the name and the type of each tree; there is no operation that gives the type of a single tree by its name, and Exists only says whether it exists. The different tree type error reports the type it has and the requested one, with no further data. |
| Keys | The keys of normal and multi-value trees are bytes from 1 to 2,048, ordered byte by byte (B5). An integer stored in big-endian sorts in numeric order; in little endian, it does not. |
| Density | An entry of a normal tree with a key of 8 bytes and a value of N bytes takes up 14 + N bytes of its leaf: 8 for the key, N for the value and 6 for its position and its two lengths. |
| Errors | Sixteen classes, those of the starting state of 5.4. Invalid size reports name, key, value or prefix. |
| Format | The current format version is `8` (M14). A store of format `6` created with the version of 5.1 and one of format `7` created with that of 5.3 are kept as test data; none of format `8` is kept. |
| Measurement | There are measurements of normal trees (A15), of scans (R11), of large values (V11), of multi-value ones (M15) and of the remembered check (C9); none compares integer keys. |
| Voron (reference) | Its fixed-size trees have signed 64-bit integer keys and values of a fixed length that is given when they are opened; they have add, delete, read, count, scan from a key and delete a range. |

## Actors

- **Integrating developer**: creates and opens fixed-size trees with their length, stores, reads, removes and scans
  entries by integer key, and handles the errors by their class.
- **Maintainer**: runs the measurement of X13 on Windows and on Linux.

## Flow

1. The application opens a transaction and a fixed-size tree with its own handle, its name and its length
   (X2–X4).
2. In a write it stores, adds and removes entries, or removes intervals of keys (X6, X7).
3. In any transaction it reads entries by key, queries the number of entries and scans the tree (X6, X7).
4. The commit applies all the changes, or none (G1).

## Validations and messages

Those of the ancestors and of 5.1 to 5.4 apply, including the failures of T11 and T12, with the precedence of X11.
This PRD adds these situations, in the library only (the tool does not open stores until module 12):

| Situation | Rule | Error class |
|---|---|---|
| A fixed-size tree is opened or created requesting a length of more than 2,048 bytes, in any mode | X2 | Invalid size, as a value, with minimum `0` and maximum `2048` |
| A value of a length other than the tree's is stored or added | X1, X6 | Invalid size, as a value, with the length of the tree as minimum and maximum |
| A fixed-size tree of another length, or a normal or multi-value one, is opened or created as fixed-size of N bytes; or a fixed-size one as normal or multi-value | X3 | Different tree type, with the two types and their lengths |
| An operation is made with the handle of a fixed-size tree of another transaction of the same store | X4, X11 | Misuse |
| An operation is made with the handle of a fixed-size tree that the same write deleted | X4, X11, A6 | Tree not found |
| An operation needs a damaged page of the tree | X10, G6 | Damage, with the page number |
| An operation of one type is used with the handle of a tree of another type, or a write operation in a read transaction | X4, X6, B1 | None: it does not compile |
| Any integer key or bound, including `0` and `2^64−1` | X5 | None: it is valid |
| The type of a name that has no tree is requested | X15 | None: it returns that it does not exist |
| A key that is already there is added, a key that is not there is read or removed, or an empty interval or one without keys is scanned | X6, X7 | None: it returns that it did not add, that it is not there, that it was not there, `0` or nothing |

## Output

The module writes nothing to the screen. The store information (P12) keeps its data; the format version it reports
becomes `9` (X12). List reports each fixed-size tree with its type and its length; normal and multi-value ones are
still reported as before. The values are returned with the stored bytes, unchanged, and the keys as integers, in
numeric order.

Example, for a fixed-size tree `contadores` of length 8, with the keys `0`, `7` and `2^64−1`:

| Operation | Result |
|---|---|
| List | `contadores`, fixed-size of 8 bytes |
| Type of `contadores` | fixed-size of 8 bytes |
| Type of `pedidos`, which does not exist | does not exist |
| Full scan, forward | `0`, `7`, `2^64−1`, each with its value of 8 bytes |
| Scan from `7` excluded to the end, backward | `2^64−1` |
| Scan from `8` to `5`, both included | nothing (empty interval) |
| Read `3` | not there |
| Add `7` with a value of 8 bytes | did not add |
| Remove range from `1` to `2^64−1`, both included | `2`; the number of entries becomes `1` |

Example of the data of an invalid size error, for storing a value of 9 bytes in that tree, in a store on disk:

| Datum | Value |
|---|---|
| Folder | the store's |
| What violates its size | value |
| Size received | `9` |
| Minimum | `8` |
| Maximum | `8` |

Example of the data of a different tree type error, for opening `contadores` as fixed-size of 16 bytes:

| Datum | Value |
|---|---|
| Folder | the store's |
| Name | `contadores` |
| Type it has | fixed-size of 8 bytes |
| Type requested | fixed-size of 16 bytes |

## Reuse

- The common rules of the trees module (B1–B12) and those of 5.1 to 5.4: opening modes, catalog, errors,
  precedence, damage, identity, changes, number of entries, scans with their bounds and ends, Remove range and
  opening by type.
- The invalid size class with its value datum, and the different tree type class, with no new classes (X11).
- The free space (F4–F8) for the pages that are freed (X9).
- The transactions, their snapshot and the zero-copy reads (T2, T14, T16, D11).
- The journal and the recovery (G1, G5): the changes of fixed-size trees travel like any other change.
- The integrity check and the damage class (G6), and the remembered layout check (C1–C8, X14).
- The simulation of crashes, power cuts and failures of the tests of modules 3 to 5.4.
- The measurement of 5.1 (A15) as a model of loads and report for X13.
- The stores of format `6` and `7` kept as test data, which are still rejected as an earlier format, and one of format
  `8` that the execution creates with the version before this change and keeps the same way (AC-X16).

## Out of scope

- Signed integer keys (parent): the application that needs them converts them itself into ordered unsigned integers;
  integer keys of another width (32 or 128 bits) and fixed-size keys that are not integers.
- Values of variable length or of more than 2,048 bytes in a fixed-size tree, off-leaf values, and multi-value
  fixed-size trees.
- Changing the length of an existing tree and converting a tree of one type into another (B3, X1, X3).
- Opening a fixed-size tree without giving its length (X2); whoever does not know it gets it with Type (X15) or with
  List (X3).
- Prefix search in fixed-size trees (X7), and what the in-order scan already leaves out of scope, such as counting the
  entries of an interval without scanning it.
- Fixed-size trees stored inside an entry of another tree, as Voron does; here they are always trees of the catalog.
- Combined read and write operations, such as incrementing a counter (parent), and reading or storing several entries
  in a single operation.
- Using fixed-size trees inside the engine itself, for example for the free space.
- Compression of leaf pages (5.6); whether fixed-size trees can be compressed is decided by 5.6.
- Migrating a store of format `8` (D7).
- Performance thresholds (D5, module 6).

## Left to the implementation

- The layout of the pages of a fixed-size tree, as long as it meets X12, and its split and merge policy (parent).
- How the type with its length is requested when opening and how it is represented in List, in Type and in the data of
  different tree type, as long as it meets X3; and the names of the handle and of the new operations (parent).
- Which page is reported in a damage error when an operation needs several damaged ones (X10), as long as it is one of
  those it needs, also when the operation is repeated.
- Which failure of T11 is reported when a single operation would cause several, according to what it tries first, and
  what the write itself sees after that failure, since it can no longer be committed.
- The command that runs the measurement of X13 and the shape of its report.

## Acceptance criteria

The test conditions of the earlier modules and the header of the parent's criteria apply: each criterion runs on a
store on disk and in temporary mode, except the parts that reopen the store or damage its files, which are on disk
only. This PRD checks the parent's criteria assigned to 5.5: AC-B7 in its fixed-size part (AC-X1), AC-B16 (AC-X3) and
AC-B8 with fixed-size (AC-X4), and AC-B17 and AC-B18 with fixed-size trees (AC-X11 and AC-X12); it repeats AC10 and AC20
of the root with fixed-size trees (AC-X13 and AC-X14) and AC-B4, AC-B9 and AC-B13 of the parent with the requested
length (AC-X17), and adds these. The tests that depend on the layout of the pages, on damaged pages, on failures at
specific points or on counting the layout checks use knowledge of the implementation itself.

1. **AC-X1** — Given a fixed-size tree, when the keys `0`, `1`, `2^63−1`, `2^63`, `2^64−2` and `2^64−1` and others at
   random are stored in random order, then a full scan returns them in numeric order forward and in the inverse order
   backward, in the same write, in a later read and, on disk, after reopening (X5, AC-B7).
2. **AC-X2** — Given a write, when it creates fixed-size trees of lengths `0`, `1`, `8` and `2048`, then they are
   created, and in each one identical values of its length are stored and read; in the one of length `0`, Read
   distinguishes a stored key, with its empty value, from one that is not there; and when it opens or creates one of
   length `2049`, in each mode of A1 and also in Open in a read transaction, with and without a tree of that name, then
   it fails without effect with the invalid size class, as a value, with the size received `2049`, minimum `0` and
   maximum `2048`, and creates nothing (X1, X2).
3. **AC-X3** — Given a fixed-size tree of length 8, when a value of 0, 7 or 9 bytes is stored or added, also with a key
   that is already there, then it fails without effect with the invalid size class, as a value, with the size received,
   minimum `8` and maximum `8`, and the transaction stays usable; and with a tree of length 0 and a value of 1 byte,
   the same, with minimum `0` and maximum `0` (X1, X6, AC-B16).
4. **AC-X4** — Given a fixed-size tree of length 8, a normal one and a multi-value one, when the fixed-size one is
   opened or created, in each mode of A1, as fixed-size of 16 bytes, as normal and as multi-value, and the other two as
   fixed-size of 8 bytes, then it fails with the different tree type class, with the name, the type it has and the type
   requested, with their lengths, and the tree is left intact; it fails the same way with a page of the tree's own
   damaged and the catalog intact; deleting it without requesting a type never fails with that class; List reports
   each tree with its type and, in the fixed-size one, its length; renaming it and emptying it preserve its length;
   Rename with overwrite deletes a fixed-size destination tree; and after deleting the fixed-size one, creating
   another with its name and another length creates it (X3, X4, AC-B8).
5. **AC-X5** — Given an empty fixed-size tree, when entries are stored, added, read and removed, with the keys `0` and
   `2^64−1` among them, then Store creates or overwrites, Add returns that it added the first time and that it did not
   add the second without changing anything, Read returns the stored value or that it is not there, Remove returns that
   it was there the first time and that it was not there the second, and the number of entries matches at each step the
   keys stored and not removed (X6, A4, A5).
6. **AC-X6** — Given a fixed-size tree with entries, when it is scanned with included, excluded and open bounds, with
   `0` and `2^64−1` as bounds and with bounds that are not keys of the tree, alternating the two ends, then each scan
   returns exactly what R1 and R2 say in numeric order; with each empty-interval case of X7 (the lower bound greater
   than the upper, both equal and either excluded, `5` and `6` excluded, from `2^64−1` excluded and to `0` excluded),
   the scan returns nothing and Remove range removes `0`, also with the root page of the tree damaged (on disk); and
   when intervals are removed, also the whole tree, then Remove range returns how many it removed, the number of
   entries goes down by that amount and a scan returns the others (X7, R1–R3, R7).
7. **AC-X7** — Given the code of a test application, when it uses with the handle of a fixed-size tree an operation of
   the normal or multi-value trees, or an operation of this PRD with the handle of a normal or multi-value tree, or
   Store, Add, Remove or Remove range in a read transaction, or uses a reference or a scan after one of the operations
   of X8 that failed without effect, or moves to another thread the handle of a fixed-size tree of a write or a scan of
   it, then it does not compile; moving to another thread that of a read and its scans compiles; and Read, the number of
   entries and the scans do not end the earlier references or scans (X4, X8, B1, R9, AC-B11).
8. **AC-X8** — Given a store with a fixed-size tree already created, which stays within a single region of the free
   space, taking the pages in use as in AC-A15: when an empty fixed-size tree is created, committed, the value is
   taken, enough entries for several levels are stored in it and committed, and then all of them are removed with each
   of these forms in separate tests (Remove, Remove range, a combination of the two and emptying it), then the final
   value is the one taken with the empty tree; and when the tree is deleted or overwritten with Rename, the final value
   is the one from before creating it when its entry did not make the catalog grow, as in AC-A15 (X9, A2, A3).
9. **AC-X9** — Given a write with a fixed-size tree opened twice, and another transaction of the same store, when it
   deletes it through one handle and uses the other with each operation of this PRD, also after creating another tree
   with the same name, then it fails without effect with tree not found and the last name of the tree; when it uses the
   handle in the other transaction, then it fails without effect with misuse; and with a value of another length in
   addition, in both cases, the error is invalid size (X4, X11, A6, A7, AC-B14).
10. **AC-X10** — Given a fixed-size tree on disk of at least two levels, with a damaged page in the part that leads to
    some keys, when those keys are read, stored, added or removed, also one that is already there or that is not there,
    or an interval that contains them is scanned or removed, then each operation fails with the damage class and the
    page number, and fails the same way again when repeated (except for the residual risk of C7, which the test does
    not provoke); the scan ends after the failure and its earlier entries are correct; the keys whose path from the
    root does not go through that page are read without error; the modes Open, Create new and Open or create, Exists,
    Type, List, Rename without overwrite and the number of entries never fail with damage in the pages of the tree
    itself, and give their usual result or error (for example, Create new on that tree fails with tree already
    exists); and when a write empties, deletes or overwrites that tree with Rename, then it can fail with the damage
    class and the write follows T11 (X10, A9, R8, G6, F9).
11. **AC-X11** — Given an open read transaction and a write that, with insertions and deletions, splits and merges pages
    of a fixed-size tree until its height changes, when the read reads all the entries and the number of entries before
    and after the write commits, then it sees exactly those of its snapshot; and a new read sees those of the write
    (G3, T14, AC-B17).
12. **AC-X12** — Given a write that splits and merges pages of a fixed-size tree, when a process crash and a simulated
    power cut are provoked at each point where its commit writes or takes data to the disk, then on reopening the tree
    has all its entries from before or all those from after, with its number of entries; and when a failure of T11
    other than input and output is simulated at each point where an operation reserves, reads or frees pages, then
    committing fails with the class of the first failure and applies nothing, and after aborting the write the tree and
    its number of entries are as before and a new write works without error (G1, G5, T11, AC-B18).
13. **AC-X13** — Given a small store with a fixed-size tree of at least two levels, when each of its bytes is altered
    separately, it is opened and all its entries are read, then each alteration is detected as damage or has no effect,
    with the rules of AC10 of the root and its annotations; a key or a value with a different byte is never returned
    (X10, G6, AC10).
14. **AC-X14** — Given the values returned by Read and the scans of a fixed-size tree of length 8, when they are read,
    then the references are inside the engine memory that T16 sets and not in a copy, on disk and in temporary mode, and
    using a reference after the transaction ends does not compile (X6, X7, AC20).
15. **AC-X15** — Given a write on disk that only does Open or create on an existing fixed-size tree, Add of keys that
    are already there, Remove of keys that are not there, Remove range that removes nothing, Read, the number of
    entries, scans, and operations that fail by invalid size or by different tree type, when it is committed, then it
    ends without error and the store files are left byte for byte identical (X8, T10).
16. **AC-X16** — Given a store of format `8` that the execution creates with the version before this change, compiled
    separately, and keeps as test data, and those of format `6` and `7` already kept, when they are opened, then they
    fail with the different format class as an earlier format and are not modified; a new store reports the format
    version `9`, with the same size and the same free pages as a new store of format `8` while it has no trees; and
    reading from disk the bytes of a leaf of a fixed-size tree, with knowledge of the implementation, its keys, as the
    leaf stores them, and the length of the tree are in little endian (X12, A12, AC-B12).
17. **AC-X17** — Given an operation that opens a fixed-size tree with a name of 1,025 bytes and a length of 2,049, when
    it is run, then the invalid size error is the name's; and given an opening with a valid name and a length of 2,049,
    when it is run on an existing fixed-size tree of another length, on a normal one, on one that does not exist in
    Open mode, on an existing one in Create new mode and, on disk, with the catalog page damaged, then it fails in all
    cases with invalid size, without effect; in a store that does not accept writes (T12), it fails with that class and
    not with the input and output one; and a write with changes before and after failures of X1 and X2 is committed
    without error and applies all those changes (X2, X11, AC-B4, AC-B9, AC-B13).
18. **AC-X18** — Given a write on disk with a fixed-size tree, when it makes only one of these changes in separate
    tests (Store with the same value the key already had, emptying an already empty tree, Add of a new key, Remove of a
    key that is there, or Remove range that removes at least one entry) and is committed, then the store files change,
    as in any write with changes (X8, T10).
19. **AC-X19** — Given a fixed-size tree with its pages in the data file and room to remember all the nodes that the
    test reads, when a read transaction reads the same key several times and another reads it afterwards, then all
    return the same, and the layout of each node on the path is checked only in the first read of that node and its
    integrity in all of them; and given a page of one of its leaves in the data file with its layout damaged and the
    integrity code recalculated to be correct, when it is read twice in the same read transaction and once in another,
    then each read fails with the damage class and the number of that page, also when another correct version of that
    page had been remembered before; that same damaged page in the temporary area (committed and not moved) fails the
    same both times; a byte altered in the page of a remembered node without recalculating its code makes the next read
    fail with damage; and after closing and reopening the store, the first read of each node checks its layout again.
    The test uses nodes to which the implementation applies what is remembered (X14, C1–C4, C7, AC-C1, AC-C4, AC-C5,
    AC-C7, AC-C9).
20. **AC-X20** — Given a normal tree, a multi-value one and two fixed-size ones of lengths `0` and `8`, when their Type
    is requested by name in a read transaction and in a write one, then it returns the type of each, with its length in
    the fixed-size ones, the same as List; with a name that has no tree, also one deleted in the same write, it returns
    that it does not exist without failing; after renaming a tree and after deleting and recreating it with another
    type in the same write, it returns the current type with its new name; with an empty name or one of 1,025 bytes, it
    fails without effect with invalid size, as a name; with a page of one of those trees damaged and the catalog intact
    (on disk), it does not fail; on disk, with the catalog page damaged, it fails with the damage class, and with a name
    of 1,025 bytes it fails with invalid size; in a store that does not accept writes (T12), it works the same; it does
    not end the earlier references or scans, and a write on disk that only requests types is committed leaving the files
    byte for byte identical (X15, X8, T10).
21. **AC-X21** — Given the closing report, when it is reviewed, then it contains the measurement of X13 with its times,
    by phase, and the sizes of the store, for each tree type and each value length, and the data of the machine, on
    Windows and on Linux (X13).

## Annotations to other PRDs

This PRD changes or refines rules of earlier documents, which carry the corresponding annotations:

- Trees module, B3 and B8: the type of a fixed-size tree includes its length, and the data of different tree type carry
  it (X3).
- Trees module, B6: in fixed-size trees, each value has exactly the length of the tree, from 0 to 2,048 bytes (X1).
- Trees module, B7 and B8: the length requested when opening a fixed-size tree is validated as a value, with minimum
  `0` and maximum `2048`, after the name (X2, X11).
- Trees and Key Operations, A1: a fixed-size tree is opened in the same modes, with its length (X2, X4).
- Trees and Key Operations, A3: List reports the length of fixed-size trees (X3), and there is a new catalog
  operation, Type, which gives the type of a tree by its name for the three types (X15).
- Trees and Key Operations, *Out of scope* (an Exists that requests a type): Type is not an Exists with a type, it
  returns the type of the tree; it remains out of scope for Exists to check a type (X15).
- Trees and Key Operations, A5: the number of entries is also in the handles of fixed-size trees (X4).
- Trees and Key Operations, A9 and A10; In-Order Scan, R6, R7 and R8: they apply the same to the operations of
  fixed-size trees (X8, X10).
- Trees and Key Operations, A7: the invalid size of the requested length comes after that of the name (X11).
- Trees and Key Operations, *Out of scope*: fixed-size trees are set by this PRD (X1–X15).
- Remembered Node Layout Check, C1 and *Out of scope*: the pages of fixed-size trees are nodes that use what is
  remembered, over the same bytes as the others (X14).
- In-Order Scan, *Out of scope*: the scans and Remove range of fixed-size trees are set by this PRD, without prefix
  (X7).
- In-Order Scan, R3: in fixed-size trees, the interval is also empty when no integer meets both bounds (X7).
- Large Values, *Out of scope*: fixed-size trees have no off-leaf values (X1).
- Multi-Value Trees, M4: the handles of fixed-size trees are a third type with the same rule (X4).
- Root parent, D7; Pager, P5; Journal, J3; grouped writing, E15; Free space, F11; Transactions, the whole document;
  Trees and Key Operations, A13 and its annotation of 5.3; In-Order Scan, R10, *Output* and AC-R14; Large Values, V9,
  *Output* and AC-V12; Multi-Value Trees, M14, *Output* and AC-M18; Remembered Node Layout Check, *Output*: the
  current format version becomes `9`, and a store of format `8` is of an earlier format (X12).

## References

- Parent: [trees.prd.en.md](trees.prd.en.md) v1.0; root: [krummi.prd.en.md](krummi.prd.en.md) v1.1
- Siblings: [trees-keys.prd.en.md](trees-keys.prd.en.md) v1.0, [trees-scan.prd.en.md](trees-scan.prd.en.md) v1.0,
  [trees-large-values.prd.en.md](trees-large-values.prd.en.md) v1.0,
  [trees-multivalue.prd.en.md](trees-multivalue.prd.en.md) v1.0
- Related: [trees-node-check-cache.prd.en.md](trees-node-check-cache.prd.en.md),
  [free-space.prd.en.md](free-space.prd.en.md), [transactions.prd.en.md](transactions.prd.en.md),
  [pager.prd.en.md](pager.prd.en.md)
- Voron, fixed-size trees: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron/Data/Fixed>

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| 67d5698f | 2026-10-31 08:14 UTC+00:00 | 47 h 39 min | 43 h 45 min | 3 h 54 min | 2 h 52 min | 1 h 2 min | 21 |

<!-- prd-time: [{"session": "67d5698f-6683-4ac4-83d2-7287ae689237", "start": "2026-10-31T08:14:08.343000+00:00", "wall": 171523, "discarded": 157472, "agent": 10311, "human": 3738, "interventions": 21, "idle_minutes": 10}] -->
