# Krummi Multi-Value Trees — PRD

**Version**: 1.0
**Date**: 2026-10-29
**Parent document**: [Krummi B+ Trees — PRD](trees.prd.en.md) (v1.0)

## Objective and context

Build child PRD **5.4** of the [split of the trees module](trees.prd.en.md#split-into-child-prds): multi-value
trees, whose keys have an ordered set of distinct values (B3), with operations to add, remove and check values, count
the values of a key and scan the keys and the values of a key.

When this change closes, an application can store in a tree, for each key, a set of values, as a secondary index
does: the key is the indexed value and its values are the keys of the records that have it. As in Voron, the values of
a key with few values travel together inside its entry, and those of a key with many pass to a tree of their own for
that key (M2), so that both situations are efficient.

**Why now**: it is the fourth submodule of the split. The indexes of the tables (module 7) store, for each indexed
value, the set of records that have it.

This PRD inherits everything from its ancestry, the [trees module](trees.prd.en.md) (B1–B12) and the
[root](krummi.prd.en.md), and what the parent inherits from the earlier modules: the [Pager](pager.prd.en.md)
(P1–P17), the [Transactions](transactions.prd.en.md) (T1–T18), the [Journal](journal.prd.en.md) (J1–J17), the
[grouped and direct journal writing](journal-group-commit.prd.en.md) (E1–E16) and the
[Free space](free-space.prd.en.md) (F1–F14). It also reuses what its siblings
[Trees and Key Operations](trees-keys.prd.en.md) (A1–A15 and its annotations),
[In-Order Scan and Prefix Search](trees-scan.prd.en.md) (R1–R11) and
[Large Values](trees-large-values.prd.en.md) (V1–V11) set. Only what is specific to 5.4 is added here. What changes
or refines earlier rules is declared in its decisions and annotated in the affected documents (see
[Annotations to other PRDs](#annotations-to-other-prds)).

In this document, the **values of a key** are the set of values that a key of a multi-value tree has; and a
**subtree** is the tree of its own in which the values of a key are stored when they exceed the promotion threshold
(M2).

### Product decisions

| ID | Decision |
|---|---|
| M1 | **Values** (B6): each value of a multi-value tree is a sequence of 0 to 2,048 bytes, the same maximum as a key (B5); the empty value is valid and, by the order of B5, goes before any other. The values of a key are distinct from each other and are ordered with the order of B5. Every value that an operation receives is validated the same way, also the one passed to look it up or remove it and the one that acts as a bound of a value scan (M7). A value outside that size fails with invalid size, as a value, with minimum `0` and maximum `2048` (B7, B8). In multi-value trees there are no off-leaf values (5.3). |
| M2 | **Two representations with two thresholds**: while the values of a key do not exceed the **promotion threshold**, they are stored together inside its entry; when an Add exceeds it, they pass to a subtree of that key. They return to the entry only when, on removing values, they fall below the **demotion threshold**, which is strictly lower than the promotion one; between the two thresholds, the key keeps the representation it has. This way, adding and removing values alternately near a threshold does not make the key pass from one representation to the other in every operation. The switch happens in the same operation that crosses the threshold, also when Remove value range (M8) removes at once values from above the promotion threshold to below the demotion one. The switch is not visible in the results of the operations: their returned values, their validation errors and the order of the values are the same in both representations; it shows only in the space used (M10) and in the pages that an operation needs (M11). Subtrees are not catalog trees: they have no name, do not appear in List, do not count as trees for Exists or for B11, have no identity of their own (A8) and are freed with their key (M10). |
| M3 | **Thresholds** (refines *Left to the implementation* of the parent): the two thresholds of M2 remain left to the implementation, but their choice rests on measurements. The execution measures at least three candidate promotion thresholds: a low one, which passes to a subtree early; a high one, which keeps the values in the entry as long as possible; and an intermediate one; and, for each of them, at least two candidate demotion thresholds: one close to the promotion one and another around its half. It runs all the loads of the measurement (M15) with each of those pairs, at least six, and chooses only with that data, by performance, space used and the complexity it adds; if the data does not clearly separate the candidates, it chooses the simplest pair and says so. The evaluation, with the results of each pair, the chosen one and why, goes in the closing report. |
| M4 | **Opening by type** (refines A1 and B3): a multi-value tree is opened with the same modes of A1, and each type is opened with its own handle, which has only the operations of its type. Using an operation of one type with the handle of a tree of another type does not compile: Store, Add, Read and Remove of 5.1, and the scans and Remove range of 5.2, with the handle of a multi-value tree; and the operations of this PRD with the one of a normal tree. The number of entries (A5) is in the handles of both types. The type requested when opening is still checked against the catalog (B3). The catalog operations (Exists, Delete, Rename and List, A3) are valid for both types; List reports the multi-value type. The rules of the handles of B1, B9, A6, A7, A8 and of the exception to B1 annotated in the parent (misuse with a handle of another transaction) apply the same to those of a multi-value tree. |
| M5 | **Operations by value** (like a .NET dictionary of `SortedSet`): **Add** (key, value) adds the value to the values of the key, creating the key if it is not there, and returns whether it added it; if the value was already there, it changes nothing, like `HashSet.Add`. **Remove value** (key, value) removes the value and returns whether it was there. **Contains** (key, value) says whether the key has that value. None fails because of the presence or absence of the key or of the value, but all validate their sizes first (B7): the key, and then the value. Contains is in both kinds of transaction; Add and Remove value, only in write ones. |
| M6 | **Keys** (refines A4 and A5): a key exists while it has at least one value. It is born with its first Add and disappears when it loses its last value, by Remove value or by Remove value range (M8); there are no keys without values, so knowing whether a key exists is knowing whether its number of values is not `0`. **Remove key** (key) removes the key with all its values and returns how many values it removed, or `0` if it was not there; it is only in write transactions. The **number of entries** of the tree (A5) is its number of keys. The **number of values** of a key (key), in both kinds of transaction, returns how many values it has as its transaction sees it, without reading its values, or `0` if the key is not there; it validates the size of the key. There is no total of values of the tree. |
| M7 | **Scans** (like R1–R6, R8 and R9): there are two. The **key scan** scans the keys of the tree with the rules of R1 to R6, R8 and R9, and returns each key with its number of values, not with its values; its bounds are keys and are validated as in R5. The **value scan** (key, bounds, prefix) scans the values of a key with those same rules applied to the values: two bounds, a prefix and the two ends; with a key that is not there, it returns nothing and does not fail. Its bounds are values and are validated as a value (M1), from 0 to 2,048 bytes: the empty value is a valid bound, included or excluded; the prefix is from 0 to 2,048 bytes, as in R5. Its validation order is the key, the lower bound, the upper bound and the prefix. An interval that is empty because of its bounds or its prefix (R3) is decided before reading anything, also the entry of the key. After a failure, the value scan ends as R8 says. The keys and values that both scans return are zero-copy references with the lifetime of R6. |
| M8 | **Remove range** (like R7): there are two, only in write transactions. **Remove key range** removes each key of the interval with all its values, with the rules of R2 to R5, and returns how many keys it removed, not how many values; the number of entries goes down by that amount. **Remove value range** (key, bounds, prefix) removes from a key the values of the interval, with the rules of M7, and returns how many values it removed; if the key is left with no values, it disappears (M6). With a key that is not there, it returns `0`. |
| M9 | **Changes and references** (like A10, R6 and R7, with the operations of this PRD): Add, Remove value, Remove key and the two Remove range end the earlier references and scans, even if they change nothing or fail. Contains, the number of values and requesting a scan or taking elements from it do not end them. For T10 they are changes: Add when it adds, Remove value when it removes, and Remove key and the Remove range operations when they remove some value or some key; those same operations are not changes when they change nothing, nor are those that fail with a public validation error. Emptying a multi-value tree is a change, as A2 says. |
| M10 | **Space** (F8): the values of a key in a subtree follow the space rules of a normal tree: the pages that the subtree stops using on removing values return to the free space (F8). All the pages of a subtree return to the free space in the same write in which the key returns to its entry, disappears or is removed, and on emptying the tree (A2), deleting it and overwriting it with Rename (A3), and become available according to F6. An emptied multi-value tree takes up the same as a newly created empty one (A2), and one left with no keys, by any combination of the operations of this PRD, the same as an emptied one, also if it had subtrees. Outside those cases, the space a tree takes up also depends on its history, because a key between the two thresholds keeps its representation (M2): two trees with the same keys and the same values can take up different amounts. |
| M11 | **Damage** (G6, extends A9 and R8): Add, Remove value, Contains, the value scan and Remove value range fail with the damage class and the page number when they need a damaged page of the tree or of the subtree of their key, also when the result would have been not adding or not removing anything. The number of values of a key and the key scan never read the subtrees, so a damaged page of a subtree does not make them fail; they do fail with damage in the pages of the tree that they need, like Read (A9) and R8. A page that an operation does not need does not make it fail, and the other keys are not affected. The operations that free a whole subtree read the pages they need to find its own, and can fail with damage in any page of the subtree, like Delete in A9: Remove key, Remove key range, emptying the tree (A2), deleting it and overwriting it with Rename (A3), and Remove value or Remove value range when the key returns to its entry or disappears. Unlike an off-leaf value (V7), a key with its subtree damaged may not be removable. A damaged page fails the same way in every later operation that needs it (F9). In a write, every failure by damage follows T11. |
| M12 | **Errors**: there are no new classes. The precedence is that of A7 and R8, with the sizes in the order of M5 and M7: first invalid size; then a handle of another transaction (misuse) and a handle of a tree that the same write deleted (tree not found), both without reading any page; and then, the failures reading the pages of the tree and of its subtrees. The different tree type carries the data of B8. |
| M13 | **Limits** (extends B11): there is no artificial maximum of values per key either; the physical and representation limits of B11 apply. |
| M14 | **Format `8`**: the change is incompatible (B12): an engine of format `7` does not know the multi-value type and takes the entry of a multi-value tree in the catalog as damaged, so it would misread a healthy store: List, and opening, deleting or renaming that tree, would fail with the damage class. The current format version becomes `8`, and a store of format `7` is of an earlier format even if it is empty; before 1.0 there is no migration and the data is recreated (D7). The numbers that the format stores for a multi-value tree, such as the number of values of a key and the page of its subtree, are stored in little endian, like the rest of the format (V3). |
| M15 | **Measurement** (B12, no threshold by D5): the change includes a reproducible measurement, on Windows and on Linux, on a store on disk with durable commit, with random keys and values of 16 bytes, with a fixed seed. **Loads by set size**: always 1 million values in total, distributed in keys of N values each, for N = 1, 10, 100, 1,000, 10,000, 100,000 and 1,000,000 (from 1 million keys with one value to a single key with a million). **Skewed load**: 1 million values, each assigned to a key chosen at random among 100,000 with a Zipf distribution of exponent 1, so that a few keys hoard many values and most have few. In each of these loads, in a new store, it measures separately: adding all the values in random order, interleaving keys, in writes of 1,000 values; the size of the store in bytes that its information gives (P12) after the last commit of that phase; after closing and reopening it, without emptying the system cache, checking each value at random with Contains in a single read transaction; scanning the keys of the whole tree; scanning the values of each key; and removing all the values at random with Remove value, in writes of 1,000. **Oscillating load**: in a new store, one key with just enough values to be at the promotion threshold, so that the next Add exceeds it; 100,000 times, starting with Add, Add of a new value and Remove value of that same value alternate, in writes of 1,000 operations; it measures the time and the size of the store after the last commit. Each load is run with each candidate threshold pair of M3. The report includes the operating system, the processor and the disk. Its results go in the closing report and feed the evaluation of M3. |

### Starting state

Verified on 2026-10-29 in the code:

| Aspect | Situation |
|---|---|
| Public API | Normal trees have the operations of 5.1, the scans and Remove range of 5.2 and the values of up to 256 MiB of 5.3. A tree is opened by its name and its type, and the handle is the same whatever the type; the operations of a tree receive its handle. |
| Types | The only tree type is the normal one. The different tree type class exists, but cannot be provoked because there is no second type. The list of types admits new types without breaking the code of the applications that use it. List returns the name and the type of each tree. |
| Catalog | It records each tree with its name, its type and its identity, and takes up no pages until the first tree is created (A12). |
| Keys | The empty key is not valid (B5), and the engine's trees do not store empty keys. |
| Errors | Sixteen classes, those of the starting state of 5.3. Invalid size reports name, key, value or prefix. |
| Format | The current format version is `7` (V9). A store of format `6` created with the version of 5.1 is kept as test data; none of format `7` is kept. |
| Voron (reference) | A multi-value tree stores the values of a key in a page nested inside its entry and, when they do not fit, passes them to a tree of the key's own; those trees are not recorded in the catalog. |

## Actors

- **Integrating developer**: creates and opens multi-value trees, adds, removes and checks values, counts and scans
  keys and values, and handles the errors by their class.
- **Maintainer**: runs the measurement of M15 on Windows and on Linux and reviews the evaluation of M3.

## Flow

1. The application opens a transaction and a multi-value tree with its own handle (M4).
2. In a write it adds values to its keys and removes them, or removes whole keys and intervals (M5, M6, M8).
3. In any transaction it checks whether a key has a value, counts its values and scans the keys of the tree or the
   values of a key (M5–M7).
4. The commit applies all the changes, or none (G1).

## Validations and messages

Those of the ancestors and of 5.1, 5.2 and 5.3 apply, including the failures of T11 and T12, with the precedence of
M12. This PRD adds these situations, in the library only (the tool does not open stores until module 12):

| Situation | Rule | Error class |
|---|---|---|
| A value of more than 2,048 bytes, when adding, removing, checking or as a bound of a value scan or of Remove value range | M1, M7 | Invalid size, as a value, with minimum `0` and maximum `2048` |
| A key that is empty or of more than 2,048 bytes, in any operation of this PRD that receives a key, or as a bound of a key scan or of Remove key range | B5, R5 | Invalid size, as a key |
| A prefix of more than 2,048 bytes, in a scan, Remove key range or Remove value range | M7, R5 | Invalid size, as a prefix |
| A normal tree is opened or created as multi-value, or a multi-value one as normal | B3 | Different tree type |
| An operation is made with the handle of a multi-value tree of another transaction of the same store | M4, M12 | Misuse |
| An operation is made with the handle of a multi-value tree that the same write deleted | M4, M12, A6 | Tree not found |
| An operation needs a damaged page of the tree or of a subtree (M11) | M11, G6 | Damage, with the page number |
| An operation of one type is used with the handle of a tree of another type | M4 | None: it does not compile |
| A value that is already there is added, a value or a key that is not there is removed, or the values of a key that is not there are scanned | M5–M8 | None: it returns that it did not add, that it was not there, `0` or nothing |

## Output

The module writes nothing to the screen. The store information (P12) keeps its data; the format version it reports
becomes `8` (M14). List reports the multi-value type next to the name of each multi-value tree. The values are
returned with the stored bytes, unchanged, in the order of B5, with the empty value first if it is there.

Example, for a multi-value tree with the key `rojo` and the values empty, `a` and `b`, and the key `verde` with the
value `a`:

| Operation | Result |
|---|---|
| Full key scan, forward | `rojo` with `3`; `verde` with `1` |
| Value scan of `rojo`, forward | empty, `a`, `b` |
| Value scan of `rojo`, backward | `b`, `a`, empty |
| Value scan of `rojo`, from `a` excluded | `b` |
| Number of values of `azul` | `0` |
| Add `rojo`, `a` | did not add |
| Remove value `verde`, `a` | it was there; `verde` disappears and the number of entries becomes `1` |
| Remove key `rojo` | `3` |

Example of the data of an invalid size error, for a value of 2,049 bytes in a store on disk:

| Datum | Value |
|---|---|
| Folder | the store's |
| What violates its size | value |
| Size received | `2049` |
| Minimum | `0` |
| Maximum | `2048` |

## Reuse

- The common rules of the trees module (B1–B12) and those of 5.1, 5.2 and 5.3: opening modes, catalog, errors,
  precedence, damage, identity, changes, scans with their bounds, prefix and ends, and Remove range.
- The engine's normal trees, for the subtrees (M2).
- The free space (F4–F8) for the pages that are freed (M10).
- The transactions, their snapshot and the zero-copy reads (T2, T14, T16, D11).
- The journal and the recovery (G1, G5): the changes of multi-value trees travel like any other change.
- The integrity check and the damage class (G6).
- The simulation of crashes, power cuts and failures of the tests of modules 3 to 5.3.
- The store of format `6` kept as test data, which is still rejected as an earlier format, and one of format `7`
  that the execution creates with the version of 5.3 and keeps the same way (AC-M18).

## Out of scope

- Repeated values within a key (multisets) and application-specific value comparators (B5).
- Values of more than 2,048 bytes in multi-value trees, and off-leaf values in them.
- Reading a single value of a key with an operation of its own, such as the first or the last (a value scan with a
  single step from an end gives it), an operation of its own to know whether a key exists (its number of values gives
  it, M6), and replacing all the values of a key at once.
- Scans that return several keys with their values at once, such as key–value pairs, and scans with key and value
  bounds at the same time.
- A total of values of the tree, and counting the keys or the values of an interval without scanning it.
- Adding or removing several values, or values of several keys, in a single operation.
- Set operations on the values (union, intersection, inclusion), and copying or moving the values of a key to another
  key or to another tree.
- Converting a normal tree into a multi-value one or the other way around (B3).
- Choosing or configuring the thresholds from the application, choosing the representation of the values of a key or
  knowing which one it has, and statistics of the subtrees (storage report, module 12).
- Reorganizing the existing keys if a future version changes the thresholds.
- Repairing a damaged subtree so that its key can be removed (M11; command-line tool, module 12).
- Fixed-size trees and compression of leaf pages (5.5 and 5.6); whether multi-value trees can be compressed is decided
  by 5.6.
- Migrating a store of format `7` (D7).

## Left to the implementation

- The two thresholds of M2, with the evaluation that M3 requires, and their unit (bytes, number of values or pages),
  chosen so that the values of a key that are in its entry always fit in it.
- How the values of a key are represented inside its entry and in its subtree, where its number of values is stored,
  as long as it meets M11, and how the empty value is stored in each representation. User preference: in the subtree,
  a mark in the key's entry saying that it has the empty value, so that the subtree stores no empty keys; in the
  entry, the empty value as one more value.
- Which page is reported in a damage error when an operation needs several damaged ones (M11), as long as it is one of
  those it needs, also when the operation is repeated; and which pages of a subtree an operation that frees it whole
  reads, beyond those it needs to find its own.
- Which failure of T11 is reported when a single operation would cause several, according to what it tries first, and
  what the write itself sees after that failure, since it can no longer be committed.
- The names of the handles and of the new operations (parent, *Left to the implementation*), and how each type is
  requested when opening, as long as it meets M4.
- The command that runs the measurement of M15 and the shape of its report.

## Acceptance criteria

The test conditions of the earlier modules and the header of the parent's criteria apply: each criterion runs on a
store on disk and in temporary mode, except the parts that reopen the store or damage its files, which are on disk
only. This PRD checks the parent's criteria assigned to 5.4: AC-B7 in its multi-value part (AC-M1), AC-B15 (AC-M2),
AC-B4 in its different tree type part, AC-B6 in its second part and AC-B8 with normal and multi-value (AC-M10), and
AC-B17 and AC-B18 with multi-value trees (AC-M13 and AC-M14); it repeats AC10 and AC20 of the root with multi-value
trees (AC-M15 and AC-M16), and adds these. The tests that depend on the thresholds, on the representation of the
values of a key, on damaged pages or on failures at specific points use knowledge of the implementation itself.

1. **AC-M1** — Given an empty multi-value tree, when empty values, values of 1 byte and values of 2,048 bytes are
   added, in random order, to keys of 1 and of 2,048 bytes, with bytes from `0x00` to `0xFF` and values that are a
   prefix of others, then each Add returns that it added the first time and that it did not add the second, changing
   nothing; Contains finds them and does not find those that were not added; the number of values of each key matches
   those added, and is `0` for a key that is not there; the number of entries is the number of keys; and the value
   scan of each key returns them ordered byte by unsigned byte, with the empty one first and each value that is a
   prefix of another before it, in the same write, in a later read and, on disk, after reopening (M1, M5, M6, AC-B7).
2. **AC-M2** — Given a multi-value tree, when a value of 2,049 bytes is added, removed, checked, or used as a bound of
   a value scan or of Remove value range, then it fails without effect with the invalid size class, as a value, with
   the size received `2049`, minimum `0` and maximum `2048`, and the transaction stays usable; when each operation of
   this PRD that receives a key receives it empty or of 2,049 bytes, also the number of values and Remove key, then it
   fails the same way as a key, with minimum `1` and maximum `2048`; with a key and a value of 2,049 bytes, the error
   is the key's; with a lower bound and a prefix outside their size, the lower bound's; and with a prefix of 2,049
   bytes, the prefix's, with minimum `0` and maximum `2048` (M1, M6, M7, M12, AC-B15).
3. **AC-M3** — Given a key with values, when they are removed one by one with Remove value, then each returns that it
   was there the first time and that it was not there the second; on removing the last one, the key disappears:
   Contains, the number of values (`0`), the key scan and the number of entries reflect it; and when a value is added
   again, the key reappears with a single value (M5, M6).
4. **AC-M4** — Given a key with one value, another with several values in its entry and another with values in a
   subtree, when each is removed with Remove key, then it returns its number of values (`1` for the first) and the key
   disappears; and with a key that is not there, it returns `0` without failing (M6, M2).
5. **AC-M5** — Given a key, with the two thresholds that the test takes from the implementation, when values are
   added to it, one by one, until the promotion threshold is exceeded, and then removed until it is below the demotion
   one, with the empty value among them and without it, in separate writes and in the same write, then after each
   step Contains, the number of values and the value scan, forward and backward, match the values added and not
   removed, in the same write, in a later read and, on disk, after reopening; the values pass to the subtree just on
   exceeding the promotion threshold, stay in it while they are between the two thresholds and return to the entry just
   on falling below the demotion one; with Remove value range jumping at once from above the promotion threshold to
   below the demotion one, they return to the entry in that operation; and the empty value keeps its place when
   passing from one representation to the other (M2, M8, M1).
6. **AC-M6** — Given a multi-value tree with keys with values in their entry and in a subtree, when its keys and the
   values of each key are scanned with included, excluded and open bounds, with the empty value as an included and an
   excluded bound, with prefixes (also empty and ending in `0xFF` bytes), and alternating the two ends, then each scan
   returns exactly what R1 to R4 say applied to the keys or to the values, each key with its number of values; the
   value scan of a key that is not there, and the key scan of an empty tree or one left with no keys, return nothing;
   with inverted bounds, the interval is empty, also with the page of the key's entry damaged (on disk); and when a
   write modifies the tree through another handle, continuing to use an earlier scan does not compile (M7, R1–R4, R6).
7. **AC-M7** — Given a multi-value tree, when intervals of values are removed from a key in its entry and in a
   subtree, also all those of the key, then each Remove value range returns how many it removed, and the key that is
   left with no values disappears; and when intervals of keys are removed, also with keys with a subtree, then it
   returns how many keys it removed, the number of entries goes down by that amount and a key scan returns the others
   with their number of values (M8, M6).
8. **AC-M8** — Given a store with a multi-value tree already created, which stays within a single region of the free
   space, taking the pages in use as in AC-A15: when an empty multi-value tree is created, committed, the value is
   taken, keys with values in their entry and in subtrees are added to it and committed, and then all the keys are
   removed with each of these forms in separate tests (Remove value, Remove key, Remove value range, Remove key range
   and emptying it), then the final value is the one taken with the empty tree; when values are removed from a key
   with a subtree until it returns to its entry and committed, then the pages in use are the same as with a tree to
   which those same values were added without the key ever reaching the subtree (the tests prepare it knowing the
   implementation); and when the tree is deleted or overwritten with Rename, the final value is the one from before
   creating it when its entry did not make the catalog grow, as in AC-A15 (M10, A2, A3).
9. **AC-M9** — Given the code of a test application, when it uses Store, Add, Read or Remove of 5.1, a scan or Remove
   range of 5.2 with the handle of a multi-value tree, or an operation of this PRD with the one of a normal tree, or
   Add, Remove value, Remove key or Remove range in a read transaction, or uses a reference or a scan after one of the
   operations of M9 that failed without effect, or moves to another thread the handle of a multi-value tree of a write
   or a scan of it, then it does not compile; and moving to another thread that of a read and its scans compiles; and
   Contains, the number of values and the scans do not end the earlier references or scans (M4, M9, B1, R9, AC-B11).
10. **AC-M10** — Given a normal tree and a multi-value one, when they are opened or created by name as the other type,
    in each mode of A1, then it fails with the different tree type class, with the name, the type it has and the type
    requested, and the tree is left intact; it fails the same way with a page of the tree's own damaged and the catalog
    intact; and deleting it without requesting a type never fails with that class; given a write with changes made
    before and after an operation that fails by different tree type, when it is committed, then it ends without error
    and applies all those changes; given a read transaction, when an open fails by different tree type, then it keeps
    reading; given a store that does not accept writes (T12), when an open fails by different tree type, then it fails
    with that class and not with the input and output one, and committing that write without changes ends without
    error; given a normal tree that is deleted, when another is created with its name as multi-value, then it is
    created; List reports each tree with its type; and Rename with overwrite deletes the destination tree whatever its
    type, also a multi-value one with subtrees (M4, M12, AC-B4, AC-B6, AC-B8, A3).
11. **AC-M11** — Given a write with a multi-value tree opened twice, and another transaction of the same store, when it
    deletes it through one handle and uses the other with each operation of this PRD, also after creating another tree
    with the same name, then it fails without effect with tree not found and the tree's last name; when it uses the
    handle in the other transaction, then it fails without effect with misuse; and with, in addition, an argument
    outside its size, in both cases, the error is invalid size (M4, M12, A6, A7, AC-B14).
12. **AC-M12** — Given a multi-value tree on disk, in separate tests, with the root page of the subtree of a key
    damaged (a subtree of at least two levels) and with a damaged page in the part of the tree that leads to another
    key, when a value is added, removed or checked, also one that is already there or that is not there, or the values
    of those keys are scanned, then each operation that needs the page fails with the damage class and its number, and
    fails the same way again when repeated; the value scan ends after the failure, its earlier values are correct and
    in a read a new scan fails the same way again; the other keys are read and modified without error; the number of
    values of the key with the damaged subtree and the key scan do not fail; and when a write removes the key with the
    damaged subtree with Remove key, removes values from it until it returns to its entry, removes a key interval that
    contains it, empties the tree, deletes it or overwrites it with Rename, then it fails with the damage class and
    the number of a page of the subtree, and the write follows T11 (M11, A9, R8, G6, F9).
13. **AC-M13** — Given an open read transaction and a write that, with Add and Remove value, makes keys pass from their
    entry to a subtree and back, and splits and merges pages of the tree and of the subtrees until their height
    changes, when the read reads all the keys, their values, their number of values and the number of entries before
    and after the write commits, then it sees exactly those of its snapshot; and a new read sees those of the write
    (G3, T14, AC-B17).
14. **AC-M14** — Given a write that adds and removes values, making keys pass from one representation to the other and
    splitting and merging pages, when a process crash and a simulated power cut are caused at every point where its
    commit writes or carries data to disk, then on reopening the tree is whole as before the write or whole as after
    it: each key with its values and its number of values, and the number of entries; and when a T11 failure other
    than input and output is simulated at every point where an operation reserves, reads or frees pages, also when the
    values of a key pass to a subtree and back, then committing fails with the class of the first failure and applies
    nothing, and after aborting the write the tree, its keys, its values and the number of entries are as before and a
    new write works without error (G1, G5, T11, AC-B18).
15. **AC-M15** — Given a small store with a multi-value tree with keys with values in their entry and in a subtree,
    when each of its bytes is altered separately, it is opened and all its values are read, then each alteration is
    detected as damage or has no effect, with the rules of AC10 of the root and its annotations; a value is never
    returned with a different byte (M11, G6, AC10).
16. **AC-M16** — Given the keys and values that the key scan and the value scan return, from keys with values in their
    entry and in a subtree, when they are read, then the references are inside the engine memory that T16 fixes and
    not in a copy, on disk and in temporary mode, and using a reference after the transaction ends does not compile
    (M7, AC20).
17. **AC-M17** — Given a write on disk that only does Add of values that are already there, Remove value of values that
    are not there, Remove key of keys that are not there, Remove key range and Remove value range that remove nothing,
    Contains, the number of values, scans, and operations that fail by invalid size or by different tree type, when it
    is committed, then it ends without error and the store's files are left identical byte for byte (M9, T10).
18. **AC-M18** — Given a store of format `7` that the execution creates with the version of 5.3, compiled separately,
    and keeps as test data, and the one of format `6` of 5.1, when they are opened, then they fail with the different
    format class as an earlier format and are not modified; a new store reports the format version `8`; and reading on
    disk the bytes of the entry of a key with a subtree, with knowledge of the implementation, its number of values and
    the page of its subtree are in little endian (M14, AC-B12).
19. **AC-M19** — Given the closing report, when it is reviewed, then it contains the measurement of M15 with its times,
    by phase, and the store sizes for each load and each candidate threshold pair, and the machine data, on Windows and
    on Linux (M15).
20. **AC-M20** — Given the closing report, when it is reviewed, then it contains the evaluation of M3 with at least the
    six candidate threshold pairs it names, the measurement data of each one, the complexity that each adds, the chosen
    one and why, justified with that data (M3).

## Annotations to other PRDs

This PRD changes or refines rules of earlier documents, which carry the corresponding annotations:

- Trees module, B6: in multi-value trees, each value is from 0 to 2,048 bytes (M1).
- Trees module, B3: the subtrees of the keys of a multi-value tree are not catalog trees (M2); each type is opened
  with its own handle, and using an operation of one type with a tree of another does not compile (M4).
- Trees module, B11: there is no artificial maximum of values per key either (M13).
- Trees module, *Left to the implementation*: the promotion and demotion thresholds of multi-value trees remain left to
  the implementation, with the measured evaluation of M3.
- Trees and Key Operations, A1: each type is opened with its own handle, with the same modes (M4).
- Trees and Key Operations, A5: in a multi-value tree, the number of entries is its number of keys (M6).
- Trees and Key Operations, A9; In-Order Scan, R8: in a multi-value tree, the operations that free a whole subtree can
  fail with damage in any page of it, and Remove key range also needs the pages of the subtrees it frees (M11).
- Trees and Key Operations, A10; In-Order Scan, R6 and R7: Add, Remove value, Remove key and the two Remove range end
  the references and the scans, and are changes for T10 only when they add or remove something; the catalog operations
  and the opening modes do not change, and emptying is still always a change (M9).
- In-Order Scan, *Out of scope*: the scans and Remove range of multi-value trees are set by this PRD (M7, M8).
- Large Values, *Out of scope*: multi-value trees have no off-leaf values; each value is from 0 to 2,048 bytes (M1).
- Root parent, D7; Pager, P5; Journal, J3; grouped writing, E15; Free space, F11; Transactions, the whole document;
  Trees and Key Operations, A13 and its 5.3 annotation; In-Order Scan, R10, *Output* and AC-R14; Large Values, V9,
  *Output* and AC-V12: the current format version becomes `8`, and a store of format `7` is of an earlier format
  (M14).

## References

- Parent: [trees.prd.en.md](trees.prd.en.md) v1.0; root: [krummi.prd.en.md](krummi.prd.en.md) v1.1
- Siblings: [trees-keys.prd.en.md](trees-keys.prd.en.md) v1.0, [trees-scan.prd.en.md](trees-scan.prd.en.md) v1.0,
  [trees-large-values.prd.en.md](trees-large-values.prd.en.md) v1.0
- Related: [free-space.prd.en.md](free-space.prd.en.md), [transactions.prd.en.md](transactions.prd.en.md),
  [pager.prd.en.md](pager.prd.en.md)
- Voron, multi-value trees: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron/Data/BTrees>

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| a3b768ea | 2026-10-29 08:33 UTC+00:00 | 1 h 26 min | 10 min | 1 h 16 min | 39 min | 37 min | 16 |

<!-- prd-time: [{"session": "a3b768ea-ddde-4ebc-a01d-ce4fd902416e", "start": "2026-10-29T08:33:25.283000+00:00", "wall": 5150, "discarded": 608, "agent": 2317, "human": 2224, "interventions": 16, "idle_minutes": 10}] -->
