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
| T1 | S1–S3 | inline | pending | |

## Log

- **L1** (2026-10-22): CI on `main` (run `39813413014`) failed on Linux:
  `ac_p5_dropping_a_store_without_closing_it_releases_the_lock` got "store … is already open". Cause: `flock`
  belongs to the open file description, which a child process spawned by another test thread shares until its
  `exec`. User, verbatim: "Sí" (fix it), "Sí, subir la rama (Recommended)".
