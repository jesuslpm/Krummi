# Krummi Journal and Recovery — PRD

**Version**: 1.0
**Date**: 2026-10-22
**Parent document**: [Krummi, an embedded storage engine inspired by Voron — PRD](krummi.prd.en.md) (v1.1)

## Objective and context

Build the **journal and recovery**, module 3 of the [parent's split](krummi.prd.en.md#split-into-child-prds):
a write-ahead journal that makes commits durable (G2), with compression; the transfer of what is committed to the
data file; and the automatic recovery when opening a store that was closed by a crash (G5).

When this module is done, a durable commit that finishes without error survives a process crash, a system crash or
a power cut, and the next open returns the store, without any intervention from the application, to a state with
everything committed and no parts of half-done commits. This ends the temporary exception that module 2 declared
for G1, G2, G5 and G12 (T9 of [transactions.prd.en.md](transactions.prd.en.md)).

**Why now**: it is the central guarantee of the product (objective 1 of the parent) and the basis of the modules
that copy the store in use (backup, module 10). In addition, module 2 left here when the header is rewritten (T17)
and moving to files the page versions that are not yet in `krummi.data` (T14, T16).

This PRD inherits everything from the parent, from the [Pager](pager.prd.en.md) (P1–P17) and from
[Transactions](transactions.prd.en.md) (T1–T18); only what is specific to the module is added here. What changes
from earlier rules is declared in its decisions and annotated in the affected documents (see
[Annotations to other PRDs](#annotations-to-other-prds)).

### Product decisions

| ID | Decision |
|---|---|
| J1 | **Public surface of the module**: when committing a write transaction, deferred mode (J10) can be chosen; durable is the default. The store offers an operation to make everything committed durable (J11). The journal, the transfer and the recovery have no public API: recovery happens on open. |
| J2 | **Store files**: besides `krummi.data`, the store folder can contain the journal files, named `krummi.journal.` followed by a 19-digit decimal number with leading zeros (for example `krummi.journal.0000000000000000001`), and the `krummi.temp` subfolder of the temporary area (J7). Those names are reserved (P3): in a store folder, the entries with those names belong to the store, which creates them and deletes them when it no longer needs them; the other names follow the rule for foreign entries (P7). An open that finishes without error leaves in the folder only `krummi.data`, besides the foreign entries; the journal files and `krummi.temp` appear later, with the commits. A folder without `krummi.data`, or whose `krummi.data` does not start with the mark, follows the parent's rule: if it has any entry, including journal files or `krummi.temp` and also next to `krummi.data.new` (P8), it is not a store and nothing is touched. Refines P3 and P7. |
| J3 | **Format `2`**: an engine without a journal cannot safely open a store with pending commits in the journal, so the current format version becomes `2` (D7 of the parent). A store of format `1` is of an earlier format even if it was closed without error. Refines P5. |
| J4 | **Durable commit** (G2): committing in durable mode returns without error only when the commit is durable: it survives a process crash, a system crash or a power cut even if it is not yet in `krummi.data`. Until then its changes are not visible to any other transaction and the next write request does not get its turn (T2, T3). |
| J5 | **Transfer to `krummi.data`**: what is committed moves to `krummi.data` without the application asking for it and without waiting for the close, as soon as no open read needs the previous version of those pages (T14), without blocking readers or the writer. Only what is already durable is transferred: if a deferred commit has to be transferred, it is first made durable, together with all the earlier ones (J10). Thus, without reads that prevent it, the journal does not grow without limit while the store is open; while a read prevents the transfer, it grows with no limit other than the disk (G7). Transferring never makes `krummi.data` grow: it grows when pages are reserved inside the write (T14), so the lack of space and the maximum size are detected in the write or in its commit (G7, P10, P11), never after the commit returns. |
| J6 | **Header**: it is rewritten when what has been transferred and flushed to disk advances. It replaces T17; AC-T21 no longer applies. |
| J7 | **Temporary area in files**: the committed versions of pages that are not yet in `krummi.data` (T14, T16) are kept in files inside `krummi.temp`, not in process memory. They count toward the maximum size as in T14, but a write never fails because of the maximum size on account of versions that no read needs: they are transferred first. The references of the reads (D11, T16) can point to them and remain valid, with the same address and the same content, while their transaction is open, even if the area grows. A failure when creating or writing them makes the operation or the commit that needs them fail with its class (access, disk full, out of memory or input/output), without applying anything and without it appearing on reopen, and follows T11 and T12. Since reads do not copy, a physical disk failure when reading them ends the process, as in P17. `krummi.temp` is deleted on close; the one a crash leaves is deleted by the next open in its step 8, after checking the journal, whether or not there is anything to recover. It is never read for recovery. Refines T14, T16 and P17. |
| J8 | **Recovery** (G5, step 8 of the parent's open order): if the journal has commits that are not recorded as transferred, the open applies them to `krummi.data` in the order in which they were committed; in any case, it ends without journal files or `krummi.temp`, which are deleted only after recovering: an open that fails deletes neither of the two. A damaged journal (J9) is reported without touching the store. If a journal file cannot be read because of permissions, or a journal file or `krummi.temp` cannot be deleted, the open fails with the access class. If recovery fails for lack of space or memory, or because of an input/output failure, the open fails with its class and the next one repeats it (G5). When several occur, the first in this order wins: damage, access, and then whichever of the others is found. With one of the two header copies damaged (P6), recovery still brings back everything committed. Recovery makes `krummi.data` grow as much as the pages of the recovered commits need, even if that exceeds the maximum size requested on open, and never shrinks it; the store then opens as one larger than the maximum (P10). The growth made by a write that did not get to commit may not survive a crash (P11). Refines P10 and P11. |
| J9 | **Damaged journal**: a commit in the journal is **durable according to the journal** if some later intact commit was written when it was already durable. A commit that is incomplete, that fails its integrity check, or that is missing because the journal file where it belonged is missing or empty, is a commit that did not finish if it is not durable according to the journal: it is discarded together with all the later ones (G5, J10). If it is durable according to the journal, the journal is damaged: the open fails with the damage class without touching the store (G6), with the same error as a store that cannot be opened because of damage. It is also damage if an entry with a reserved name is not of the expected type (a file where a folder goes or the other way round). What is already recorded as transferred is not read and its damage is not reported. Consequence: if the last durable commit is damaged with no later commit, it cannot be told apart from one that did not finish and it is discarded without error. Refines G5, G6 and D10 of the parent. |
| J10 | **Deferred mode** (G11): a deferred commit returns without waiting to be durable; from that moment it is visible (T2) and survives a process crash, but not necessarily a system crash or a power cut. After one of those, recovery returns the store to a prefix of the commits: all the durable ones and, of the deferred ones after the last durable one, the first ones in order up to some point, never part of one nor one without all the earlier ones. A deferred commit becomes durable, together with all the earlier ones, when a later durable commit with changes finishes, the operation of J11, its transfer to `krummi.data` (J5) or a close without error, including ceasing to use the store without closing it (J14). A durable commit without changes writes nothing (T10) and does not make the earlier ones durable. The lack of space and the failures when writing the journal are reported in the deferred commit just as in the durable one (J12). The mode does not change the queue (T3), the wait limits (T4) or the precedence of T13. |
| J11 | **Making what is committed durable**: the operation returns when all the commits that had returned before it was called are durable; those in progress in the meantime are not guaranteed. It does not enter the writer queue (T3), does not wait for the open write and has no wait limit. In a store that does not accept writes (T12) it fails at once with the input/output class, even if nothing is pending. Otherwise, and if nothing is pending, it returns without error without writing. If it fails when flushing the journal to disk, it fails with that class and the store stops accepting writes (G12); the pending deferred commits follow the prefix rule of J10. |
| J12 | **Failures when writing the journal**: if space is lacking, the commit fails with the disk-full class, applies nothing and never appears on reopen (G7). If memory is lacking, the same with the out-of-memory class, and the store keeps accepting writes (P13). If flushing the journal to disk fails, or creating its files or writing it for any other reason, the commit fails with the input/output class, is not visible, the store stops accepting writes (T12) and on reopen the commit is whole or absent, like an interrupted commit (G1). What a commit that fails, or that is interrupted by a panic (T10), leaves half-done in the journal never prevents recovering the following commits, not even after a power cut, and appears on reopen only in the input/output case. |
| J13 | **Failures outside an operation**: an input/output failure when transferring or when flushing `krummi.data` to disk, when it does not happen inside an application operation, leaves the store not accepting writes (T12) from that moment: it is reported with that class by the write requests that are waiting and the new ones, the commit with changes of the open write, the operation of J11 and the close; reads keep working and a commit without changes still finishes without error (T10). On reopen, recovery brings back everything committed. It refines T12 (who reports the failure) and replaces the part of T9 and T12 according to which what was committed could be missing on reopen. |
| J14 | **Close**: closing without error leaves everything committed in `krummi.data` flushed to disk, the header up to date and the folder without journal files or `krummi.temp`, so the next open recovers nothing. Ceasing to use the store without closing it does the same (T9, P9). If the close fails (P14, T12), the journal files and `krummi.temp` may remain, the lock is released and the next open recovers everything committed (J8). |
| J15 | **Maximum size** (P10): the journal does not count; the versions in `krummi.temp` count as in J7. The store information (P12) still describes only `krummi.data`. |
| J16 | **Compression**: what is kept in the journal takes up less than the pages it changes when their data compresses well; data that does not compress is kept anyway, without error. Compression changes nothing of what transactions read nor of what `krummi.data` contains. |
| J17 | **Temporary mode**: it has no journal, no `krummi.temp` and no recovery (G9). Deferred mode and the operation of J11 are accepted without error and do nothing more. |

*Annotations of 2026-10-23 from the rename ([rename-to-krummi.prd.en.md](rename-to-krummi.prd.en.md)):*

- *Whole document: the product is called Krummi, its packages and the files of a store take the names of R2 and
  R3, and this document was updated with the new names (decisions R1–R13). The file names and format numbers of
  the earlier annotations are read according to R3 and R5, and the current format is `3`.*
- *J3: the current format version becomes `3`; a store of format `2` is of an older format (decision R5).*

*Annotations of 2026-10-23 from group and direct writes
([journal-group-commit.prd.en.md](journal-group-commit.prd.en.md)):*

- *J2: the journal files have their final size from the moment they appear (decision E7).*
- *J3: the current format version becomes `4`; a store of format `3` is of an older format (decision E15).*
- *J4: the commit yields the turn at its point of no return, before being durable; from there the following writes
  see it, and reads only when it is durable (decisions E2 and E3).*
- *J9 and AC-J3: the commits that are written together count as a group; the window in which damage is discarded
  without error moves from the last commit to the last group, and a journal file without intact commits counts as
  empty (decisions E7 and E9).*
- *J10 and AC-J7: a deferred commit becomes durable at the same moment as one in durable mode, can be lost to a
  process crash until then, and reads do not see it until it is durable; its list of when it becomes durable and
  the point of Left to the implementation about how long it takes are replaced; it can wait for room in the
  journal queue or for the reservation of a new file, and the failures when writing the journal are reported in it
  only before its point of no return (decisions E2, E3, E5 and E8).*
- *J11: it also waits for the commits that passed their point of no return before it was called (decision E10).*
- *J12 and AC-J14: the lack of space and of memory are detected before the point of no return, and a later panic
  does not undo the commit; in AC-J14, the panic is caused before the point of no return. A commit that failed
  with the input/output class may be there on reopen if the close manages to write it (decisions E2 and E12).*
- *J13 and AC-J9: a failure when writing a group is reported by its commits that are waiting and by the commit
  without changes in durable mode of a write that saw it, which no longer always finishes without error
  (decisions E8 and E11).*
- *Out of scope: grouping commits from several threads into a single journal write enters with that change
  (decision E4).*

*Annotations of 2026-10-24 from free space ([free-space.prd.en.md](free-space.prd.en.md)):*

- *J8: when recovery ends, `krummi.data` reaches at least the frontier of used pages (decision F3).*
- *J15: the store information adds the number of free pages, which combines what is durable in the free-space map
  with the size of `krummi.data` (decision F10).*
- *J3: the current format version becomes `5`; a store of format `4` is of an older format (decision F11).*

*Annotation of 2026-10-25 from trees and key operations ([trees-keys.prd.en.md](trees-keys.prd.en.md)): J3, the current format version becomes `6`; a store of format `5` is of an older format (decision A13).*

### Starting state

Verified on 2026-10-22:

| Aspect | Situation |
|---|---|
| MuninnDB | The Pager (module 1) and transactions (module 2) are implemented. There is no journal: what is committed is written in place in `muninn.data` at the latest on close or on ceasing to use the store (T9). The committed versions that are not yet in `muninn.data` live in process memory. The header stores the format version (`1`), the page size and a revision that is only written when the store is created (T17). The error classes are the parent's plus maximum size, out of memory, timeout and incorrect use. There are no commit modes and no operation to flush what is committed to disk. |
| Transactions | The closing report of module 2 leaves pending the memory that a long read holds without a maximum size, and that the temporary-area files arrive with this module. Before failing with maximum size, a write already moves into place the versions that no read needs. |
| Parent PRD | [muninndb.prd.en.md](krummi.prd.en.md) v1.1 assigns to this module the non-durable modes (G11) and the full disk during recovery, and annotates it with the temporary exception of T9, which ends here. |
| Voron (reference) | Journal in numbered files with direct writes to the physical medium and compressed; each transaction with its integrity check; transfer to the data file in the background when no reader needs the previous version; the header records the last journal and the last transaction transferred; recovery applies the journal from there and discards a half-done tail; safe, deferred and no-journal commit modes; temporary area in files in a folder of its own that are deleted on open. |

## Actors

- **Integrating developer**: commits transactions in durable or deferred mode, asks to make what is committed
  durable, opens stores that were closed by a crash and handles errors by their class.

The operator and the command-line tool do not take part in this module (D9 of the parent arrives with module 12).

## Flow

1. The application opens a store (flow of the [Pager](pager.prd.en.md#flow)). In step 8 of the parent's order, the
   open checks the journal, recovers the commits that are not recorded as transferred and deletes the journal and
   the `krummi.temp` that a crash left (J7, J8).
2. The application opens transactions (flow of [Transactions](transactions.prd.en.md#flow)) and commits the write
   ones in durable or deferred mode (J4, J10).
3. When it needs to, it asks to make everything committed durable (J11).
4. Without intervention from the application, what is committed moves to `krummi.data` as soon as no read needs the
   previous version (J5), and the header records the progress (J6).
5. The application closes the store: everything committed is in `krummi.data` and the folder has no journal or
   `krummi.temp` (J14).

## Validations and messages

The parent's, the Pager's and Transactions' apply. This module adds no error classes or tool messages. Summary of
the new situations, library only; the complete rule is in the decision cited:

| Situation | Rule | Error class |
|---|---|---|
| Store of format `1` | The open fails without touching it (J3). | Different format |
| Folder without `krummi.data`, or with `krummi.data` without the mark, with journal files or `krummi.temp` | It is not a store; nothing is touched (J2). | Not a store |
| Incomplete, damaged or missing commit, durable according to the journal; or reserved entry of another type | The open fails without touching the store (J9). | Damage |
| Incomplete, damaged or missing commit, not durable according to the journal | It is discarded with the later ones; the open continues (J9). | — |
| Lack of space or memory, or system failure, during recovery | The open fails without deleting the journal; the next one repeats (J8). Damage and access win. | Disk full, out of memory or input/output |
| A journal file cannot be read, or the journal or `krummi.temp` cannot be deleted, on open | The open fails (J8). | Access |
| Lack of space or memory when writing the journal on commit | The commit fails and never appears on reopen (J12). | Disk full or out of memory |
| Other failure when creating, writing or flushing the journal to disk on commit | The commit fails; the store stops accepting writes; on reopen it is whole or absent (J12). | Input/output |
| Failure when creating or writing `krummi.temp` | The operation or commit fails without applying anything (J7). | Access, disk full, out of memory or input/output |
| Failure of the transfer or when flushing `krummi.data` to disk | The store stops accepting writes; the operations of J13 report it. | Input/output |
| Making what is committed durable in a store that does not accept writes, or failure when doing so | It fails; in the second case the store stops accepting writes (J11). | Input/output |

## Output

The module writes nothing to the screen and does not change the shape of the store information (P12); the format
version it reports becomes `2`.

On disk, the folder of a freshly opened store, or one closed without error, contains only `krummi.data` and the
foreign entries (J2, J14). While open it can also contain journal files and `krummi.temp`. Example, with an open
store in which a long read prevents transferring what is committed:

```text
krummi.data
krummi.journal.0000000000000000001
krummi.temp\
```

## Reuse

- The Pager: pages, integrity check, double header, growth, maximum size and out of memory (P6, P10, P11, P13).
- Transactions: the writer queue, snapshots, the temporary area, the rule of when a version is written in place and
  the one of moving into place what no read needs before failing with maximum size (T3, T14, T16), and the rules
  for failures inside a write and for a store that does not accept writes (T11, T12).
- The existing error classes; this module adds none.

## Out of scope

- Encryption of the journal and of `krummi.temp` (module 9).
- Keeping the journal for incremental backups (module 10).
- Recovering from the command-line tool (module 12, D9), and opening a store that needs recovery without modifying
  it.
- A no-journal mode like Voron's *Danger*, in which a crash can leave the store half-done.
- A default commit mode for the whole store or an open option to choose it (J1, J10).
- Informing the application that a recovery took place, how much it recovered, or whether a deferred commit is
  already durable.
- Repairing a damaged journal or opening a store discarding the damaged part (data rescue, module 12).
- A size limit for the journal while a long read prevents transferring, or for the time recovery takes (J5).
- A wait limit or the cancellation of the operation of J11.
- An API to force the transfer or the rewriting of the header.
- Choosing another folder for the journal or for `krummi.temp`.
- Deleting, trimming or replacing journal files with the store closed, or bringing those of another store: only
  what J9 calls damage is detected; deleting the last journal files, or copying `krummi.data` without its journal,
  is not detected.
- Symbolic links or other special entries with reserved names.
- Grouping commits from several threads into a single journal write.
- Migrating stores of format `1` (D7).

## Left to the implementation

- The compression algorithm and the internal organization of the journal and of `krummi.temp` (parent), including
  how a commit is recognized as durable according to the journal (J9).
- How many journal files there are, their size, the number of the first one and when a new one is created, as long
  as the numbers grow one by one; and when the part already transferred is deleted or reused, as long as J8 holds
  with one header copy damaged.
- How long the transfer takes after a commit and when the header is rewritten, as long as J5 holds.
- In deferred mode, how much time can pass until a commit is durable without a durable one with changes, the
  operation of J11 or a close.
- Where the changes of the open write live before it is committed, and the names of the files inside
  `krummi.temp`.
- The names of the types, functions and fields of the public API, which follow Rust conventions and are in English,
  and how the mode is indicated on commit.
- How the tests simulate crashes, power cuts, failures when writing and when flushing to disk, lack of space and of
  memory, and how they stop the transfer.

## Acceptance criteria

The criteria that need to read or write pages are internal tests of the library over its internal transactions
API, as in module 2; those that need to know how the journal is organized (where a commit starts, in which file it
is) use the knowledge of the implementation itself, like the Pager's with the header. For the journal to have
untransferred commits, the test keeps a read open or stops the transfer. A process crash is simulated by killing a
child process. A power cut is simulated by discarding everything that was not flushed to the physical medium,
including file creations, deletions and size changes, and leaving the last write of each file half-done.

This module checks at library level these criteria of the parent, adapted to what exists in it:

- **AC1, AC2, AC3 and AC4**: as they are, with pages as data and with the journal untransferred.
- **AC5**: as it is, interrupting recovery with a process crash and with a simulated power cut.
- **AC6, AC7 and AC8**: as they are.
- **AC10**: altering each byte of a store closed by a crash with durable untransferred commits, its journal files
  included, and opening it and reading all the pages: each alteration produces an error of the damage class, no
  difference, or, only if the alteration is in the last commit of the journal, the discarding of that commit (J9);
  never data that was not written.
- **AC11**: with a commit that does not fit in the journal for lack of space (J12).
- **AC12**: with a simulated failure when flushing the journal to disk in a durable commit; on reopen all the
  earlier durable commits are there and the one that failed is whole or absent (J12).
- **AC14**: temporary mode creates no journal and no `krummi.temp`.
- **AC18**, **AC20** and **AC21**: as they are; AC20 also with versions in `krummi.temp`.

The criteria of Transactions still apply, with these changes: AC-T21 no longer applies (J6); the adaptation of AC12
and AC-T20, which inject the failure in the writing of what is committed to `krummi.data`, are replaced by AC12 and
AC-J9; and AC-T15 and AC-T16 are repeated with the versions in `krummi.temp`.

And in addition:

1. **AC-J1** — Given a folder that does not exist, when it is opened, then it contains only `krummi.data`; with the
   transfer stopped by the test, after a commit with changes it also contains journal files; and after closing
   without error, only `krummi.data`, and on reopen all the changes are read (J2, J14).
2. **AC-J2** — Given a store with an open read that prevents transferring, when writes are committed and the
   process is killed, then on reopen all of them are read and the folder contains only `krummi.data` (J8).
3. **AC-J3** — Given a journal whose last commit is trimmed, and in another test altered in one byte, when it is
   opened, then it opens without error without any of its changes and with all those of the earlier ones; and given
   a journal with a durable commit altered in one byte followed by another intact durable one, the open fails with
   the damage class and the store files, `krummi.temp` included, stay identical byte for byte (J9).
4. **AC-J4** — Given a store closed by a crash whose journal takes up several files with durable commits, when one
   that is not the last is deleted and it is opened, then it fails with the damage class and the rest stays
   identical byte for byte; and when `krummi.temp` is replaced by a file, or a journal file by a folder, the same
   (J9).
5. **AC-J5** — Given a store whose commits made `krummi.data` grow, closed by a simulated power cut that loses that
   growth, and a disk without space for it to grow again, when it is opened, then it fails with the disk-full class
   and the journal files are still present; when space is freed and it is opened again, it opens with everything
   committed; and if in addition the journal has a damaged durable commit, it fails with the damage class (J8).
6. **AC-J6** — Given a valid store whose header carries format `1`, when it is opened, then it fails with the
   different-format class as an earlier format and stays identical byte for byte (J3).
7. **AC-J7** — Given the sequence of commits durable, deferred, deferred, durable, deferred, deferred, when a power
   cut is simulated at each point where the journal is written or flushed to disk, then on reopen it opens without
   error and the set of commits present is a prefix of the sequence that includes all the durable ones that had
   returned; when the process is killed after the last deferred one returns, all are there; and when J11 was called
   before the cut or the store was closed without error, all are there (J9, J10, J11).
8. **AC-J8** — Given the operation of J11, when it is called with nothing pending, it returns without error; when
   it is called from another thread with a write open and held by the test, it returns without waiting for it; with
   a simulated failure when flushing the journal to disk, it fails with the input/output class and the following
   write requests fail with that class; in a store that no longer accepts writes and with nothing pending, it fails
   at once with that class; and in temporary mode, it returns without error, like a deferred commit (J11, J17).
9. **AC-J9** — Given a simulated input/output failure in the transfer, when it happens, then the next write request
   fails with that class, a commit without changes finishes without error and reads work; the close with the
   persistent failure fails with that class, and on reopen without the failure all the commits are there (J13,
   J14).
10. **AC-J10** — Given a store closed by a crash that needs recovery, when it is opened with a maximum size smaller
    than that of its `krummi.data`, then it opens with everything committed and the first later growth fails with
    the maximum-size class (J8, P10).
11. **AC-J11** — Given a long read while writes are committed, when the folder is observed, then it contains
    `krummi.temp` with files; when the reads end and the store is closed, it disappears; and after a crash with it
    present, the next open deletes it, also when there is nothing to recover (J7).
12. **AC-J12** — Given a store closed by a crash with untransferred commits after the header has been rewritten
    several times, when a header copy is damaged, in one test the one the open uses and in another the other one,
    then it opens with everything committed (J8, P6).
13. **AC-J13** — Given a durable commit that the test stops after writing the journal and before it is durable, when
    another thread opens a read, then it does not see its changes; and when a power cut is simulated at that point,
    on reopen the commit is whole or absent (J4).
14. **AC-J14** — Given a disk without space for the journal, when a commit is made, then it fails with the
    disk-full class; and when space is freed, another write is committed durably and the process is killed, or in
    another test a power cut is simulated, on reopen the second one is there and the first is not; the same when the
    first commit is interrupted by a panic after starting to write the journal (J12).
15. **AC-J15** — Given a commit that changes 100 pages full of the same byte, when what it takes up in the journal
    is measured according to the implementation itself, then it is less than those pages; and given one with random
    bytes, it commits without error and is recovered after a crash (J16).
16. **AC-J16** — Given a folder with only a journal file, another with a journal file and `krummi.data.new`, and
    another with only `krummi.temp`, when they are opened, then they fail with the not-a-store error class and stay
    identical byte for byte (J2).
17. **AC-J17** — Given a transfer that the test stops, when another thread commits a write and opens and reads a
    read, then both finish without waiting for the transfer (J5).
18. **AC-J18** — Given a store with a maximum size that only leaves room for the pages that one write changes, when
    2,000 writes that change those same pages are committed with no reads open, then all of them commit without
    error; and, waiting in each measurement for the transfer to finish, the size on disk of the journal files after
    the 2,000 is not greater than after the first 1,000 (J5, J7, J15).

## Annotations to other PRDs

This PRD changes or refines rules of earlier documents, which carry the corresponding annotations:

- Parent, G5, G6 and D10, and AC10: a damaged, incomplete or missing commit that is not durable according to the
  journal is discarded without error like one that did not finish, and the last durable commit damaged with no later
  one cannot be told apart from it (J9).
- Parent, temporary exception to G1, G2, G5 and G12 (T9): ends with this module (J4, J13).
- Pager, P3 and P7: the names in J2 belong to the store, which creates and deletes them (J2).
- Pager, P5: the current format version becomes `2` (J3).
- Pager, P10 and P11: recovery makes `muninn.data` grow as much as the recovered commits need even if that exceeds
  the requested maximum, and the growth of a write that did not get to commit may not survive a crash (J8).
- Pager, P17: it also applies to the files of `muninn.temp` (J7).
- Transactions, T9 and T12: what is committed can no longer be missing on reopen, and a transfer failure is reported
  by the operations of J13.
- Transactions, T14, T16 and the *Out of scope* item about the temporary area: the committed versions that are not
  in `muninn.data` live in `muninn.temp` (J7).
- Transactions, T17 and AC-T21: the header is rewritten when the transfer advances (J6).

## References

- Parent and root: [krummi.prd.en.md](krummi.prd.en.md) v1.1
- Previous modules: [pager.prd.en.md](pager.prd.en.md), [transactions.prd.en.md](transactions.prd.en.md)
- Voron, in the RavenDB repository: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron>

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| cdcd1dd6 | 2026-10-22 20:45 UTC+01:00 | 31 min | 0 min | 31 min | 25 min | 6 min | 5 |

<!-- prd-time: [{"session": "cdcd1dd6-3127-46eb-9d09-5e2bd3a6b0a9", "start": "2026-10-22T19:45:44.292000+00:00", "wall": 1853, "discarded": 0, "agent": 1515, "human": 338, "interventions": 5, "idle_minutes": 10}] -->
