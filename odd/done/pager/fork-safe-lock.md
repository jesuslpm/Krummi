# Fork-safe store lock — ODD execution document

Objective: a store's lock must not be kept by child processes on Linux (P9: "Cerrar el almacén, o dejar de
usarlo sin cerrarlo explícitamente, libera el bloqueo"). Branch: `fix/fork-safe-lock` (push authorized).
Test runner: `cargo test --workspace` on Windows, WSL and GitHub Actions. Estimate: 40–80 min.
Real start: 2026-10-22 17:28:34 +0100.

## Specs

- **S1** — On Linux, a child process created while a store is open does not keep it locked: after the store
  is closed or dropped, it reopens at once even if the child has not reached `exec` yet.
- **S2** — The rules of P9 still hold: a second open in the same process or in another process fails with
  "Store in use"; a failed second open in the same process never releases the lock; another path to the same
  store is the same store; a crash leaves the store unlocked.
- **S3** — Windows keeps its current lock (handles are not inherited).

## Defaults taken

- POSIX record locks (`fcntl F_SETLK`, through `rustix`) plus an in-process table of locked files keyed by
  device and inode, as SQLite does. The data file is opened once per store and closed under the table's lock,
  because closing any descriptor of a file drops the process's POSIX locks on it.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1–S3 | inline | done | `67bf998`, `377563b`, `601d591`, `6958306`, `456d63a` |

## Log

- **L1** (2026-10-22): CI on `main` (run `39813413014`) failed on Linux:
  `ac_p5_dropping_a_store_without_closing_it_releases_the_lock` got "store … is already open". Cause: `flock`
  belongs to the open file description, which a child process spawned by another test thread shares until its
  `exec`. User, verbatim: "Sí" (fix it), "Sí, subir la rama (Recommended)".
- **L2** (2026-10-22): RED on Linux (WSL): `a_child_process_spawned_while_the_store_is_open_does_not_keep_it_locked`
  failed with `flock` (a child parked between fork and exec held the store). GREEN after `67bf998`; Linux suite 5/5
  runs, Windows green. Clippy on Linux flagged a complex type in Unix-only code (fixed with a type alias).
- **L3** (2026-10-22, RDD review): `30baaeb..67bf998`; lenses risk, resilience, reliability; **approved**; lineage
  `review-0e144bcb102749f8`. WARNING (resilience and reliability): a creation whose lock fails left
  `muninn.data.new` behind — fixed in `377563b`. SUGGESTIONS: deterministic fork handshake and a symlink alias test —
  done in `377563b`; the process-wide table mutex serializes opens — documented, kept.
- **L4** (2026-10-22, independent verifier, sonnet): no blockers; S1–S3 PASS. Probe: 40k–43k child spawns per 25 s
  while 8 threads reopened stores, 0 failures (the same probe on `30baaeb` failed in seconds); symlink and hard-link
  aliases; ~39k Windows cycles. Advisories fixed in `6958306`: `rustix` declared for every Unix (macOS would not
  compile); explicit unlock on Windows close. Left open: the table mutex serializes opens; (dev, ino) identity on
  file systems with unstable inodes. Scratch removed (74 MB Windows, 826 MB WSL).
- **L5** (2026-10-22, RDD reviews): `377563b` → **approved**, lineage `review-cbf154ecf09f6c96`; WARNING the cleanup
  was tested only through a helper — fixed in `601d591` (lock step as a fault point, mutation-checked); WARNING an
  I/O lock error could remove a file another opener holds — left open (a lock conflict is reported as in use, not
  as an I/O error). `601d591..6958306` → **approved**, lineage `review-0e52271452c30923`; WARNING the in-use branch
  lost its test — fixed in `456d63a` (mutation-checked). `456d63a` → **approved**, lineage
  `review-3869eb3634ef8dc4`; two SUGGESTIONS not applicable (the test already reopens and checks the folder; fault
  state is per thread).
- **L6** (2026-10-22): CI run `39817011810` on `456d63a` green on both systems (Linux 1 min 23 s, Windows 1 min 44 s);
  AC10 full 40.7 s and 49.2 s.

## Closing

Report: [fork-safe-lock-report.md](fork-safe-lock-report.md).
