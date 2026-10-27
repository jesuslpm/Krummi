# Journal: group and direct writes — PRD

**Version**: 1.0
**Date**: 2026-10-23
**Parent document**: [Krummi Journal and Recovery — PRD](journal.prd.en.md) (v1.0)

## Objective and context

Change how the journal makes commits durable, so that several commits made from different threads share a single
journal write (*group commit*, like InnoDB and PostgreSQL), and so that the journal is written directly to the
physical medium, without going through the system cache.

Today each commit in durable mode writes its record, waits for the disk to save it and only then yields the write
turn: while the disk works, no other write advances, and durable commits per second are limited by the latency of
the disk. With this change, the commit yields the turn before waiting for the disk, the next write works in the
meantime, and the commits that coincide in the wait are written together. A deferred commit becomes durable at the
same moment as one in durable mode; the only difference is that it returns without waiting for it.

G4 does not change: there is at most one open write transaction. A commit that has already yielded the turn and is
waiting to be durable is not an open transaction.

**Why now**: the closing report of the journal follow-ups left pending that a deferred commit that reaches the
threshold of pending commits waits for the disk, against the letter of J10; resolving it needs another thread to
write the journal, which is what this change brings. It is also the performance base that the comparative benchmark
(module 6) will measure.

This PRD inherits everything from the [Journal](journal.prd.en.md) (J1–J17) and, through it, everything from the
root parent, the [Pager](pager.prd.en.md) and [Transactions](transactions.prd.en.md). What changes from earlier
rules is declared in its decisions and annotated in the affected documents (see
[Annotations to other PRDs](#annotations-to-other-prds)).

In this document, the **journal queue** is the queue of the commits that yielded the turn and are waiting to be
written, and a **group** is the set of commits that are written together; the queue of write requests remains the
one of T3.

### Product decisions

| ID | Decision |
|---|---|
| E1 | **Public surface**: it does not change. There are no new open options or commit options. |
| E2 | **Point of no return**: when committing a write with changes, the commit first does everything that can fail: the failures of T11, saving its pages in `krummi.temp` (J7), preparing it for the journal and reserving its place in it (E7). If something fails, it applies nothing and T11, J7 and J12 follow: with the input/output class the store stops accepting writes (T12); with the others it keeps accepting them. If the journal queue is full, or if the commit needs a new journal file (E7), it waits, without yielding the turn, for there to be room or for the file to be reserved, also in deferred mode; if in the meantime the store stops accepting writes, it fails with the input/output class without applying anything. Then, the commit enters the journal queue and yields the turn: the next write request gets it without waiting for the previous one to be durable. That moment is the **point of no return**: from it the commit can no longer be undone, not even by a panic of the thread that requested it, and it can only fail as E8 says. A failure of a commit before its point of no return does not affect those already in the journal queue: they are written and become visible as usual, also when that failure leaves the store not accepting writes. It replaces the part of J4 according to which the next write request does not get its turn until the commit is durable. |
| E3 | **What each transaction sees** (store on disk; the temporary one follows E14): a write sees, besides what is in T2, all the commits that passed their point of no return before it got the turn, whether or not they are already durable. A read only sees durable commits, in both modes: a group becomes visible in full to new reads as soon as it is durable, and a commit in durable mode does so before returning, so that every transaction opened after it returns sees it (J4, T2). A deferred commit is not visible to reads until it is durable, even if it has already returned, not even to those of the same thread (E5). Commits become visible in the order in which they passed their point of no return: a read that sees a commit sees all the earlier ones. It replaces the part of J10 and T2 according to which a deferred commit is visible to every transaction opened after it returns. |
| E4 | **Group commit**: the commits that are in the journal queue at the same time are written together, in the order in which they passed their point of no return, and become durable at the same time. One in durable mode returns when it is durable (J4); that wait has no limit and cannot be cancelled. |
| E5 | **Deferred commit**: it becomes durable at the same moment as a commit in durable mode that had passed the point of no return in its place would; the journal does not treat the two modes differently. It returns at its point of no return, without waiting to be durable or for the earlier ones to be; from then on the following writes see it, and reads see it when it is durable (E3). Until its group is durable, a process crash, a system crash or a power cut can lose it, always according to the prefix rule of J10. It replaces the part of J10 according to which a deferred commit survives a process crash from the moment it returns. |
| E6 | **Direct write**: the journal is written directly to the physical medium, without going through the system cache, so that finishing writing a group already makes it durable. If the file system or the disk does not support it, the journal is written through the cache and flushed to disk in each group, with the same guarantees. Checking whether it is supported does not create files other than those of J2 nor does it make the open or a commit fail by itself, and the store does not report which of the two modes it uses. A failure when writing with direct write is a failure of E8, never a switch to the alternative. `krummi.data` and `krummi.temp` are written as until now. |
| E7 | **Journal reservation**: each journal file is created when a commit needs it (J2), with its final size, and its creation and size are durable before the point of no return of that commit, so writing a group never changes the size of a file. The lack of space for the journal is detected on reservation, before the point of no return (E2). A commit larger than the usual size of a file is admitted. The reserved and unused part of a file, and a journal file without any intact commit, are not damage: the latter counts as empty for J9. |
| E8 | **Failures after the point of no return**: if writing or flushing a group to disk fails, all the commits of that group and of the journal queue that had not yet returned fail with the input/output class, and the store stops accepting writes (T12). From that failure nothing more is written to the journal until the close, which retries it (E12). If none is waiting, the operations of J13 report it. The write open at that moment, which may have seen those commits (E3), fails when committed with changes with that class. Reads never see the commits of that group or of the journal queue, not even the deferred ones that had already returned. On reopen, all the commits in durable mode that had returned without error are there and, of the others, the first ones in order up to some point (J10). |
| E9 | **Damage and crashes with groups**: for J9, the commits of a group count together: a group is durable according to the journal if some later intact group was written when it was already durable. Thus, the window of J9 in which damage cannot be told apart from a commit that did not finish moves from the last commit to the last group: damage in it discards without error the damaged commit and the later ones, even if they had returned without error. A process crash, a system crash or a power cut while a group is being written never damages or makes earlier groups disappear. |
| E10 | **Making what is committed durable** (J11): it returns when all the commits that passed their point of no return before it was called are durable, including those that have not yet returned. The rest of J11 does not change. |
| E11 | **Commit without changes** (T10): in durable mode, if the write saw commits that are not yet durable (E3), it waits for them to be and, if they fail (E8), it fails with the input/output class; otherwise it finishes at once without error. In deferred mode it finishes at once without error. In no case does it write to the journal. Aborting a write (T10) never waits. |
| E12 | **Close**: closing, or ceasing to use the store without closing it, waits for everything in the journal queue to be written and durable, and then J14 follows. In a store that does not accept writes, the close retries writing the queue and flushing it to disk, like T12: if it succeeds, it finishes without error, and on reopen there may also be commits that failed with the input/output class (E8 admits it); if not, it fails with that class. In both cases, when the close returns no thread of the store is left running. |
| E13 | **Failures of the mechanism itself**: what writes the journal is started on open, before recovering (step 8 of the root parent's order). If that is not possible, the open fails with the out-of-memory class without touching the store (P13). If it stops working while the store is open, it is a failure of E8. |
| E14 | **Temporary mode**: it has no journal (J17); committing works as until now, in both modes: the commit is visible to every transaction opened after it returns (T2). |
| E15 | **Format `4`**: the journal changes what an earlier engine can read safely, so the current format version becomes `4` (D7 of the root parent). A store of format `3` is of an earlier format even if it was closed without error, and before 1.0 it is not migrated. It refines P5, J3 and R5. |
| E16 | **Measurement**: the change includes a reproducible measurement of durable commits per second with 1, 4 and 16 threads committing at the same time, with direct write and with the alternative of E6, on Windows and on Linux. It sets no threshold (D5 of the root parent); its results go in the closing report. |

*Annotation of 2026-10-24 from free space ([free-space.prd.en.md](free-space.prd.en.md)): E15, the current format
version becomes `5`; a store of format `4` is of an older format (decision F11).*

*Annotation of 2026-10-25 from trees and key operations ([trees-keys.prd.en.md](trees-keys.prd.en.md)): E15, the current format version becomes `6`; a store of format `5` is of an older format (decision A13).*

*Annotation of 2026-10-27 from large values ([trees-large-values.prd.en.md](trees-large-values.prd.en.md)): E15, the current format version becomes `7`; a store of format `6` is of an earlier format (decision V9).*

### Starting state

Verified on 2026-10-23 in the code:

| Aspect | Situation |
|---|---|
| Commit | The whole commit runs in the committing thread, in this order: it saves the pages in the temporary area, writes the journal record, waits for the disk (only in durable mode), makes the changes visible, yields the turn and then tries to transfer to `krummi.data`. While it waits for the disk it holds the journal exclusively, so no other commit advances. Each commit is one record; there are no groups. |
| Deferred mode | It writes the record without waiting for the disk. The transfer forces the journal to be flushed to disk when there are 256 commits or 1 MiB pending, and the deferred commit that causes it waits for that write before returning. |
| Making durable (J11) | It flushes the current journal file to disk; it does not enter the write request queue and is ordered with the commits only through exclusive access to the journal. |
| Journal files | They are opened as normal files, with the system cache; there is no direct write anywhere. They grow as records are added, they are not reserved in advance, and the next one is used on reaching 16 MiB. Each record carries its integrity check and up to which commit the journal was durable when it was written, is compressed when that takes less space and is padded with zeros to a multiple of 4 KiB. |
| Threads | The store creates no thread; the transfer to `krummi.data` runs in the committing thread, after yielding the turn. |
| Failures | A failure when flushing the journal to disk leaves the store not accepting writes; the waiting requests and the new ones fail with the input/output class. Lack of space and of memory do not leave it like that. |
| Tests | There is simulation of power cuts and of failures when writing and when flushing to disk, but it only acts on the thread that activates it. |
| Measurement | There is no performance measurement in the repository. |
| References | PostgreSQL and InnoDB group into a single log write the commits that wait at the same time. Voron writes the journal with direct write and checks on open whether the system supports it; CanopyDB offers it as an option disabled by default, with check and alternative. |

## Actors

- **Integrating developer**: commits from several threads, in durable or deferred mode, and handles errors by their
  class. Does nothing different from before.
- **Maintainer**: runs the measurement of E16 on Windows and on Linux.

## Flow

1. A thread commits a write. The commit does what can fail, including the reservation in the journal (E2, E7).
2. The commit passes its point of no return: it enters the journal queue and yields the turn. The next write
   starts and sees it (E2, E3). A deferred one returns here (E5).
3. The commits in the journal queue are written together, directly (E4, E6).
4. When the group is durable, all its commits become visible to reads, and those in durable mode return without
   error (E3, E4).
5. On close, what remains in the journal queue is written and the store stops (E12).

## Validations and messages

Those of the Journal and of its ancestors apply. This change adds no error classes or tool messages; it only
changes at what moment some situations are reported (E2, E7, E8, E11, E13). The only new situation is opening a
store without being able to start what writes the journal, which fails with the out-of-memory class (E13).

## Output

The change writes nothing to the screen and does not change the shape of the store information (P12); the format
version it reports becomes `4` (E15).

On disk, the folder has the same names as in the Journal (J2). What changes is that each journal file has its final
size from the moment it appears (E7): while it is open, a store with a single small commit takes up on disk, besides
`krummi.data`, a complete journal file, and the lack of space can appear earlier than today, when reserving that
file.

## Reuse

- The journal, its organization in numbered files, the integrity check and the compression of each commit, the
  rule of durable according to the journal and the recovery (J2, J8, J9, J16).
- The queue of write requests and its strict order of arrival (T3).
- The rule of a store that does not accept writes and who reports its failures (T12, J13).
- The simulation of crashes, power cuts and failures of the Journal tests, extended to what the journal writes.

## Out of scope

- Merging several transactions into one, like the transaction merging of RavenDB (out of scope of the root
  parent): each commit remains a transaction with its own turn.
- Several write transactions open at the same time (G4).
- Reads seeing commits that are not yet durable, in either of the two modes.
- Public options to choose direct write or the alternative, or to tune the grouping.
- Informing the application whether direct write is used, how many commits were grouped or whether a deferred one
  is already durable.
- A wait limit for a commit to be durable, cancelling that wait, or detecting a disk that does not respond.
- Reducing the window of J9 within the last group (E9).
- Direct write in `krummi.data` or in `krummi.temp`.
- Storing in the journal only the differences of each page, like Voron.
- Detecting disks that ignore the write to the physical medium (G2).
- Performance thresholds (module 6).
- An asynchronous API (root parent).

## Left to the implementation

- When a group starts being written and whether it waits a little for more commits to arrive, as long as no commit
  in the journal queue waits without being written, and without treating the two modes differently (E5).
- The size of the journal queue from which a commit waits (E2).
- The usual size of the journal files (E7).
- How it is checked whether the system supports direct write and which system calls request it (in the proposal,
  `FILE_FLAG_NO_BUFFERING` with `FILE_FLAG_WRITE_THROUGH` on Windows, and `O_DIRECT` with `O_DSYNC` on Linux), and
  how it is flushed to disk in the alternative.
- Which thread writes the journal and which one transfers to `krummi.data`, as long as J5 and J13 hold.
- The command that runs the measurement of E16, its workloads and the shape of its report.
- How the tests stop the journal write and simulate crashes, power cuts, panics and failures in it, and which
  points after the point of no return a commit has.

## Acceptance criteria

The test conditions of the Journal apply: the criteria that read or write pages are internal tests of the library,
and those that need to know how many writes the journal made or the size of its files use the knowledge of the
implementation itself. To hold commits between their point of no return and their durability, or to form a known
group, the test stops the journal write, waits for the commits to be in the journal queue and resumes it; the tests
also know when a group is durable and force the alternative of E6. "Without waiting" and "at once" mean, as in
Transactions, that the call returns while what would block it remains held.

The criteria of the Journal, of Transactions and of the root parent still apply, and those of the Journal are
repeated with direct write and with the alternative, with these changes:

- AC-J7: killing the process after the last deferred one returns leaves a prefix (E5).
- AC-T19 and the panic of AC-J14: they apply only to the points before the point of no return; in AC-J14, "after
  starting to write the journal" reads "before its point of no return". For the later points AC-E6 applies: the
  interrupted commit is there on reopen.
- AC6 of the root parent, AC-T3 and AC-T5: "a read opened after the commit returns" sees its changes in durable
  mode; in deferred mode, after its group is durable (E3).
- AC-J3 (third part) and AC10 of the root parent: the intact commit that follows the damaged one is in a later
  group; the discard without error applies to damage in the last group (E9).
- AC-J9: the commit without changes finishes without error if the write did not see commits that failed (E11).

And in addition:

1. **AC-E1** — Given a commit in durable mode held after its point of no return, when another thread requests a
   write, then it gets it without waiting and sees its changes; a read opened at that moment does not see them; and
   when the held one returns without error, a read opened afterwards by the same thread sees them (E2, E3).
2. **AC-E2** — Given 8 commits in durable mode from 8 threads held in the journal queue, which fit in the current
   journal file, when it resumes, then all return without error, the journal wrote them in a single write, a read
   opened before resuming sees none of them and one opened after the first one returns sees them all; and given 16
   threads that each commit 1,000 writes in durable mode at the same time, all return without error, there were
   never two writes open at the same time and, after killing the process and reopening, all are there (E4, G4).
3. **AC-E3** — Given a known sequence of commits in both modes, which the test groups into groups of one, two and
   three, when a power cut is simulated at each point where the journal is written or flushed to disk, then on
   reopen it opens without error and the set of commits present is a prefix of the sequence that includes all
   those in durable mode that had returned without error and all the groups before the one being written (E8, E9).
4. **AC-E4** — Given a commit that needs a new journal file, with no disk space to reserve it and with another
   write request waiting, when it is committed, then it fails with the disk-full class, the write that gets the
   turn does not see its changes, and when space is freed a following commit finishes without error without
   reopening; and if the process is killed after the failure, the next open opens without a damage error (E2, E7).
5. **AC-E5** — Given a deferred commit that has already returned and three commits in durable mode held in the
   journal queue, and an open write that sees their changes, when the write of that group fails, then the three
   fail with the input/output class, the open write fails when committed with changes with that class, the
   following write requests also fail, and a new read sees none of the four; and on reopening without the failure,
   those present are a prefix of the four and all the earlier ones are there (E8).
6. **AC-E6** — Given a commit in durable mode that the test interrupts with a panic at each point after its point
   of no return, when its group is durable, then a new read sees its changes and, on reopen, they are there (E2).
7. **AC-E7** — Given the journal write stopped, with a commit in durable mode held in the journal queue, when
   another thread gets the turn and commits a deferred one, then it returns without waiting; the following write
   sees both; a read that this thread opens after the deferred one returns sees neither; and when it resumes and
   the group is durable, a new read sees both (E3, E5).
8. **AC-E8** — Given a deferred commit and then one in durable mode held in the journal queue, when it resumes,
   then the journal writes them in a single write, and a simulated power cut after the durable one returns keeps
   both; and given a single deferred commit, a simulated power cut after its group is durable, with no other
   operation, keeps it (E4, E5).
9. **AC-E9** — Given a store with a commit with changes, when others that fit in the same journal file are
   committed, then the size of the file does not change; and a commit larger than the usual size of a file
   finishes without error and is recovered after a crash (E7).
10. **AC-E10** — Given deferred commits held in the journal queue, when the store is closed and the test resumes
    the write during the close, and in another test when it is ceased to be used without closing it, then the close
    does not return until they are written, the folder is left with only `krummi.data` and on reopen all are there;
    and with a simulated failure in the write of the group that stops occurring before the close, the close
    finishes without error and on reopen all the deferred commits are there; and with the failure persistent, the
    close fails with the input/output class and the next open without the failure opens with a prefix of them (E12).
11. **AC-E11** — Given a write without changes that saw a commit in durable mode held in the journal queue, when it
    is committed in durable mode, then it does not return until the held one is durable; if its group fails, it
    fails with the input/output class; and in deferred mode, or with no pending commits, it finishes at once
    without error (E11).
12. **AC-E12** — Given a commit in durable mode held in the journal queue, when another thread calls the operation
    of J11, then it does not return until the write resumes and the held one is durable (E10).
13. **AC-E13** — Given a simulated failure when starting what writes the journal, when the store is opened, then
    it fails with the out-of-memory class and the store is left identical byte for byte; and given a simulated
    failure of that mechanism with commits waiting, then they fail with the input/output class, the following
    requests also, and on reopen all those that had returned without error are there (E13).
14. **AC-E14** — Given a valid store whose header carries format `3`, when it is opened, then it fails with the
    different-format class as an earlier format and is left identical byte for byte; and the information of a new
    store gives format `4` (E15).
15. **AC-E15** — Given the closing report, when it is reviewed, then it contains the commits per second of the
    measurement of E16 with 1, 4 and 16 threads, with direct write and with the alternative, on Windows and on
    Linux (E16).
16. **AC-E16** — Given two groups of three commits in durable mode that returned without error, in a store closed
    by a crash without transferring, when one byte of the second commit of the last group is altered, then it opens
    without error with the first group and the first commit of the last one, without the other two; and when one
    byte of a commit of the first group is altered, the open fails with the damage class and the store files are
    left identical byte for byte (E9).
17. **AC-E17** — Given a commit waiting for room in the full journal queue, with commits held in it, when another
    thread requests a write, then it does not get it while the first one waits; and when a simulated failure when
    flushing `krummi.data` to disk (J13) leaves the store not accepting writes during that wait, the commit fails
    with the input/output class without applying anything, and when the held ones resume they are written and a new
    read sees them (E2).
18. **AC-E18** — Given a store in temporary mode, when a deferred one is committed, then a read opened after it
    returns sees it (E14).

## Annotations to other PRDs

This PRD changes or refines rules of earlier documents, which carry the corresponding annotations:

- Journal, J2: the journal files have their final size from the moment they appear (E7).
- Journal, J4: the commit yields the turn at its point of no return, before being durable, and from there the
  following writes see it, not the reads (E2, E3).
- Journal, J9 and AC10 of the root parent (and its annotation to G5, G6 and D10): the commits of a group count
  together, and the window in which damage is discarded without error moves from the last commit to the last
  group; a journal file without intact commits counts as empty (E7, E9).
- Journal, J10 and AC-J7: a deferred commit becomes durable when its group is, can be lost to a process crash until
  then, and reads do not see it until it is durable (E3, E5); its list of when a deferred commit becomes durable,
  and the point of *Left to the implementation* about how long it takes, are replaced by E5. "Returns without
  waiting to be durable" admits the wait for room in the journal queue or for the reservation of a new file (E2),
  and the failures when writing the journal are reported in the deferred commit only before its point of no
  return (E8).
- Journal, J11: it also waits for the commits that passed their point of no return and have not returned (E10).
- Journal, J12 and AC-J14: the lack of space and of memory are detected before the point of no return, and a later
  panic does not undo the commit (E2); a commit that failed with the input/output class may be there on reopen if
  the close manages to write it (E12), within what J12 already admits ("whole or absent").
- Journal, J13 and AC-J9, and Transactions, T12: a failure when writing a group is reported by its commits that
  are waiting, by the commit without changes in durable mode of a write that saw it (E11) and, if there is none,
  by the operations of J13; that commit without changes no longer always finishes without error (E8, E11).
- Journal, *Out of scope*: grouping commits from several threads into a single journal write enters with this
  change (E4).
- Transactions, T2: a write also sees the commits that passed their point of no return before it even if they are
  not durable, and a read only sees durable commits, also after a deferred one returns (E3).
- Transactions, T10 and AC-T19: a panic after the point of no return does not undo the commit (E2), and a commit
  without changes in durable mode can wait and can be the operation that finds an input/output failure (E11).
- Root parent, AC6, and Transactions, AC-T3 and AC-T5: in deferred mode, the read sees the changes when its group
  is durable, not on return (E3).
- Pager, P13: not being able to start what writes the journal on open is out of memory (E13).
- Pager, P5; Journal, J3; and the rename, R5: the current format version becomes `4` (E15).

## References

- Parent: [journal.prd.en.md](journal.prd.en.md) v1.0
- Ancestry: [krummi.prd.en.md](krummi.prd.en.md) v1.1 (root)
- Related: [pager.prd.en.md](pager.prd.en.md), [transactions.prd.en.md](transactions.prd.en.md),
  [rename-to-krummi.prd.en.md](rename-to-krummi.prd.en.md)
- Origin proposal: closing report of the journal follow-ups, `odd/tasks/journal-followups-report.md`
- PostgreSQL, *WAL configuration* (`commit_delay`, `synchronous_commit`): <https://www.postgresql.org/docs/current/wal-configuration.html>
- Voron, in the RavenDB repository: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron>

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| b7c9add8 | 2026-10-23 10:17 UTC+01:00 | 42 min | 0 min | 42 min | 23 min | 19 min | 7 |

<!-- prd-time: [{"session": "b7c9add8-aa47-43e9-8507-11b8b6891d58", "start": "2026-10-23T09:17:28.630000+00:00", "wall": 2549, "discarded": 0, "agent": 1403, "human": 1145, "interventions": 7, "idle_minutes": 10}] -->
