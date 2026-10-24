# Krummi Transactions — PRD

**Version**: 1.1
**Date**: 2026-10-22
**Parent document**: [Krummi, an embedded storage engine inspired by Voron — PRD](krummi.prd.en.md) (v1.1)

> English translation of [transactions.prd.md](transactions.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Build the **transactions**, module 2 of the [parent's split](krummi.prd.en.md#split-into-child-prds):
read and write transactions over an open store, with snapshot isolation (G3), a single writer (G4) and concurrent
readers that do not block with it.

When this module is closed, the integrating developer can open read transactions from several threads and one
write transaction at a time, commit them and abort them. They cannot store data in them yet: the data structures
arrive with the B+ trees (module 5). This module fixes the frame that all of them will use: who waits for whom,
what each transaction sees and how long what it reads lives.

**Why now**: all the following modules read and write inside a transaction, and the parent leaves here what G4
does not decide (waiting order and limits, a second write from the same thread), whether a transaction can move
from one thread to another, the limit of readers and what happens when a store is closed with open transactions.

This PRD inherits everything from the parent and from the [Pager](pager.prd.en.md) (P1–P17); only what is
specific to the module is added here. The temporary exception to guarantees of the parent and the clarification of
D11 are declared in T9 and T16 and annotated in it (see [Annotations to the parent](#annotations-to-the-parent)).

### Product decisions

| ID | Decision |
|---|---|
| T1 | **Public surface of the module**: open a read transaction, open a write one (with an optional waiting limit, T4), commit a write one and abort it. Finishing a read transaction is ceasing to use it. Reading, reserving and writing pages inside a transaction is internal to the library, as in P1 of the Pager: the tests and the following modules use it, not the applications. |
| T2 | **What each transaction sees**: a read transaction sees what was committed up to the moment it is opened (G3). A write transaction sees what was committed up to the moment it obtains its turn, not when it asked for it, plus its own changes. A commit is visible to every transaction that is opened after the commit returns, including those of the same thread; a read that is opened while a commit is in progress sees all the changes of that commit or none. A thread with an open read can ask for a write on the same store; its read keeps its snapshot. |
| T3 | **A single writer and strict arrival order** (G4): while a write transaction is open or any request is waiting, new requests go to the end of the queue and are served in the order in which they arrived; none overtakes another, not even at the instant a write ends. A request that stops waiting (because of its limit, T4, or because of T12) leaves the queue without changing the order of the others. No read waits for the write, nor does the write wait for the reads. |
| T4 | **Waiting limit**: by default, a write request waits as long as necessary. Whoever makes it can indicate in that call a time limit; if it expires before obtaining its turn, the request fails with the new timeout error class, leaves the queue and has no effect. A limit of zero does not wait: it obtains the write if there is none open and no requests in the queue and, if not, fails with that class without being queued. A limit so large that it cannot be counted from the current moment fails with the misuse class (T5), without being queued; any limit of up to 24 hours is always accepted, and the maximum that the API admits always fails. |
| T5 | **Second write from the same thread**: if a thread asks for a write transaction on a store in which it already has one open, the request fails without waiting, with the new **misuse** error class: a library class for calls made in an order or with values that the library does not admit, distinct from the invalid options on opening and from misuse of the tool. The open transaction is not affected and can still be committed or aborted. A write request on another store does not count. |
| T6 | **Threads**: a read transaction can be passed to another thread and shared among several at the same time, which see the same snapshot, always within the life of the store (T8), that is, with threads that end before it is closed. A write transaction stays in the thread that opened it: the compiler prevents passing or sharing it. The store can be used from several threads at the same time. |
| T7 | **No limit of readers**: there is no maximum number of read transactions open at the same time; the only limit is memory (P13). |
| T8 | **Life of the transactions**: a transaction depends on the open store it came from. Closing the store while any is open, or using one after closing it, does not compile. Committing and aborting end the transaction: committing or aborting it again, or using it afterwards, does not compile. The references that a read returns are valid while its transaction is open, and using them afterwards does not compile (D11 of the parent); inside a write, the references to its pages are valid until the next change of that same transaction, and using them afterwards does not compile either. |
| T9 | **Commit without a journal** (temporary exception until module 3): committing makes the changes visible (T2) and takes them to `krummi.data` at the latest when the store is closed or when it is no longer used without closing it; after closing without error and reopening, all the committed changes are there. While the journal does not exist, after a crash of the process, of the system or a power cut with the store open, after an input/output failure that the close does not manage to resolve (T12), or after ceasing to use the store without closing it when the write to `krummi.data` fails, any commit after the last opening, or a part of it, may be missing from `krummi.data`. G6 still holds: on reopening, a half-written page is reported as damage, never as valid data. Inside the process, G1 holds in full. It is an exception to G1 (only on crashes), G2 and G5 of the parent, and to G12 in what the reopening recovers, annotated in it, which ends with module 3. |
| T10 | **Abort**: a write transaction that is aborted, or that is no longer used without committing it (including the panic of the thread that holds it, also in the middle of the commit), applies none of its changes, and the next request in the queue obtains its turn. The store remains usable without any action from the application. If the transaction made `krummi.data` grow, the file keeps the new size (P11). Committing a write with no changes and no failed operations ends without error and changes nothing: it does not write to `krummi.data`, so it is never the operation that encounters an input/output failure (T12). |
| T11 | **Failure inside a write**: if an operation of the write transaction fails (disk full G7, maximum size P10, out of memory P13, damage G6 or input/output G12), when reading or when writing, the transaction can no longer be committed. Its following operations can be attempted, but committing it fails with the class of the first failure, applies nothing, ends the transaction and the next request in the queue obtains its turn. Except with an input/output failure when writing (T12), the store keeps accepting writes. A commit that fails by itself follows the same rule: it applies nothing and ends the transaction. |
| T12 | **Store that does not accept writes** (G12): the first input/output failure when writing is reported by the operation that encounters it (an operation of the write, its commit, the commit of a later write or the close). From that moment, all the write requests that are waiting fail at once with the input/output class, and so do the new ones, even if the write in progress is still open. Read transactions keep working, and a new read sees what was committed up to the failure. On closing, the store tries again to take what was committed to `krummi.data`: if it succeeds, the close ends without error and, on reopening, everything committed is there; if not, the close returns the input/output class and releases the lock (P14). |
| T13 | **Precedence of failures when asking for a write**: when several no-wait failure conditions are met at once, the first one wins: misuse (T4, T5), input/output (T12), timeout with a limit of zero (T4). |
| T14 | **Retained versions and growth** (v1.1): until module 4, the pages that a write stops using are not reused. Each page keeps its number: the version that a write commits lives in memory (T16) while some open read needs the earlier one, and is then written in its place in `krummi.data`. `krummi.data` only grows when a write reserves new pages. All the space of the store counts toward the maximum size (P10): on disk, that of `krummi.data` plus that of the versions that are only in memory, including those of the open write; in temporary mode, all the memory of the store. A long read on a store that keeps being modified can take it to the maximum, and then a write that needs more space fails with the maximum-size class (T11). That space is recovered when the reads that retain it end, without reopening: the following writes fit again. The store's information (P12) describes `krummi.data` as it is, including what has grown through open or aborted transactions. |
| T15 | **Temporary mode**: transactions work the same as on disk (G9); what is committed lives in memory until the store is closed. |
| T16 | **Zero-copy reads in transactions** (D11 of the parent): a read returns a reference to the projection of `krummi.data`, to the memory of the temporary store or, for a committed version that is not yet in `krummi.data` or that is no longer in it, to the engine's memory where it is kept; never to a copy made for that read. It is a clarification of D11 of the parent, annotated in it. |
| T17 | **Header**: in this module the transactions do not rewrite the header; the journal (module 3) fixes when it is rewritten (P3 of the Pager). |
| T18 | **Data of the new errors** (D8 of the parent): both carry the store's folder, or that it is temporary; the timeout one also carries the requested limit. |

*Annotation of 2026-10-22 (v1.1), before implementation: T14 and AC-T16 said that the space retained by a long read
was not recovered in that opening. They are corrected to follow Voron's design, in which each page keeps its number
and the new version is written in its place when no read needs the earlier one, so that space is recovered when the
reads end.*

*Annotations of 2026-10-22 from module 3 ([journal.prd.en.md](journal.prd.en.md)):*

- *T9 and T12: the temporary exception ends; what was committed can no longer be missing after reopening, and a
  failure of the transfer to `muninn.data`, which happens outside the application's operations, is reported by the
  write requests, the commit with changes of the open write, the operation that makes what was committed durable,
  and the close (decision J13).*
- *T14, T16 and the Out of scope item about the scratch area: the committed versions that are not in `muninn.data`
  live in files of the `muninn.temp` subfolder, not in the process memory (decision J7).*
- *T17 and AC-T21: the header is rewritten when the transfer to `muninn.data` advances; AC-T21 no longer holds
  (decision J6).*

*Annotation of 2026-10-23 from the rename ([rename-to-krummi.prd.en.md](rename-to-krummi.prd.en.md)):*

- *Whole document: the product is called Krummi, its packages and the files of a store take the names of R2 and
  R3, and this document was updated with the new names (decisions R1–R13). The file names and format numbers of
  the earlier annotations are read according to R3 and R5, and the current format is `3`.*

*Annotations of 2026-10-23 from group and direct writes
([journal-group-commit.prd.en.md](journal-group-commit.prd.en.md)):*

- *T2, AC-T3 and AC-T5: a write also sees the commits that passed their point of no return before it got the
  turn, even if they are not durable; a read only sees durable commits, so, in deferred mode, it sees one when its
  group is durable and not on return (decision E3).*
- *T10 and AC-T19: a panic after the point of no return does not undo the commit; and a commit without changes in
  durable mode waits for the commits it saw to be durable, and can be the operation that finds an input/output
  failure (decisions E2 and E11).*
- *T12: a failure when writing a journal group is also reported by the commits waiting to be durable (decision
  E8).*
- *Whole document: the current format version becomes `4` (decision E15).*

*Annotations of 2026-10-24 from free space ([free-space.prd.en.md](free-space.prd.en.md)):*

- *T8 and T10: reserving and freeing pages are changes; a write that does so is not a write without changes, and the
  references to its pages stop being valid as after any other change (decision F8).*
- *T14: the "until module 4" ends: the pages a write frees are reused from the point of no return of its commit,
  without waiting for open reads, and a long read does not make `krummi.data` grow (decisions F6 and F7). The store
  information adds the number of free pages (decision F10).*
- *T18: the free-space errors carry the same data, and the damage one also the number of the damaged map page
  (decision F14).*
- *Whole document: the current format version becomes `5` (decision F11).*

*Annotations of 2026-10-24 from B+ trees ([trees.prd.en.md](trees.prd.en.md)):*

- *T8: an operation that can modify the transaction ends the references even if it fails without effect; opening an
  existing tree does not end them (decision B9).*
- *T10: a write whose only operations failed with invalid size, different tree type or a handle of a deleted tree is
  a write without changes (decisions B7 and B9).*
- *T11: those public validation errors are not among its failures and do not prevent committing; T11 failures and
  internal misuse still invalidate the transaction (decision B7).*

### Starting state

Verified on 2026-10-22:

| Aspect | Situation |
|---|---|
| MuninnDB | The Pager (module 1) is implemented. The public API only opens, queries and closes stores; there are no transactions. Internally, pages are read without copy while the store is open and are written directly to `muninn.data` with exclusive access to the store, without snapshots or isolation. The header is only written when the store is created. The existing error classes are those of the parent plus maximum size and out of memory; there is none of timeout or misuse. |
| Pager | The Pager's closing report leaves pending releasing the memory that the store retains when growing once no transaction uses it. |
| Parent PRD | [muninndb.prd.en.md](krummi.prd.en.md) v1.1 assigns to this module what G4 does not decide, the passing of transactions between threads, the limit of readers and the close with open transactions or the use of a closed store. P3 of the Pager leaves to modules 2 and 3 when the header is rewritten. |
| Voron (reference) | A single writer with a write lock that admits a waiting limit and fails when it expires; error if the same thread opens a second write; readers with no fixed limit; the pages that the write changes are copied to a scratch area, and what is committed goes through the journal before reaching the data file when no reader needs the earlier version. |

## Actors

- **Integrating developer**: opens read and write transactions from the threads of their application, commits or
  aborts the write ones and handles the errors by their class.

The operator and the command-line tool do not take part in this module.

## Flow

1. The application opens a store (flow of the [Pager](pager.prd.en.md#flow)).
2. From any thread, it opens read transactions; each sees its snapshot (T2) until it ceases to use it.
3. From a thread, it asks for a write transaction, with an optional waiting limit (T4). The request is checked in
   the order of T13 and, if there is another write open or requests in the queue, waits for its turn (T3).
4. Inside the write, the library reads and changes pages (internal, T1).
5. The application commits the write (T9) or aborts it (T10). The next request in the queue obtains its turn.
6. When no transaction remains open, the application closes the store (T8, P14); what was committed stays in
   `krummi.data` (T9).

## Validations and messages

The parent's and the Pager's hold. This module adds these situations, for the library only (the tool does not
open transactions):

| Situation | Rule | Error class |
|---|---|---|
| A thread asks for a write on a store in which it already has one open | The request fails without waiting; the open one remains usable (T5). | Misuse (new) |
| The waiting limit cannot be counted from the current moment | The request fails without being queued (T4). | Misuse (new) |
| A write is requested, or one is waiting, and the store does not accept writes | The request fails at once (T12). | Input/output |
| The waiting limit of a write request expires | The request fails with no effect and leaves the queue (T4). | Timeout (new) |
| Limit of zero with a write open or requests in the queue | The request fails without being queued (T4). | Timeout (new) |
| Memory runs out when opening a transaction or during it | The operation fails; the store remains usable (T7, P13). | Out of memory |
| An operation of a write fails | The transaction can no longer be committed (T11). | That of the failure |
| A write in which an operation failed is committed | The commit fails with the class of the first failure, applies nothing and ends the transaction (T11). | That of the first failure |
| A system write fails when committing or when taking what was committed to `krummi.data` | The operation that encounters it fails; the store stops accepting writes (T12). | Input/output |
| A store that stopped accepting writes is closed | The close tries again to take what was committed to `krummi.data`: without error if it succeeds; if not, it fails, releases the lock and what was committed may be missing on reopening (T12, T9). | Input/output, if it fails |
| Committing or aborting a transaction twice, or using it after it has ended | It does not compile (T8). | — |
| Closing the store with open transactions, using a transaction after closing it, or using a reference after its transaction ends or after a change of the same write | It does not compile (T8). | — |
| Passing to another thread or sharing a write transaction | It does not compile (T6). | — |

When a write request meets several rows at once, the one that comes first in T13 wins.

## Output

The module writes nothing to the screen nor adds data to the store's information (P12), which keeps its shape.
Its public output is the results of opening, committing and aborting transactions and the two new error classes,
with the data of T18.

On disk, the store's folder still contains only `krummi.data`, besides the foreign files (P7).

## Reuse

- The Pager: the pages, their integrity check, growth, the maximum size, out of memory and the zero-copy reads
  (P10, P11, P13, P15).
- The error classes of the parent and of the Pager; this module only adds two.

## Out of scope

- Storing data from the application: data structures and their API (module 5 onwards).
- A public API to read, reserve or write pages (T1, P1 of the Pager).
- Journal, durability of commits, atomicity on crashes and recovery (module 3, T9), including criteria AC1 to
  AC5 of the parent.
- Rewriting the header (T17, module 3).
- Keeping the scratch area where the versions of T14 and T16 live in temporary memory-mapped files, as Voron
  does: in this module it is in the process's memory, and the files arrive with the journal (module 3).
- Non-durable commit modes (G11, module 3).
- Reusing the space that writes stop using (module 4, T14).
- Several write transactions at the same time, even if they change different parts of the store (G4).
- Nested transactions, savepoints inside a transaction and converting a read into a write.
- Priorities among write requests, or an order other than arrival (T3).
- A default waiting limit for the whole store (T4).
- Cancelling from another thread a request that is waiting.
- Detecting deadlocks between stores: two threads that wait without a limit, each with a write open in the store
  that the other asks for, wait forever.
- Retrying a commit that failed (T11).
- A write transaction that the application forgets without ending it (without aborting, committing or ceasing to
  use it, something possible in safe Rust): the queue stays blocked until the process ends.
- A limit of readers, of the lifetime of a read, or evicting or aborting long reads (T7).
- Exposing the number of a snapshot or statistics of transactions and of the queue.
- Passing or sharing a write transaction between threads, and an asynchronous API (parent).
- Transactions that span several stores (parent).
- Performance thresholds: they are set by the benchmark (module 6).

## Left to the implementation

- When, between the commit and the close, the committed changes are written to `krummi.data` and when they are
  taken to disk (T9).
- How much the store grows with each commit (T14).
- How much memory a transaction uses and when the memory retained by the reads that end is released.
- The precision with which a waiting limit expires: never before the limit, with no bound on how much after.
- From what waiting limit, above 24 hours, the request fails with the misuse class (T4).
- At what moment of the operations of a write or of its commit the space is reserved and disk full and maximum
  size are detected, as long as the commit reports the failure (T11).
- The names of the types, functions, new error classes and fields of the public API, which follow Rust
  conventions and are in English, and how the waiting limit is indicated.
- How the tests simulate failures, lack of memory and the arrival order of the threads, including the injection
  points in the commit and in the handover of the turn that AC-T5, AC-T18 and AC-T19 need.

## Acceptance criteria

The criteria that need to read or write pages are internal tests of the library over its internal transactions
API, visible to the tests; the non-compilation ones use the public API when the criterion only touches
transactions and the internal one when it touches references to pages. When a criterion needs a known arrival
order, the test waits for each request to be in the queue before launching the next. "Without waiting" means
that the request returns its error while the write that would block it is still open.

This module checks at library level these criteria of the parent, adapted to what exists in it:

- **AC6**: as is, with pages as data.
- **AC7**: with several threads asking for the write at the same time without a limit, all obtain it and commit,
  and there are never two open at the same time; and with the write held by the test and requests queued in a known
  order, some without a limit and others with a short limit, those with a limit fail with the timeout class and
  the others obtain the write and commit in their arrival order (T3).
- **AC10**: altering each byte of a small store in which several write transactions have been committed and which
  has been closed, and then reading all the pages in a read transaction.
- **AC11**: with a write that needs to grow; the commit fails with the disk-full class (T11), applies nothing,
  reads keep working and, when space is freed, a following write commits without reopening.
- **AC12**: with a simulated failure in the first write to `krummi.data` of what a transaction committed; the
  operation that encounters it (the commit, the commit of a later write or the close) fails with the input/output
  class, the following write requests fail with that class (T12), reads work and see everything committed, and on
  reopening the store opens (recovery arrives with module 3).
- **AC14**: temporary mode with transactions creates no files or folders.
- **AC18** and **AC21**: as is.
- **AC20**: a read inside a read transaction returns a reference according to T16, never to a copy made for that
  read: two reads of the same page, in the same transaction and in another opened later without intermediate
  commits, return the same address, also for a committed version that is not yet in `krummi.data`; using it after
  the transaction ends does not compile.

And in addition:

1. **AC-T1** — Given a store, when a write transaction changes pages and commits, and then the store is closed and
   reopened, then a read transaction sees all the changes; the same when the store is left without closing it and
   reopened; and in temporary mode, a read opened after committing sees them.
2. **AC-T2** — Given a write transaction that changes pages and makes the store grow, when it is aborted, and in
   other tests when it is left unused without committing and when the thread that holds it panics, then no later
   read sees its changes, not even after closing and reopening, the size given by the store's information does not
   go down, and the next write request obtains it without any other action.
3. **AC-T3** — Given an open write transaction with uncommitted changes, when another thread, and the same thread,
   open a read, then the read does not see those changes and the write sees its own; and when the write commits, a
   read opened afterwards by the same thread sees them (T2).
4. **AC-T4** — Given a read opened by a thread before that same thread asks for a write that waits for its turn
   behind another, when the first write commits a change and the second obtains its turn, then the second sees that
   change and the read does not (T2).
5. **AC-T5** — Given a commit of a write that changes several pages, stopped by the test at each of its internal
   points, when another thread opens a read and reads those pages at each point, then it sees all the changes or
   none, and a read opened after the commit returns sees them all (T2).
6. **AC-T6** — Given a write opened by another thread, when a write is requested with a limit of 100 ms, then it
   fails with the timeout class, not before 100 ms and without effects, and a later request without a limit
   obtains the write when the open one ends; with a limit of zero, it fails without waiting; with a limit of zero
   and no write open or requests in the queue, it obtains it; and with a limit of zero, no write open and a request
   in the queue, it fails with the timeout class.
7. **AC-T7** — Given a write request with a limit that expires in the queue between two others, when the open write
   ends, then the other two obtain it in their arrival order (T3).
8. **AC-T8** — Given a thread with an open write, when it asks for another on the same store, without a limit,
   with a limit and with a limit of zero, then it fails without waiting with the misuse class, and the open one is
   committed afterwards without error; and when it asks for one on another store, it obtains it.
9. **AC-T9** — Given a write request with the largest limit the API admits, when it is requested, then it fails
   with the misuse class without being queued; and with a limit of 24 hours and the write free, it obtains it
   (T4).
10. **AC-T10** — Given a read transaction, when it is passed to another thread and shared among several while
    writes are committed, then all see the same snapshot; and passing or sharing a write transaction between
    threads does not compile (T6).
11. **AC-T11** — Given a store, when it is closed with a read or write transaction open, or a transaction is used
    after closing it, then it does not compile; when a write is committed or aborted twice, or used after it ends,
    it does not compile either; and inside a write, using a reference to a page after another change of the same
    transaction does not compile (T8).
12. **AC-T12** — Given a store, when 10,000 read transactions are opened at once from several threads (10,000
    open transactions, not 10,000 threads), then all open and read without error; and with a simulated memory
    failure when opening a read, it fails with the out-of-memory class and the store remains usable (T7).
13. **AC-T13** — Given a write in which an operation fails, in separate tests by maximum size, by out of memory
    and by a damaged page, when another operation is attempted and it is committed, then the commit fails with the
    class of the first failure, applies nothing, the next write request obtains it and a following write that does
    not fail commits (T11).
14. **AC-T14** — Given a store that stops accepting writes with the write that failed still open and another
    request waiting, when the failure occurs, then the request that waits fails with the input/output class
    without the open write ending; so does a new request, without waiting, including one with a limit of zero; a
    new request from the thread that has the write open, and one from another thread with the largest limit the API
    admits, fail with the misuse class (T13); and a new read sees what was committed up to the failure (T12).
15. **AC-T15** — Given an open read, when writes are committed that change the pages it sees, then its references
    keep pointing to the same address with the same content while it is open (T8, T16).
16. **AC-T16** — Given a store with a maximum size and an open read, when writes are committed that always change
    the same pages, then there comes a moment when a write fails with the maximum-size class, on disk and in
    temporary mode, applying nothing; and after the read ends, a write that changes those same pages commits without
    error, without reopening (T14).

    *Annotation of 2026-10-22 to AC-T16: with the design of T14 (v1.1), a long read retains at most one more
    version of each page changed since it started; the intermediate ones, which no read needs, are freed at once.
    Rewriting the same pages does not make memory grow without limit, so the failure comes when the maximum size
    leaves no room for those retained versions, not with any maximum.*
17. **AC-T17** — Given a write with no changes, when it is committed, then it ends without error and the store's
    information does not change; and with a simulated failure in every write to `krummi.data`, the commit of a
    write with no changes still ends without error (T10).
18. **AC-T18** — Given a write that ends while there is a request in the queue, when the test stops the handover
    of the turn and another thread asks for a write at that instant, without a limit and with a limit of zero,
    then the one in the queue obtains it first, the new one without a limit obtains it afterwards and the one with
    a limit of zero fails with the timeout class (T3, T4).
19. **AC-T19** — Given a commit that the test interrupts with a panic at each of its internal points, when a new
    read reads the store, then it sees none of its changes, and the next write request obtains it and commits
    without error (T10).
20. **AC-T20** — Given a store that stopped accepting writes because of a simulated failure when taking what was
    committed to `krummi.data`, when the failure stops occurring and it is closed, then the close ends without
    error and, on reopening, everything committed is there; and when the failure persists, the close fails with
    the input/output class and the store can be opened again (T12).
21. **AC-T21** — Given a store with commits, when it is closed, then the two copies of the header are identical
    byte for byte to those from before opening it (T17).
22. **AC-T22** — Given a write request whose limit expires, and another that fails by misuse, in a store on disk
    and in a temporary one, when the error is examined, then it carries the store's folder or that it is
    temporary, and the timeout one also carries the requested limit (T18).

## Annotations to the parent

This PRD declares a temporary exception and a clarification in the parent, which carries the corresponding
annotations:

- G1 (only on crashes), G2, G5 and G12 in what the reopening recovers: until module 3, a crash, an input/output
  failure that the close does not resolve or ceasing to use the store without closing it when the write fails may
  leave `muninn.data` without part of what was committed; G6 still holds (T9, T12).
- D11: the reference of a read may also point to the memory where the engine keeps a committed version that is not
  in `muninn.data` (T16).

## References

- Parent and root: [krummi.prd.en.md](krummi.prd.en.md) v1.1
- Previous module: [pager.prd.en.md](pager.prd.en.md)
- Voron, in the RavenDB repository: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron>

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| 103bdb2b | 2026-10-22 18:17 UTC+01:00 | 1 h 1 min | 0 min | 1 h 1 min | 26 min | 35 min | 26 |

<!-- prd-time: [{"session": "103bdb2b-a5ca-4e77-bfc3-9c07014dd34e", "start": "2026-10-22T17:17:11.788000+00:00", "wall": 3649, "discarded": 0, "agent": 1573, "human": 2075, "interventions": 26, "idle_minutes": 10}] -->
