# Krummi B+ Trees — PRD

**Version**: 1.0
**Date**: 2026-10-24
**Parent document**: [Krummi, an embedded storage engine inspired by Voron — PRD](krummi.prd.en.md) (v1.1)

## Objective and context

Build the **B+ trees**, module 5 of the [parent's split](krummi.prd.en.md#split-into-child-prds): named trees that
store entries ordered by key, with variable-size keys and values, large values, in-order traversal, prefix search,
multi-value trees, fixed-size trees and leaf page compression.

It is the first data structure of the product and the first way applications have to store data in a store: until
today, reading and writing pages is internal to the library (T1, F1). Tables (module 7) and the later structures are
built on these trees.

The module is too large for a single execution, so this document is the umbrella of the module, as the parent is of
the product: it sets the rules common to all trees (types, names, keys and their order, maximum sizes, errors and
trees open at the same time) and splits the module into six child PRDs, each with its own execution and its own
closing (see [Split into child PRDs](#split-into-child-prds)). This document has no execution of its own (B2).

**Why now**: free space (module 4) already allows reserving and freeing pages and ranges of contiguous pages, which
is what the trees need to grow, shrink and store large values.

This PRD inherits everything from the parent, from the [Pager](pager.prd.en.md) (P1–P17), from
[Transactions](transactions.prd.en.md) (T1–T18), from the [Journal](journal.prd.en.md) (J1–J17), from the
[group and direct journal writes](journal-group-commit.prd.en.md) (E1–E16) and from
[Free Space](free-space.prd.en.md) (F1–F14); here only what is common to the module is added. What changes or
refines earlier rules is declared in its decisions and annotated in the affected documents (see
[Annotations to other PRDs](#annotations-to-other-prds)).

In this document, a **tree** is a named collection of entries ordered by key inside a store; an **entry** is a key
with its value, or with its values in a multi-value tree; the **catalog** is the record of the trees of a store with
their name and their type; and the **type** of a tree is normal, multi-value or fixed-size.

### Product decisions

| ID | Decision |
|---|---|
| B1 | **Public surface**: from this module on, applications store and read data in trees. Inside a transaction a tree is opened by its name; it is read in any transaction and modified only in a write one. Modifying a tree opened in a read transaction does not compile, and neither does using a tree after the transaction in which it was opened has ended (as T8). A tree opened in a read transaction can be passed to other threads and shared with them like the transaction, and one opened in a write transaction stays in its thread, like it (T6). Each child PRD sets the operations it adds. |
| B2 | **Split and closing**: the module is split into the six child PRDs of the [table](#split-into-child-prds), which are built in that order. This document has no execution of its own: each common rule is implemented and tested by the first child PRD that needs it, and the following ones keep complying with it. Module 5 is closed when its six child PRDs are closed; until then it does not count as closed for objective 2 of the parent. |
| B3 | **Tree types**: a tree is **normal** (each key has one value), **multi-value** (each key has an ordered set of distinct values) or **fixed-size** (integer keys, B5, and values of the fixed length that child PRD 5.5 sets). The type is set when the tree is created and does not change while the tree exists; deleting a tree and creating another with the same name and another type is allowed. Leaf page compression (5.6) is not a type. All types share the catalog and its namespace: two trees of a store never have the same name. Requesting an existing tree as a type that is not its own, when opening it or when creating it, fails with the new class **different tree type** (B7), which wins over any existing-tree error that 5.1 sets and over damage in the tree's own pages. Operations that name a tree without requesting a type (such as deleting it) never fail with different tree type. Child PRD 5.1 creates the catalog with the normal trees; 5.4 and 5.5 add their types. |
| B4 | **Tree names**: a name is UTF-8 text of 1 to 1,024 bytes, counted in its UTF-8 encoding; the library receives names as text, so a name that is not valid UTF-8 cannot be passed. Two names are the same only if their bytes are equal: upper case, lower case and Unicode forms are not normalized, so `pedidos` and `Pedidos` are two different trees. No name is reserved. A name outside those sizes fails with the new class **invalid size** (B7). |
| B5 | **Keys and order**: in normal and multi-value trees, a key is a sequence of 1 to 2,048 bytes; the empty key is not valid. Voron supports 1,024; Krummi supports 2,048 so that the keys of multi-field secondary indexes fit (module 7). Every key that an operation of those trees receives is validated the same way, when storing, reading and deleting; the prefix and the key a traversal starts from are set by child PRD 5.2. Keys are ordered byte by byte, comparing each byte as an unsigned number from 0 to 255, and a key that is a prefix of another goes before it. There are no application comparators. The same order holds for the values of a key in a multi-value tree and for the names of the catalog. In fixed-size trees, a key is a 64-bit unsigned integer, from 0 to 2^64−1, in numeric order. A key of a normal or multi-value tree outside its sizes fails with invalid size (B7). |
| B6 | **Values**: a value is a sequence of 0 to 268,435,456 bytes (256 MiB); the empty value is valid. Until child PRD 5.3 (large values), the maximum is a fixed number that child PRD 5.1 sets, the same for any valid key, including one of 2,048 bytes; 5.3 raises it to 256 MiB, also for any valid key. Larger values are a matter for streams (module 8). Child PRDs 5.4 and 5.5 may set smaller maximums, or in 5.5 an exact length, for their types. Every value that an operation receives is validated the same way, including the one passed to look it up or delete it in a multi-value tree. A value outside the sizes in force for its tree fails with invalid size (B7). |
| B7 | **New errors** (D8 of the parent): **invalid size**, for a key, a value or a name outside its sizes (B4–B6), and **different tree type** (B3). Both are detected before the operation changes anything: the operation fails without effect and the transaction remains usable, whether it is a read or a write one, also when the store does not accept writes (T12). They are public validation errors and are not among the failures of T11, so they do not prevent committing: a write keeps its changes and can be committed or aborted. Only the public validation errors of this module have this rule: these two classes and the one of opening a deleted tree (B9). The failures of T11, and the internal misuses that invalidate the transaction according to the [free space validations](free-space.prd.en.md#validations-and-messages), still invalidate it. If a write also had a failure that invalidates it, before or after, committing it fails with the class of the first of those failures, never with one of these two. A write whose only operations failed with these classes is a write without changes (T10). **Precedence**: invalid size is decided only with the arguments of the operation and the properties of the already open tree, and wins over everything else; if several arguments are outside their size, the first one in this order is reported: name, key, value. Next wins the first failure that the operation finds when reading the catalog (damage, out of memory, input/output), and then different tree type. |
| B8 | **Data of the new errors** (D8 of the parent): both carry the data of T18 (the store folder, or that it is temporary). The invalid size one also carries what breaks its size (name, key or value), the size received in bytes and the minimum and maximum sizes in force for that tree: `1` and `1024` for a name, `1` and `2048` for a key, and for a value the sizes in force for its tree (B6): `0` and the maximum, or the exact length as both minimum and maximum in a fixed-size tree. The different tree type one also carries the name of the tree, the type it has and the type requested. |
| B9 | **Several trees open**: a transaction can have open at the same time as many trees as it wants, also the same one twice, and all see the same state: in a write, with all the changes made so far through any of them. A reference read from any tree stops being valid at the next operation that can modify the transaction, in that tree or in another, even if it fails without effect (T8): the compiler checks it without knowing whether the operation will fail. Opening an existing tree does not end references; creating or deleting a tree does. Each handle is bound to the identity of the tree it opened, which is distinct from its name: deleting a tree and creating another with the same name, any number of times and with the same type or another, always creates a new tree; child PRD 5.1 sets how this is guaranteed. If a write deletes a tree, operations through any other handle of it fail without effect with a public validation error that 5.1 sets, with the rule of B7, also when another tree with the same name is created afterwards: an old handle never touches the new tree. |
| B10 | **Reads**: D11 of the parent, T16 and AC20 hold for the keys and values that the trees return. Child PRD 5.3 sets how a large value is returned and 5.6 where the decompressed copy of a leaf page lives, within D11. |
| B11 | **Limits**: Krummi imposes no artificial limits on the number of trees or entries. That does not remove the physical limits, the maximum size of the store (P10), memory (P13) and disk (G7), nor the representation limits of the numbers with which the format counts pages and entries, which the format chooses so that they are not reached before the physical ones; if an operation reached one, it would fail without effect with the maximum size class and would follow T11. An operation with valid sizes that does not fit in the physical limits fails with its class and follows T11. The store information (P12) adds no data in this module. |
| B12 | **Format and measurement**: the format version goes up by exactly 1 with each incompatible change of the format (D7 of the parent), not for adding persistent data. A change is incompatible when an engine of the previous version could misread, modify without respecting, or damage a store with the new format, or when the new engine cannot safely open one of the previous version; when in doubt, it is treated as incompatible. Each child PRD that changes what is stored on disk declares whether its change is incompatible and why; if it is, a store of the previous version, even if empty, is of an earlier format, as in F11. Each child PRD decides whether its change includes a reproducible measurement, like F13, with no threshold (D5 of the parent). |

*Annotations of 2026-10-25 from trees and key operations ([trees-keys.prd.en.md](trees-keys.prd.en.md)):*

- *B7: the public validation errors of the module also include tree not found and tree already exists, with
  the precedence of A7 (decisions A6 and A7).*
- *B9: the error of a handle of a deleted tree is tree not found, with the last name the tree had (decision
  A6).*
- *B10: listing the trees returns a copy owned by the application, not references (decision A3).*
- *B1 (exception, decided before executing 5.1): while transactions are obtained as values (T1) and a write has
  several handles at once (B9), the compiler cannot prevent a handle from being used in another transaction of the
  same store, also after its own has ended. That operation fails without effect with the misuse class and the
  transaction stays usable. A handle outliving its store still does not compile.*
- *B9: opening a tree in Open or create mode ends the references even if the tree exists, because the compiler does
  not know whether it will create it (decision A10).*
- *AC-B18: the part about failures during a structure change simulates a T11 failure at each point where the
  operation reserves, reads or frees a page, also after several pages have been modified, in splits and merges that
  propagate through several levels. Committing then fails with the class of the first failure and applies nothing,
  and after aborting, or after that failed commit, the tree, the catalog and the number of entries are as before and
  a new write reads and modifies them without error. The tests use knowledge of the implementation to reach each
  point.*

### Starting state

Verified on 2026-10-24 in the code and in the Voron reference collected by the parent:

| Aspect | Situation |
|---|---|
| Public API | The library opens, queries and closes stores, and opens, commits and aborts transactions. There is no public way to store or read data: reading and modifying pages, reserving and freeing them is internal (T1, F1). |
| Pages | Pages of 8,192 bytes, with 8,176 usable after their integrity check and their number. |
| Free space | A write reserves a page or a range of contiguous pages and frees loose pages or ranges. A range does not cross from one region to another: the cap is 65,407 pages, about 511 MiB (F5). |
| Errors | Twelve classes: access, store in use, not a store, damage, different format, disk full, input/output, invalid options, maximum size, out of memory, timeout and misuse. None deals with keys, values or trees. |
| Format | On this date, the format version in force is `5` (F11). There is no tree catalog or data structure on disk. |
| Voron (reference) | B+ trees with keys of up to 1,024 bytes, large values in separate pages, multi-value trees, leaf page compression and fixed-size trees with signed 64-bit integer keys, all named inside the store (starting state of the parent). |

## Actors

- **Integrating developer**: creates and opens trees by their name, stores, reads and deletes entries inside
  transactions, and handles the invalid size and different tree type errors by their class.
- **Maintainer**: runs on Windows and on Linux the tests and the measurements that each child PRD sets.

The command-line tool does not open stores until module 12, which sets how it inspects the trees.

## Flow

1. The application opens a transaction and, inside it, one or several trees by their name and their type (B1, B3,
   B9).
2. In a read transaction it reads entries; in a write one it also creates and deletes trees, and stores and deletes
   entries, with the operations that each child PRD sets.
3. Each name, key and value is validated against its sizes before anything changes (B4–B7); a size or type error
   leaves the transaction usable.
4. The commit applies all the changes of the trees and of the catalog, or none (G1).

## Split into child PRDs

Each submodule is a child PRD of this one, with its own execution. The order is the build order: each one builds on
the previous ones. The *Sets* column names what that child PRD must decide, besides its content and its own errors.

| # | Child PRD | Content | Sets |
|---|---|---|---|
| 5.1 | **Trees and operations by key** | Catalog of named trees; normal trees; store, read and delete an entry by its key; values up to the provisional maximum (B6). | The catalog operations (create, open, delete and, if there are any, rename and list trees); what happens when opening a tree that does not exist in a read and when creating one that already exists with the same type; what storing a key that already exists does; what reading a key that is not there returns and what deleting it does; the error and its data for a handle of a deleted tree, and how a recreated tree is told apart from the previous one (B9); whether damage in the pages of a tree is reported when opening it or on its first read; whether a tree reports its number of entries; the provisional maximum of a value (B6); whether its format change is incompatible and, if it is, its version (B12); whether a new store takes up more pages than before, updating the earlier criteria that set it. |
| 5.2 | **In-order traversal and prefix search** | Traverse the entries of a tree in order, forwards and backwards, from a given key; limit the traversal to the keys with a prefix. | The valid sizes of the prefix and of the start key, and what an empty prefix means; where a traversal starts when the start key does not exist; what a traversal sees when the write that makes it modifies the tree; whether there is deletion of a range of keys. |
| 5.3 | **Large values** | Values of up to 256 MiB (B6). | How a large value is returned without a copy (B10), also in temporary mode, where memory grows in separate chunks. |
| 5.4 | **Multi-value trees** | Trees whose keys have an ordered set of distinct values (B3). | The operations on the values of a key; the maximum of each value and whether the empty value is valid; what happens when adding a value that is already there. |
| 5.5 | **Fixed-size trees** | Trees with 64-bit unsigned integer keys (B5) and fixed-length values (B3). | When the length of the values is set and what the maximum is; with what class and data an invalid length fails when creating the tree; whether opening a tree with a length different from its own is a different tree type or an error of its own, and its data; which operations of 5.1 and 5.2 they have. |
| 5.6 | **Leaf page compression** | Trees whose leaf pages are stored compressed. | How compressing a tree is chosen, for which types, and whether it can be changed afterwards; where the decompressed copy of a leaf page lives (B10). |

**Before implementing 5.1**: its execution starts with a compilation test that shows that Rust's ownership and
borrowing model allows B1 and B9 together: several tree handles in a write, also of the same tree, with the changes
made through one visible through the others; references that stop being valid at the next modifying operation; and
handles that are not used outside their transaction. All without unsafe code outside the Pager (D12 of the parent).
If any of those guarantees cannot be given at compile time, the execution stops before implementing the trees and
the alternative comes back to this PRD as a product decision.

## Validations and messages

Those of the ancestors apply, including the failures of T11 and T12 in any tree operation. This module adds these
situations, library only (the tool does not open stores until module 12). None follows T11: the operation fails
without effect and the transaction remains usable (B7).

| Situation | Rule | Error class |
|---|---|---|
| A tree name that is empty or longer than 1,024 bytes in UTF-8 | The operation fails without effect (B4). | Invalid size (new) |
| A key of a normal or multi-value tree that is empty or longer than 2,048 bytes, when storing, reading or deleting | The operation fails without effect (B5). | Invalid size (new) |
| A value outside the sizes in force for its tree (B6) | The operation fails without effect. | Invalid size (new) |
| An existing tree is opened or created with a type that is not its own | The operation fails without effect (B3). | Different tree type (new) |
| An operation goes through a handle of a tree that the same write deleted | The operation fails without effect (B9). | The one that 5.1 sets |

The precedence between these situations and the other failures is that of B7. Each child PRD adds the situations of
its operations.

## Output

The module writes nothing to the screen. The data that a read returns are the stored bytes, unchanged. The store
information (P12) adds no data (B11); the format version it reports is the one set by the last child PRD that
changes it (B12). Example of the data of an invalid size error, for a key of 2,049 bytes in a store on disk:

| Datum | Value |
|---|---|
| Folder | the store's |
| What breaks its size | key |
| Size received | `2049` |
| Minimum | `1` |
| Maximum | `2048` |

With an empty key, the size received is `0` and the minimum and the maximum are the same.

## Reuse

- Transactions, their snapshot isolation and the copy of the pages that are modified (T2, T14, T16): trees are read
  and modified through them.
- The journal, recovery and the atomicity of the commit (G1, G5, J8): the changes of the trees and of the catalog
  travel like any other change.
- Free space (F4–F8): trees reserve and free pages and ranges through it.
- The integrity check of the pages and the damage class (G6).
- The misuse error class (T5) for what the child PRDs treat as an unsupported call.
- The simulation of crashes, power cuts and failures of the tests of modules 3 and 4.

## Out of scope

- Application comparators of keys, language-dependent orderings and tree names that are not case-sensitive (B4, B5).
- Signed integer keys in fixed-size trees (B5).
- Values larger than 256 MiB, and reading or writing a value in parts (streams, module 8).
- Changing the type of an existing tree (B3).
- Combined read and write operations on an entry, such as incrementing a counter or storing only if the entry has a
  given version.
- Statistics of a tree, such as its depth or its pages (storage report, module 12), except its number of entries if
  5.1 decides to report it.
- Tables with schema, secondary indexes and dictionary value compression (module 7).
- Trees with compressed keys (compact trees, module 15), containers, posting lists and lookups (modules 13, 14 and
  16).
- Several writes at the same time on different trees of the same store (G4).
- Inspecting, verifying or rescuing trees from the command-line tool (module 12).
- Encrypting the pages of the trees: covered by the encryption of everything the engine writes (module 9).
- Performance thresholds (module 6).

## Left to the implementation

- The organization of the pages of the trees and of the catalog, from what size a value is stored outside its leaf
  page, the page split and merge policy and their fill factor, as long as they meet the parent's guarantees.
- The names of the public operations, of the types and of the new error classes, which follow Rust conventions and
  are in English, and the English text of the errors (D8 of the parent).

## Acceptance criteria

The test conditions of the earlier modules apply, and each child PRD repeats AC10 of the parent on disk and AC20
also in temporary mode with what it adds, as those criteria say. The tests that damage pages of a tree or of the
catalog use the knowledge of the implementation itself. Each criterion of this list is checked by the child PRD
indicated in parentheses, on a store on disk and in temporary mode, except the parts that reopen the store or damage
its files, which are on disk only; the following child PRDs keep complying with it. The different tree type criteria
cannot be checked until a second type exists (5.4).

1. **AC-B1** — Given a write, when it stores entries with keys of 1, 1,025 and 2,048 bytes, each with a value of the
   maximum in force, then they are stored and read back identical; and when it tries to store, read or delete with
   an empty key or one of 2,049 bytes, then it fails with the invalid size class without effect, with the data of
   B8: key, size received `0` or `2049`, minimum `1` and maximum `2048` (5.1).
2. **AC-B2** — Given a write, when it creates trees with names of 1 byte and of 1,024 bytes, including one of 1,024
   bytes made of multi-byte characters, then they are created; and when it tries to create one with an empty name,
   one of 1,025 bytes or one of 600 two-byte characters (1,200 bytes), then it fails with the invalid size class
   without creating anything, with the size received in bytes (5.1).
3. **AC-B3** — Given a write, when it stores an empty value and another of the maximum in force, both with a key of
   2,048 bytes, then they are read back identical after committing and, on disk, also after reopening; and when it
   tries to store one of one byte more than the maximum, then it fails with the invalid size class without storing
   anything, with minimum `0` and the maximum in force (5.1 with its provisional maximum; 5.3 with 256 MiB).
4. **AC-B4** — Given a write with changes already made, when one operation fails by invalid size and another by
   different tree type, then the write is committed without error and applies all its changes before and after those
   failures; given a read transaction, when an operation fails by invalid size, then it keeps reading; given a write
   on disk whose only operations failed with these classes, when it is committed, then the files of the store are
   left identical byte for byte (T10); and given a store that does not accept writes (T12), when an operation fails
   by invalid size, then it fails with that class and not with the input/output one, and committing that write
   without changes ends without error (5.1 the size; 5.4 the type).
5. **AC-B5** — Given a write that already had a maximum size failure (P10), when a later operation fails by invalid
   size and it is then committed, then the commit fails with the maximum size class and applies nothing; and given a
   write that first had an invalid size failure and then a maximum size one, when it is committed, then it fails the
   same way with the maximum size class (B7) (5.1).
6. **AC-B6** — Given the trees `pedidos` and `Pedidos`, and two trees whose names are a precomposed `é` and `e`
   followed by the combining accent, when entries are stored in each one, then they are four different trees and
   each has only its own entries (5.1); and given a normal tree that is deleted, when another is created with its
   name as multi-value, then it is created (5.4).
7. **AC-B7** — Given keys stored in random order, including keys with bytes from `0x00` to `0xFF` and keys that are
   a prefix of others, when the tree is traversed in order, then they come out ordered byte by byte unsigned, and
   each key that is a prefix of another goes before it (5.2); the values of a multi-value key come out in that same
   order (5.4); and the keys `0`, `1`, `2^63` and `2^64−1` of a fixed-size tree come out in numeric order (5.5).
8. **AC-B8** — Given a tree of one type, when it is opened or created with its name as another type, then it fails
   with the different tree type class, with the name, the type it has and the type requested, and the tree is left
   intact; it fails the same way with one of the tree's own pages damaged and the catalog intact; and deleting it
   without requesting a type never fails with that class, although it may fail with the damage one if it reads the
   damaged page (5.4 with normal and multi-value; 5.5 with fixed-size).
9. **AC-B9** — Given a store on disk whose catalog fits in one page and that page is damaged, when a tree is opened
   with a name of 1,025 bytes, then it fails with the invalid size class; and with a valid name, with the damage
   class (B7) (5.1).
10. **AC-B10** — Given a write with two trees open, and one of them open twice, when it stores entries in each, then
    each handle sees the changes made through the others, and the commit applies all of them (B9) (5.1).
11. **AC-B11** — Given the code of a test application, when it modifies a tree opened in a read transaction, uses a
    tree after its transaction has ended or uses a reference read after a modifying operation that failed without
    effect, then it does not compile (B1, B9) (5.1).
12. **AC-B12** — Given a child PRD that declares its format change incompatible, when a store of the previous
    version is opened, even if empty, then it fails with the different format class as an earlier format; and given
    one that declares it compatible, when a store created before that change is opened, then it opens and is used
    without error (B12) (each child PRD that changes what is stored on disk, from 5.1).
13. **AC-B13** — Given an operation with a name and a key outside their sizes, when it is executed, then the invalid
    size error is the name's; and with a key and a value outside their sizes, the key's (B7) (5.1).
14. **AC-B14** — Given a write with a tree open twice, when it deletes it through one handle and then uses the other,
    also after creating another tree with the same name, then the operation fails without effect with the error that
    5.1 sets, the new tree does not change and the write is committed without error; repeating three times deleting
    and creating the tree, with the same type and with another, each earlier handle fails the same way and only the
    last one sees the current tree; and a read transaction opened before that write keeps seeing the original tree
    through its handle, also after the write is committed (B9, G3) (5.1).
15. **AC-B15** — Given a multi-value tree, when a value above the maximum of its values is looked up or deleted,
    then it fails with the invalid size class without effect (B6) (5.4).
16. **AC-B16** — Given a fixed-size tree, when a value of a length different from its own is stored, then it fails
    with the invalid size class, with that length as minimum and as maximum (B8) (5.5).
17. **AC-B17** — Given an open read transaction and a write that, with inserts and deletes, splits and merges pages
    of a tree until its height changes, when the read reads all the entries of the tree before and after the write is
    committed, then it sees exactly those of its snapshot; and a new read sees those of the write (G3, T14) (5.1, and
    each child PRD whose operations change the structure of a tree).
18. **AC-B18** — Given a write that splits and merges pages of a tree, when a process crash and a simulated power cut
    are caused at each point where its commit writes or flushes data to disk, then on reopening the tree has all its
    entries from before or all from after, and it can be read and modified without error (G1, G5, AC4 of the
    parent); and when an operation fails in the middle of a split because of a full disk or the maximum size, then
    the write can no longer be committed and, after aborting it, the tree is as it was (T11) (5.1, and each child PRD
    whose operations change the structure of a tree).

## Annotations to other PRDs

This PRD changes or refines rules of earlier documents, which carry the corresponding annotations:

- Root parent, split into child PRDs, module 5: it is split into six child PRDs and is closed when the six are
  closed (B2); the maximum sizes and their errors that the parent assigns to it are in B4–B8.
- Transactions, T10: a write whose only operations failed by invalid size, different tree type or opening a deleted
  tree is a write without changes (B7, B9).
- Transactions, T11: the invalid size and different tree type failures are not among its failures and do not
  prevent committing (B7).
- Transactions, T8: an operation that can modify the transaction ends the references even if it fails without
  effect; opening an existing tree does not end them (B9).

## References

- Parent: [krummi.prd.en.md](krummi.prd.en.md) v1.1 (root)
- Related: [pager.prd.en.md](pager.prd.en.md), [transactions.prd.en.md](transactions.prd.en.md),
  [journal.prd.en.md](journal.prd.en.md), [journal-group-commit.prd.en.md](journal-group-commit.prd.en.md),
  [free-space.prd.en.md](free-space.prd.en.md)
- Voron, trees: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron/Data/BTrees>
- Voron, fixed-size trees: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron/Data/Fixed>
- LMDB: <https://github.com/LMDB/lmdb/blob/mdb.master/libraries/liblmdb/mdb.c>

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| 31c36231 | 2026-10-24 10:22 UTC+01:00 | 36 min | 0 min | 36 min | 19 min | 17 min | 15 |

<!-- prd-time: [{"session": "31c36231-b691-4c62-8559-6f87e09084cf", "start": "2026-10-24T09:22:37.952000+00:00", "wall": 2181, "discarded": 0, "agent": 1146, "human": 1035, "interventions": 15, "idle_minutes": 10}] -->
