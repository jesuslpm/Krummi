# Remembered Checks of What the Engine Writes — PRD

**Version**: 1.0
**Date**: 2026-11-16
**Parent document**: [Write Checks — PRD](pager-writer-checks.prd.en.md) (v1.0)

> English translation of [pager-written-pages.prd.md](pager-written-pages.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Make updates cheaper by no longer repeating two checks on pages that the engine itself has just written and whose
content it knows:

- the **integrity check** (G6) of a page that the move (J5) writes in its place in the data file or, in temporary mode,
  in the store's memory. Today that page stops being remembered before it is written (I4), and the first later read
  computes its code again and, since the code is different, checks its whole layout;
- the **layout check** (C1) of a leaf in which a write only overwrote values stored in the leaf with others of the same
  length. Today the version that this write publishes is new for all the checks (H2) and the first read by another
  transaction checks it in full.

With this change, the page that the move writes stays remembered with the code that the commit sealed in it, as Voron
does, and a leaf in which only values of the same length changed keeps the layout check it already had.

**Why**: in `tabla-actualizacion` (W28) Krummi stands at 0.88–0.89 times Voron on Windows (benchmark reports of
2026-11-16), and almost every leaf read of a W28 update checks the whole leaf again (see
[Starting state](#starting-state)). Voron repeats neither of the two checks.

**Why now**: it is what remains of the W28 gap after the run of the write margin. The user decided on 2026-11-16 to
tackle the two checks and accept their risks (Q1, Q3, Q4).

This PRD is a change to the write checks. It inherits everything from the parent and its ancestors; only what the
engine remembers of what it itself writes changes here. What changes earlier rules is declared in its decisions and
annotated in the affected documents (see [Annotations to other PRDs](#annotations-to-other-prds)).

### Product decisions

| ID | Decision |
|---|---|
| Q1 | **Integrity of what the move writes** (changes I1, I4 and I5 of the remembered integrity; user's decision of 2026-11-16, «Sí, como Voron»). The pages that I4 stops remembering before the move writes them (whole pages, with the code that the commit sealed in them; not the header, which has its own check, P6), except those of a value stored off its leaf, stay remembered as checked, with that code, as soon as their bytes are written in their place; this also holds for the pages that the file gained by growing. A later failure when taking those bytes to disk (J13) does not make them stop being remembered, because what is read is what was written. If the write of a page fails or is left half done (G12), that page stays unremembered, as I4 says; the other pages of the same move write that were written stay remembered. The pages of a value stored off its leaf keep following I4 unchanged. What the recovery writes on opening does not change: the memory starts empty afterwards (I5). |
| Q2 | **Layout of what the move writes** (changes C1, C2 and C3 of the remembered layout check and U1). When the move writes a node whose version, at that moment, already passed the layout check in the temporary area (U1) or keeps it by Q3, the page stays remembered also in the shared memory of remembered nodes (C2), with the same code as Q1; the bytes are the same ones that were checked. If the version had not passed it, nothing is remembered and the first read from the data file checks it, as today; a later check of the version in the temporary area adds nothing to C2. The entries that Q2 adds share C2 with the others and can be discarded (C5); if C2 later discards one of them, the next read checks that page again. What this costs the reads is measured by Q7. |
| Q3 | **Leaves in which only values of the same length change** (changes H2 of the check of the nodes after commit and U1; user's decisions of 2026-11-16, «Sí, solo valor igual» and «Sí, se encadena»). It applies to the leaves of all the trees. A leaf meets Q3 in a write transaction when everything the transaction did in it, from when it obtained its copy until it committed, was to overwrite the value of entries it already had, stored in the leaf, with another of the same length, once or several times and even if the bytes are equal; and when the page or version from which it obtained the copy, at the moment of obtaining it, had its layout checked in that transaction or remembered (C2, U1, or kept by Q3); a copy obtained without that check does not meet Q3. Any other change to the leaf in the transaction disqualifies it, even if the final result looks the same: adding or removing an entry (also removing and adding back the same key), a value of another length, a value stored off the leaf, a split or a merge. The version that the commit of a leaf that meets Q3 publishes keeps the layout check: no read by another transaction checks it again, neither in the temporary area nor, after the move, in the data file (Q2). An overwrite with the same bytes may publish no new version (Y1); then there is nothing to keep. The other checks of each read of a node (C1) stay the same. |
| Q4 | **Debug builds**. In debug builds, every read by another transaction of a leaf that keeps its check by Q3, in the temporary area or in the data file, also checks its layout, outside the checks that the criteria count, like the annotation of 2026-11-09 to H1; if it fails, the read fails with the damage class and the number of its page; if it passes, nothing more is remembered. Q1, Q2 and Q3 hold equally in both builds. |
| Q5 | **Resulting guarantee** (extends the exception of G6 of the root parent and the residual risks of I8, S3, U2 and H3, annotated in them). It still holds that all damage done while the store is closed is detected (AC10): on opening, the memory of what is remembered is empty (I5). Three things change, in the face of failures that the engine does not cause (P16) or errors of the engine itself: (a) if the bytes of a page that the move wrote change later on disk or in the store's memory without the engine writing them, no transaction detects it until reopening; (b) if the bytes of a version change in the temporary area without the engine writing them and the move writes it, the first read from the data file no longer detects it, as U2 said, until reopening; (c) if an error of the engine itself leaves a leaf inconsistent when overwriting a value of the same length, or a hardware or memory failure changes the bytes of a leaf while it keeps its check by Q3, in the temporary area or in the data file, no read detects it in production builds until reopening, however many Q3 commits chain it. In the three cases, a read may return data that is not what was written or end in a panic instead of a damage error (as in I8), and if a write copies that page and commits, the damage is sealed and is no longer detected on reopening either (as in S3). It is accepted as a residual risk, as in Voron; it also extends that of C7. |
| Q6 | **No visible changes**: as S4. There are no new situations, error classes, error data or options; what is read and what is committed is the same as today; the format version does not change. What Q1 to Q3 remember uses the memories that already exist (I6, C5, U1) and reserves no memory that could run out. |
| Q7 | **Performance** (acceptance). Comparison of Krummi alone, the commit before the change against the change, in the same session, with the measurement method of the performance plan, on Windows and on Linux; the workloads of under one second per repetition and the multi-threaded ones, in at least ten rounds, and the others in at least three. A workload improves when its median after exceeds that of before and exceeds it in most of the rounds; it worsens when its median after is below that of before and below in most of the rounds; otherwise it stays the same. The change is accepted when W28 improves on both platforms and none of the workloads that write or read alongside a write (W1–W6, W12–W15, W17, W19, W21, W23–W29) nor of the read workloads that use C2 the most (W7, W22, W26) worsens. The opening (W16) is not measured: it does not read pages that the move has written. If this is not met, the closing report says so and the user decides; not meeting the threshold (K7) does not prevent closing the change. |

### Starting state

Verified on 2026-11-16 in the code and in the measurement of the write-margin run:

| Aspect | Situation |
|---|---|
| Move writes | Before writing a page in its place, the engine stops remembering it (I4); the first later read computes its code again and, since the code is different, checks its whole layout (C2 remembers by page number and code). |
| Temporary-area versions | The commit publishes them with no remembered layout check (H2); the first read by another transaction does it in full and remembers it in the version (U1). |
| Table row updates | An update of a row that does not change its length (modulo 7) overwrites its value in the leaf without changing the rest of the leaf. |
| W28 | Per update: two branch reads, already remembered, and one leaf read: a version read for the first time (54%), a page that the move has just written (43%), a page that the write itself changed (3%), an already remembered leaf (0.1%). Ratio with Voron: 0.88–0.89 on Windows and 0.96 on Linux (benchmark reports of 2026-11-16). |
| Voron (reference) | According to the RavenDB 7.2.7 code reviewed by the write-margin run: it checks the code of each page of the file once per process, does not check again the ones it wrote itself and does not check the ones it reads from the temporary area or the journal; it does not check the layout of any page in production. |

## Actors

- **Integrating developer**: reads and writes as until now; nothing they use changes.
- **Maintainer**: runs the tests, also on a production build, and the measurements of Q7 on Windows and on Linux.

## Flow

1. A write overwrites values of the same length in a leaf and commits: the published version keeps the layout check
   (Q3).
2. Other transactions read that version in the temporary area without checking its layout (Q3, U1).
3. The move writes it in its place and, when that write finishes, the page stays remembered with its code and its
   layout (Q1, Q2).
4. Later reads from the data file neither compute its code nor check its layout, until the store is closed, until the
   move writes it again or until C2 discards it.

## Validations and messages

There are no new situations, error classes or error data. What the engine ceases to detect is in Q5.

## Output

Nothing that an operation returns changes in form (Q6).

## Reuse

- The memory of remembered pages of the remembered integrity (I1, I6, I7) and the shared memory of remembered nodes
  (C2, C5), unchanged in form and size.
- The remembered check of the temporary-area versions (U1).
- The rules of the move (J5, T14) and of the writes that fail (G12).
- The measurement method of the performance plan and the Krummi-alone measurements of the write-margin run, for Q7.

## Out of scope

- Remembering the pages of a value stored off its leaf that the move writes (they keep following I4).
- Keeping the layout check of a leaf in which something other than values of the same length changed, or of a branch.
- Changing what the opening, the recovery, the store information query, backup, compaction, migration, verification or
  the journal check and write, and the check of the header (P6).
- The compression of leaf pages (5.6) and encryption (module 9): they decide in their PRDs whether they use what is
  remembered.
- Detecting that another program, or a hardware or memory failure, changes a page that the move wrote (Q5, P16).
- A public option to turn this change off, and statistics of what is remembered in the store information.
- The other pending optimizations of the performance plan: not compressing when it does not help, advancing between the
  entries of a range, the cache of recent leaves and the alignment of the loops.

## Left to the implementation

- How the commit knows that a leaf meets Q3 (for example, a mark on the copy that any other change clears) and how it
  passes to the published version.
- At what exact moment of the move's write of each page it is remembered, within Q1.
- How the tests count the integrity and layout checks, how they alter bytes and how they simulate the failure of a
  move write, without a public option.
- The workloads and repetitions of Q7 besides those it names.

## Acceptance criteria

Those of the ancestors apply and must keep passing unchanged, except those this PRD replaces or specifies (see
[Annotations to other PRDs](#annotations-to-other-prds)), including AC10 of the root parent. AC-Q1, AC-Q2, AC-Q5, AC-Q6
and AC-Q9 are checked on a store on disk and in temporary mode; AC-Q3 and AC-Q4, which simulate a write failure or
reopen the store, only on disk. AC-Q7 holds only in debug builds and AC-Q8 only in production ones; the others, in both,
with the checks of Q4 left out of the counts. The tests use knowledge of the implementation to count the checks, to
place pages in the data file or in the temporary area, to force the move, to read a page from the data file and to
alter bytes. The counts are made with a single thread and with room in C2 to remember what is read. AC-Q10 is a
measurement, not a test.

1. **AC-Q1** — Given a leaf of a tree in the data file, when a write adds an entry to it and commits, a read
   transaction reads it from the temporary area and the move writes it in its place and removes it from the temporary
   area, then a new read transaction reads it without computing its integrity or checking its layout and returns the
   entry; and after closing and reopening the store (on disk only), its first read computes its integrity and checks
   its layout (Q1, Q2, I5).
2. **AC-Q2** — Given a page of the free-space map and a branch that a write changes, and a value stored off its leaf
   that it saves, when it commits and the move writes them in their place, then the first later read of the map page
   and of the branch does not compute its integrity, and that of the value stored off its leaf does (Q1, I4).
3. **AC-Q3** — Given a page that the move is going to write, when the write of its bytes in its place fails with a
   system error (simulated), then a read of that page from the data file by the test computes its integrity (Q1, G12).
4. **AC-Q4** — Given a leaf that the move wrote and that stayed remembered, with no versions pending to move, when the
   test alters, with the store open, a byte of the value of one of its entries without breaking its layout, then a read
   transaction returns the altered byte without failing; and after closing and reopening the store, the first read of
   that leaf fails with the damage class and the number of its page (Q1, Q5).
5. **AC-Q5** — Given leaves with several entries in a normal tree, in a fixed-size one and in the table of a row update
   that does not change the row's length, when a write overwrites in each two values with others of the same length,
   one of them twice, and commits, and another write does the same again and commits, then the first read of each
   published version by another transaction, from the temporary area and, after the move, from the data file, counts no
   layout check and returns the new values (Q2, Q3).
6. **AC-Q6** — Given a leaf as in AC-Q5, when a write overwrites one of its values with another of the same length and
   also, in the same transaction, (a) adds an entry to that leaf, (b) removes one, (c) removes and adds back the same
   key with the same value, (d) changes another value to one of another length, (e) saves a value off the leaf, (f)
   splits it or (g) merges it, then the first read of the committed version by another transaction checks its layout
   (Q3, H2).
7. **AC-Q7** — Given a leaf as in AC-Q5 whose copy the test alters, breaking its layout, after overwriting a value with
   another of the same length, when the write commits without reading it again and another transaction reads it, then,
   in debug builds, the read fails with the damage class and the number of its page (Q4).
8. **AC-Q8** — Given AC-Q1, AC-Q5 and AC-Q6, when they are run on a production build, then their counts are the same
   (Q3, Q4).
9. **AC-Q9** — Given four threads with read transactions that read keys of a tree, at least a thousand times each,
   while a write commits overwrites of the same length and other changes and the move writes them, then each read
   returns the data of its snapshot and none fails (Q1–Q3, I7).
10. **AC-Q10** — Given the measurements of Q7, when they are compared, then W28 improves on both platforms and none of
    the workloads that Q7 names worsens; and the closing report includes those comparisons and the full runs of the
    benchmark on both platforms (Q7).

## Annotations to other PRDs

This PRD changes rules of earlier documents, which carry the corresponding annotations:

- Root parent, G6: the exception of I8, extended by S3, U2 and H3, is extended with Q5. AC10 does not change.
- Remembered integrity: I1 («solo se recuerdan comprobaciones superadas») has the exception of Q1: a page that the move
  writes stays remembered without computing its integrity, because its content is what the engine sealed; AC-I1 is
  specified: the first read of a page that the move wrote in the current opening does not compute its integrity, and the
  values stored off their leaf and the other pages stay as it says. I4 now says that the page that the move writes stays
  remembered when its write finishes (Q1), except those of a value stored off its leaf; I5 («las páginas que el archivo
  gana al crecer empiezan sin recordar») and the Out of scope item about them cease to hold for those that the move
  writes; step 3 of the Flow, AC-I4 («su integridad se comprueba una vez») and AC-I10 are specified with Q1; AC-I5,
  AC-I7 and AC-I9 («una página que todavía no se leyó», a page damaged with the store open) are specified: the new
  content or the page is not one that the move wrote in that opening and that stayed remembered (Q1); the residual risk
  of I8 is extended with Q5.
- Write checks: the residual risk of S3 is extended with Q5. AC-S4 («una página del archivo de datos que nadie leyó en
  la apertura se comprueba en su primera lectura») and the temporary-mode part of AC-S5 are specified: except the pages
  that the move wrote in that opening (Q1). AC-S6 does not change.
- Remembered layout check: C1 and C2 («solo se recuerdan comprobaciones superadas» in a read) have the exception of Q2,
  which remembers in C2 what passed the check as a temporary-area version or keeps it by Q3; the last sentence of C3
  («leer del archivo de datos un contenido que antes se leyó del área temporal no lo encuentra recordado»), step 4 of
  its Flow, AC-C2 and AC-C3 are specified with Q2; the residual risk of C7 is extended with Q5. Q2 relies on C8: what
  is remembered holds as long as the bytes do not change.
- Remembered check of temporary-area versions: U1 («no se consulta ni se guarda en C2» and «no sirve para otra versión
  de la misma página, más nueva») has the exceptions of Q2 and Q3; U2 ceases to hold in what it says about an altered
  version that the move writes (Q5); step 4 of its Flow, AC-U3 and AC-U6 are specified with Q2 and Q3.
- Check of the nodes after commit: H2 («La versión que publica la confirmación es nueva: nada de lo comprobado de la
  página de la que se copió vale para ella») has the exception of Q3; AC-H2 is specified: the change that the test
  makes is not a Q3 overwrite; the residual risk of H3 is extended with Q5.

## References

- Parent: [pager-writer-checks.prd.en.md](pager-writer-checks.prd.en.md) v1.0 (S2–S4)
- Ancestors: [pager-integrity-once.prd.en.md](pager-integrity-once.prd.en.md) v1.0 (I1–I9),
  [pager.prd.en.md](pager.prd.en.md) v1.0 (P6, P10, P12, P16), [krummi.prd.en.md](../krummi.prd.en.md) v1.1 (G6, G12,
  AC10)
- Related: [pager-commit-checks.prd.en.md](pager-commit-checks.prd.en.md) (H1–H3),
  [pager-version-checks.prd.en.md](pager-version-checks.prd.en.md) (U1, U2),
  [trees-node-check-cache.prd.en.md](../trees/trees-node-check-cache.prd.en.md) (C1–C5),
  [journal.prd.en.md](../journal/journal.prd.en.md) (J5),
  [transactions.prd.en.md](../transactions/transactions.prd.en.md) (T14),
  [benchmark.prd.en.md](../benchmark/benchmark.prd.en.md) (K7),
  [journal-page-diffs.prd.en.md](../journal/journal-page-diffs.prd.en.md) (Y1),
  [tables.prd.en.md](../tables/tables.prd.en.md) (the row updates of AC-Q5)
- Measurement: write-margin run, [write-margin.md](../../../../odd/done/pager/write-margin.md) (L3) and its
  [closing report](../../../../odd/done/pager/write-margin-report.md)
