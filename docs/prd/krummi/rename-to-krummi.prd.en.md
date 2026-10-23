# Product Rename to Krummi — PRD

**Version**: 1.0
**Date**: 2026-10-23
**Parent document**: [Krummi, an embedded storage engine inspired by Voron — PRD](krummi.prd.en.md) (v1.1)

## Objective and context

Rename the product from **MuninnDB** to **Krummi** in everything that people who use or maintain it see: the library
packages, the files that make up a store, the identification mark, the error messages, the documentation and the PRDs,
the tools of the working copies, the remote repository, the folder of the main copy and its memory project. The
behavior of the engine does not change in any other way.

**Why**: the name MuninnDB is already used by another product in the same category. According to the user's check of
2026-10-23, <https://muninndb.com/> is "MuninnDB" by Scrypster, a memory database for AI, published on PyPI as
`muninndb` with a source-available license. There is a risk of confusion and of trademark problems. The user chose
**Krummi**, with no suffix: it is the Icelandic nickname of the raven and it keeps the nod to Voron ("raven" in
Russian).

**Why now**: the product has not been published yet and has only three modules (Pager, transactions and journal).
Each new module adds names, files and texts that would have to be changed later.

This PRD inherits everything from the parent, from the [Pager](pager.prd.en.md) (P1–P17), from
[Transactions](transactions.prd.en.md) (T1–T18) and from the [Journal](journal.prd.en.md) (J1–J17); here only names,
the mark and the format number change. What changes from earlier rules is declared in its decisions and annotated in
the affected documents (see [Annotations to other PRDs](#annotations-to-other-prds)).

### Product decisions

| ID | Decision |
|---|---|
| R1 | **Name**: the product is called **Krummi**, with a capital initial and no suffix, in all texts, including the messages of the command-line tool and the errors of the library. Package and file names are in lowercase (R2, R3). |
| R2 | **Library packages**: the package that the integrating developer adds is called `krummi`, and the two internal ones, `krummi-pager` and `krummi-tx`. The names `muninndb`, `muninndb-pager` and `muninndb-tx` cease to exist, with no compatibility packages under the old name. Public types and functions keep their current names. |
| R3 | **Files of a store**: the data file is called `krummi.data`; the one for atomic creation, `krummi.data.new`; the journal ones, `krummi.journal.` followed by the same 19-digit number; and the subfolder of the temporary area, `krummi.temp`. They replace the `muninn.*` names in all the rules and criteria of the Pager, Transactions and the Journal, with the same rules. |
| R4 | **Identification mark**: the mark of P4 is replaced by a new one. A `krummi.data` that starts with the MuninnDB mark does not start with the Krummi mark. |
| R5 | **Format `3`**: the change of names and mark is incompatible (D7 of the parent), so the current format version becomes `3`. An intact Krummi store with a lower number, including `0`, is of an earlier format; those numbers appear only in tests. Refines P5 and J3. |
| R6 | **MuninnDB stores**: a folder with the files that MuninnDB left (`muninn.data` and, if it was closed by a crash, `muninn.journal.*` files and `muninn.temp`) does not contain a Krummi store: it has no `krummi.data` and it has other entries (P7), so the open fails with the error class for not a store, without reaching the format check, and touches nothing, not even the pending journal. Before 1.0 there is no migration and the data is recreated (D7). The treatment of MuninnDB leftovers changes: a lone `muninn.data.new`, which used to be an interrupted creation and was deleted (P8), is now a foreign entry and the folder is not a store. |
| R7 | **Documentation**: the README, the library documentation, the project instructions, the methodology, the guide to the working copies, the project's skills and hooks, continuous integration and the current PRDs (this one, the parent and those of modules 1 to 3, with their translations) now say Krummi and use the names of R2 and R3. The README explains the new name (Krummi, the Icelandic nickname of the raven, and the nod to Voron) in place of the explanation of Muninn, without naming the old one. In the current PRDs the titles, rules, flows, messages, outputs and criteria change, and each one carries an annotation that records the change, without changing its version. Their starting state sections, their earlier annotations and their writing time are history: their text does not change, and the names and format numbers in the earlier annotations are read according to R3 and R5; only their links are updated (R11). The execution documents and closing reports of earlier changes are also history and keep the name of their time. |
| R8 | **Working copies of the repository**: the copies of a change are called `krummi-<n>` (their folder and their memory project), instead of `muninndb-<n>`. Today no copy is open, so there are no copies with the old name to keep. |
| R9 | **Remote repository**: the GitHub repository is renamed `Krummi` (<https://github.com/jesuslpm/Krummi>). This change is executed in the main copy, without a working copy, and renames the repository at the close, with the user's explicit authorization given at the start of the run, which names the repository, the operation and the user's GitHub session to use. Order of the close: closing report and merge into the main branch, as in any change; then, rename on GitHub, change of the remote's address in the main copy and move of the memory (R13); and last, a commit on the main branch that adds to the closing report the result of those three steps. If the rename fails, the run does not change the remote's address and does not move the memory, and the report declares them pending. Nothing is pushed. Apart from the leftovers of R10, no file in the repository links to the old address; GitHub redirects it to the new one. |
| R10 | **Allowed leftovers of the old name**: after the change, the content of the tracked files in the repository names MuninnDB or `muninn` only in: (a) the execution documents and closing reports of changes before this one; (b) the execution document, the phase log and the closing report of this change; (c) this PRD and its translation; (d) in the current PRDs, the text of their starting state sections, their annotations and their writing time (R7); (e) the tests that check R4 and R6. No tracked folder or file path contains `muninn`. |
| R11 | **PRDs**: the PRD folder is renamed `krummi` and the root PRD, `krummi.prd.md`, with its translation `krummi.prd.en.md`; the other PRDs keep their file names. All the links in the current PRDs, including those in their historical sections and those in this PRD, and those in the current documentation, point to the new paths. The links in earlier execution documents and closing reports to the old paths are left broken: they are history and are not rewritten. |
| R12 | **Folder of the main copy**: the main copy moves to `C:\projects\Krummi`, and the project instructions, the guide to the working copies and the methodology name that path. The user moves the folder after closing the run, with the editor closed; until then the documentation already names the new path. |
| R13 | **Memory project**: the main copy detects the memory project by the address of the remote repository, so after R9 it uses the project `krummi`. With the user's authorization, given at the start of the run, to read `muninndb` and write to `krummi`, the run saves in `krummi` a copy of every memory that `muninndb` has at the time of copying, including those saved during this run, with the same type, title, content, scope and topic key; the copies have a new identifier and date, and the sessions are not copied, although their summaries are, since they are memories. Repeating the copy after a partial failure copies only the missing ones. The closing report gives the number of memories copied. The `muninndb` project stays as it was, as an archive. |

### Starting state

Verified on 2026-10-23 on the repository:

| Aspect | Situation |
|---|---|
| Product | Modules 1 (Pager), 2 (transactions) and 3 (journal and recovery) implemented on the local main branch; according to the local reference, the remote repository (<https://github.com/jesuslpm/MuninnDB>) is 10 commits behind. Nothing has been published: the packages are marked as not publishable and have no license. |
| Packages | `muninndb` (the integrator's), `muninndb-pager` and `muninndb-tx`, in folders with those names. |
| Store on disk | Format `2`. Files `muninn.data`, `muninn.data.new`, `muninn.journal.<19 digits>` and `muninn.temp`. The identification mark and the internal identifier of the journal records contain the old name. The engine recognizes a store only by the mark at the start of `muninn.data` (D10, P4). |
| Messages | The library errors, in English, say "MuninnDB store". The command-line tool does not exist yet (module 12): its messages in Spanish are only in the PRDs. |
| Occurrences | `muninn` appears, ignoring case, 619 times in 72 tracked files: 267 in the PRDs; 91 in 12 execution documents and closing reports; 173 in the code and the tests; and 88 in the README, the workspace manifests, the guide to the working copies, the methodology, continuous integration, the tools, skills and hooks of the working copies, their tests and the project instructions. One execution document and its closing report already cite the name Krummi. |
| Working copies | The tools, which work only on Windows, create `muninndb-<n>` copies with a `muninndb-<n>` memory project. No copy is open. The tools do not fix the path of the main copy; the project instructions, the guide to the working copies and the methodology name it. |
| PRDs | They are in the `muninndb` folder; the root is called `muninndb.prd.md`. The execution documents and closing reports link to that path 15 times, and the starting state sections of the three module PRDs link to the root. |
| Memory | The main copy is at `C:\projects\MuninnDB`. The memory tool reports that it detects its project, `muninndb`, by the address of the remote repository. The project has 21 memories, 7 sessions and no saved messages. The tool has no way to rename a project or to merge two with different names. |
| New name | According to the user's check of 2026-10-23, `krummi` was free on crates.io and a quick web search found no databases with that name. No trademark check has been done. |

## Actors

- **Integrating developer**: adds the `krummi` package instead of `muninndb` and sees the new name in the errors.
- **Operator**: finds the `krummi.*` files in the store folder and, when the tool exists, its messages with the new
  name.
- **Maintainer**: works with the packages, the documentation, the working copies, the remote repository and the
  memory under the new name; at the start of the run authorizes the rename of the repository and the copy of the
  memory, and at the end moves the folder of the main copy.

## Flow

1. The integrating developer declares the `krummi` dependency and uses the library as before.
2. New stores, and those opened afterwards, follow the rules of modules 1 to 3 with the files of R3, the mark of R4
   and the format of R5.
3. When opening a folder with a MuninnDB store, the open fails because it does not contain a Krummi store, touching
   nothing (R6).
4. When this change's run closes, the remote repository and the memory move to the new name (R9, R13); afterwards,
   the maintainer moves the folder of the main copy (R12).

## Validations and messages

The situations, rules and error classes are those of the parent and of modules 1 to 3. Only the file names (R3)
change and, in the messages, MuninnDB becomes Krummi; for example, the message of the row for a folder that is not a
store becomes:

```text
La carpeta <carpeta> no contiene un almacén de Krummi.
```

These rows specify how the existing rules apply to the leftovers of the old name:

| Situation | Rule | Error class |
|---|---|---|
| The folder has no `krummi.data` and has any `muninn.*` entry (`muninn.data`, `muninn.data.new`, `muninn.journal.*`, `muninn.temp`), alone or with others | It is not a store; nothing is touched and the pending journal is not recovered (R6, P7). | Not a store |
| `krummi.data` starts with the MuninnDB mark | It does not start with the mark (R4). | Not a store |

Inside the folder of a Krummi store, the `muninn.*` entries are foreign files (P7). The library errors, in English,
that today name the product now name Krummi; their exact text is still left to the implementation (D8 of the parent).

## Output

The output does not change in shape; only the names change. On disk, the folder of an open store with commits not yet
transferred contains, for example:

```text
krummi.data
krummi.journal.0000000000000000001
krummi.temp\
```

## Reuse

- All the rules of the parent and of modules 1 to 3: this change only replaces names, the mark and the format
  number.
- The earlier and later format check of D7, P5 and J3, which now compares with `3`.

## Out of scope

- The Krummi trademark check, the reservation of the name on crates.io and the publication of the packages: they are
  decided before the first publication, in the license and publication PRD that the parent already foresees.
- Opening, converting or migrating MuninnDB stores (D7: before 1.0 there is no migration), and a dedicated message
  that tells them apart from any other folder that is not a store.
- A compatibility `muninndb` package that re-exports `krummi`.
- The name of the command-line tool's executable and of its commands, which module 12 sets.
- Any other change to the on-disk format (page size, header, journal) or to the behavior of the engine.
- Rewriting the history of the repository: existing commits, branches and tags, and the execution documents and
  closing reports of earlier changes, including their broken links (R11).
- The description, topics and other data of the GitHub repository besides its name.
- Changing the text of the memories, deleting the `muninndb` memory project and copying its sessions (R13).
- The clones of the repository on other machines.
- What the editor stores tied to the path `C:\projects\MuninnDB` (the Claude Code session history and its automatic
  project memory), which is not moved to the new path (R12).
- Untracked files that are regenerated (the skill registry and the build folder).
- The file name of the PRDs other than the root.
- Making the tools of the working copies work on Linux.

## Left to the implementation

- The bytes of the new mark and its length (already left by the Pager), including whether they contain the text
  `Krummi`, and the internal identifier of the journal records (internal organization of the journal, already left
  by module 3).
- The English text of the library errors (D8), as long as it names Krummi and not MuninnDB.
- How the tests build the MuninnDB files to check R4 and R6.
- How to resolve the conflict warnings that the memory tool gives when saving the copies (R13), as long as no
  memory is lost or changed, and whether its pinned flag is kept.

## Acceptance criteria

This change adds no criteria of the parent or of modules 1 to 3: their tests and the full sweeps of AC10 keep
passing with the new names, mark and format version. AC9 is adapted to R5: earlier format with `0` and `2`, later
format with `4`.

And in addition:

1. **AC-R1** — Given a folder that does not exist, when it is opened, then it contains only `krummi.data`, which
   starts with the new mark, and the store information says format `3`; and with commits not yet transferred, the
   folder also contains `krummi.journal.` files followed by 19 digits and `krummi.temp`, which disappear on a close
   without error (R3, R5).
2. **AC-R2** — Given folders with the files that MuninnDB left (one with a `muninn.data` with the MuninnDB mark and
   format `2`; another with that `muninn.data` and `muninn.journal.*` files with commits; and three others whose
   only content is `muninn.data.new`, a `muninn.journal.0000000000000000001` file or a `muninn.temp` folder), when
   they are opened, then they fail with the error class for not a store and the content of the folders stays
   identical byte by byte (R6).
3. **AC-R3** — Given a `krummi.data` that starts with the MuninnDB mark and is otherwise a valid store, when it is
   opened, then it fails with the error class for not a store and stays identical byte by byte (R4).
4. **AC-R4** — Given a Krummi store whose folder also contains `muninn.data`, a
   `muninn.journal.0000000000000000001` file and a `muninn.temp` folder with a file, when it is opened, written to,
   committed and closed, and also when it is closed by a crash with commits not yet transferred and reopened, which
   recovers it, then those three entries stay identical byte by byte (R3, P7, J8, J14).
5. **AC-R5** — Given a new Rust project that declares the `krummi` dependency, when it creates a store with the
   library, commits a write and reopens it, then it compiles and reads what was written; the examples in the library
   documentation compile and pass; and the workspace has only the packages `krummi`, `krummi-pager` and `krummi-tx`
   (R2).
6. **AC-R6** — Given the library errors that today name the product (not a store and different format), when they
   are shown as text, then they contain `Krummi` and do not contain `muninn`, ignoring case (R1).
7. **AC-R7** — Given the repository after the change, when `muninn` is searched, ignoring case, in the content and in
   the paths of the tracked files, then it appears in no path, and in the content it appears only in the files of
   R10 (a), (b), (c) and (e) and in the other current PRDs; in these, every line that contains it is in a *Starting
   state* or *Elaboration time* section, or in an annotation (an italic paragraph or list item, or the *Annotations to
   other PRDs* section) (R7, R10).
8. **AC-R8** — Given the README, the project instructions, the guide to the working copies and the methodology, when
   they are read, then the README explains the name Krummi, and the others name the path `C:\projects\Krummi` and
   the copies `C:\wt\krummi-<n>` (R7, R8, R12).
9. **AC-R9** — Given the tools of the working copies, on Windows, when a copy of change `x` is created, then its
   folder and its memory project are called `krummi-x`, and their tests pass (R8).
10. **AC-R10** — Given the main copy at the close of the run, when the address of its remote repository is queried,
    then it is <https://github.com/jesuslpm/Krummi>, and GitHub reports that the repository `jesuslpm/Krummi`
    exists; or, if the rename failed, the address did not change and the closing report declares it pending (R9).
11. **AC-R11** — Given the current PRDs, the README, the project instructions, the methodology and the guide to the
    working copies, when their relative links and anchors are followed, then none is broken, and the root PRD is
    `krummi.prd.md` in the `krummi` folder (R11).
12. **AC-R12** — Given the main copy at the close of the run, when the memory project it detects is queried, then it
    is `krummi` and contains one memory with the same type, title, content, scope and topic key for each one that
    `muninndb` had at the time of copying, the report gives that number, and `muninndb` keeps its own; or, if the
    rename failed, the report declares the copy pending (R13).
13. **AC-R13** — The tests of all the packages and the full sweeps of AC10 pass on Windows x64 and on Linux x86-64
    with glibc (AC18); the Python tests of the hooks and skills pass on both, and those of the working-copy tools,
    on Windows.

## Annotations to other PRDs

The parent and the PRDs of modules 1 to 3, with their translations, carry an annotation of the rename and are
updated according to R7 and R11. The annotation also says that the file names and format numbers of the earlier
annotations are read according to R3 and R5. In particular:

- Parent: the product is called Krummi (R1); a MuninnDB store is not a Krummi store (R6).
- Pager, Transactions and Journal: all occurrences of the file names become those of R3, and the mark of P4 is
  replaced (R4).
- Pager, P5, and Journal, J3: the current format version becomes `3` (R5).
- Pager, P8: a lone `muninn.data.new` is no longer deleted as an interrupted creation; it is a foreign entry (R6).

## References

- Parent and root: [krummi.prd.en.md](krummi.prd.en.md) v1.1
- Modules: [pager.prd.en.md](pager.prd.en.md), [transactions.prd.en.md](transactions.prd.en.md), [journal.prd.en.md](journal.prd.en.md)
- Product with the old name: <https://muninndb.com/>

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| 5def7028 | 2026-10-23 08:13 UTC+01:00 | 24 min | 0 min | 24 min | 21 min | 3 min | 4 |

<!-- prd-time: [{"session": "5def7028-6d5a-4538-82c1-c736b443b166", "start": "2026-10-23T07:13:58.479000+00:00", "wall": 1426, "discarded": 0, "agent": 1265, "human": 161, "interventions": 4, "idle_minutes": 10}] -->
