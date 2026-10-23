# Rename to Krummi — ODD execution document

Objective: implement [rename-to-krummi.prd.md](../../docs/prd/krummi/rename-to-krummi.prd.md) (R1–R13, AC-R1–AC-R13):
the product MuninnDB becomes Krummi in packages, store files, mark, format, messages, docs, PRDs, tooling, GitHub
repository and memory project.
Branch: `feat/rename-to-krummi`, in the main copy (R9). Delivery: `ask-on-risk`. Test runner: `cargo test --workspace`
(Windows; Linux in WSL). Estimate: 1 h 15 min – 2 h. Real start: 2026-10-23 08:39:40 +0100.

## Specs

The PRD is the specification; every S# below points at its decisions and criteria, which workers read in full.

- **S1 — Packages (R2, AC-R5).** Packages `krummi`, `krummi-pager`, `krummi-tx`; `muninndb*` no longer exist; public
  type and function names unchanged; no path contains `muninn` (R10).
- **S2 — Store files, mark and format (R3, R4, R5, AC-R1, AC-R3, AC9 adapted).** `krummi.data`, `krummi.data.new`,
  `krummi.journal.` + 19 digits, `krummi.temp`; a new mark; format `3`; "AC9 se adapta a R5: formato anterior con `0`
  y `2`, posterior con `4`."
- **S3 — MuninnDB leftovers (R6, AC-R2, AC-R4).** A folder without `krummi.data` and with any `muninn.*` entry is not
  a store and is left untouched; inside a Krummi store, `muninn.*` entries are foreign and survive use, close and
  recovery byte for byte.
- **S4 — Error texts (R1, AC-R6).** The library errors that name the product today (not a store, different format)
  contain `Krummi` and no `muninn` in any case.
- **S5 — Docs (R7, R12, AC-R8).** README explains the name Krummi ("Krummi, el apodo islandés del cuervo, y el guiño a
  Voron") without naming the old one; instructions, worktree guide and methodology name `C:\projects\Krummi` and
  copies `C:\wt\krummi-<n>`.
- **S6 — Working copies (R8, AC-R9).** Copies `krummi-<n>` (folder and memory project); their tests pass on Windows.
- **S7 — PRDs (R7, R11, AC-R7, AC-R11).** Folder `docs/prd/krummi`, root `krummi.prd.md` / `krummi.prd.en.md`;
  titles, rules, flows, messages, outputs and criteria renamed; an annotation per PRD; history sections keep their
  text; every relative link and anchor works.
- **S8 — Remaining name (R10, AC-R7).** `git grep -i muninn` only in R10 (a)–(e); no tracked path contains `muninn`.
- **S9 — Close (R9, R13, AC-R10, AC-R12, AC-R13).** Suite and full AC10 sweeps on Windows and Linux; report and
  merge; then rename on GitHub, set `origin`, copy the memories of `muninndb` into `krummi`, and a final commit with
  their outcome in the report.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1 | inline | done | `903cd08` |
| T2 | S2, S3, S4 | inline | done (RED: 4 new tests failed before the change; GREEN after) | `6da69ea` |
| T3 | S6 | inline | done | `95918db`, `4526f12` (Defender scripts, L15) |
| T4 | S5 | inline | done | `d132208`, `0a6c094` |
| T5 | S7 | delegated (Writer trigger (a): independent of T1–T4, disjoint surface `docs/prd/**`, long careful edit) | done | `5129427` |
| T6 | S8, S9 | inline | done (checks, acceptance, report; external steps after the merge, L18) | this commit |

Risk: T2 touches item 1 (persisted format change), so it gets its own commit, an independent verifier and an RDD
review.

## Log

- **L1** (2026-10-23, user, verbatim): "Implementa" — authorization: "Autorizo los dos" (GitHub rename of
  jesuslpm/MuninnDB to jesuslpm/Krummi with the user's gh session and `origin` repointed, no push; Engram: read
  `muninndb`, save copies in `krummi`, delete nothing); estimate 1 h 15 min – 2 h, answer "Sí".
- **L2** RDD review of T1 (`main..903cd08`, lens reliability): approved, lineage `review-bcdaf9e94782e648`.
  Advisories: WARNING R3-ac-r5-no-package-set-assertion (no test pins the package set) — fixed in T2 with
  `crates/krummi/tests/packages.rs`; SUGGESTION R3-ledger-t1-status (task table out of date) — fixed here.
  Assessment counted renamed files as whole files (19,408 changed lines, `high_risk` from `process_boundary` and
  `shell_source` signals in moved files).
- **L3** T5 writer report: 10 PRD files changed (298+/236−); its checks: names outside history 0, relative links
  119 checked, 0 broken (parent spot check repeated both: PASS). History sections headed "Anotaciones al padre"
  (Pager, Transactions) count as annotations.
- **L4** T2: the 16-byte mark is `\x89Krummi\r\n\x1a\n` + five zero bytes; journal record magic `KrummiJ1`;
  format-1 and format-2 headers with the new mark are older formats. Old names live only in the last section of
  `crates/krummi-tx/tests/recovery.rs` (R10 e). Windows suite, clippy and fmt green.
- **L5** Linux suite green on `6da69ea` (WSL Ubuntu, clone `~/muninn-accept`). `wsl -- bash -lc '<script>'`
  loses `$` variables; the sweeps run from a script file with `bash -l` (cargo is on the login PATH only).
- **L6** Link check of README, CLAUDE.md, docs/worktrees.md and docs/methodology: 74 links, 25 broken, all in
  docs/methodology and already broken at the baseline (they point at chicago-crime PRDs and reports that this
  repository never had); not caused by this change, left as a pending item.
- **L7** Independent verifier of T2 (S1–S4): all pass, no blockers; probes with stores made by the baseline engine
  (clean and with pending journal commits) give NotAStore with unchanged hashes. Advisories: the execution
  document linked the old PRD folder (fixed); a format-2 layout under version 1 reads as damage, as at the baseline
  and outside AC9 (accepted). Scratch removed by the parent.
- **L8** RDD of `903cd08..6da69ea` refused with `lens_context_budget_exceeded` (the PRD move counts whole files).
  Split: `903cd08..5129427` (T5) assessed alone `passive` (`non_executable_only`), no review; T2 reviewed alone.
- **L9** RDD review of T2 (`5129427..6da69ea`, lenses risk and reliability): approved, lineage
  `review-2bff1273b1ee719b`. Advisories: WARNING R3-workspace-members-literal and SUGGESTION R3-package-name-parse
  (the AC-R5 test matches the manifest text literally) — accepted: the test is meant to pin the exact layout.
- **L10** T3 `95918db`, T4 `d132208`; assessment `6da69ea..d132208`: medium, `under_budget`. Tooling tests:
  Python 33 + 62 + 24 OK, PowerShell `Test-Worktree.ps1` 41 passed, 0 failed (178 s).
- **L11** Last-slice RDD review (`6da69ea..d132208`, lenses readability and reliability): approved, lineage
  `review-4effe6bb7ceabfe2`. Findings: WARNING R2-001, R2-002, R3-002 and SUGGESTION R3-003 assume the code and the
  PRDs were not renamed — not applicable, they were renamed in `6da69ea` and `5129427`; WARNING R2-003 and R3-001
  (copies made with the old prefix) — not applicable, no copy exists (R8); SUGGESTION R2-004 (help table alignment
  in `WorktreeCommon.psm1`) — accepted as is.
- **L12** AC-R5: a new project outside the repository with `krummi = { path = ... }` compiles, creates a store
  (format 3), commits, reopens and leaves only `krummi.data`. The public API stores no data yet, so "lee lo
  escrito" is limited to commit and reopen. AC-R7: no tracked path contains `muninn`; content only in R10 (a)–(e)
  (old `odd/tasks` documents, this run's documents, the rename PRD pair, the PRD history parts, the last section of
  `crates/krummi-tx/tests/recovery.rs`).
- **L13** Linux full AC10 sweeps on `6da69ea` (release): Pager 43 s, transactions 32 s, journal 19 s, all green.
- **L14** (2026-10-23, user, verbatim): "Qué es lo que está tardando tanto?" — the Windows full Pager sweep (65,536
  reopens, started 08:54:57). Diagnosis, measured on the sample sweep (192 flips, release): 15 s at `6da69ea` against
  4 s at `903cd08`; bisected to `FORMAT_VERSION = 3` alone (with 2 and everything else from T2: 4 s). No engine code
  branches on the number; the time is all in `Pager::open` of a file just modified, also when it fails on the mark
  before reading the header; the test process uses almost no CPU. Without the engine, a Python probe that rewrites a
  byte and maps the real `krummi.data` takes 18–22 ms per round with the format-2 file and 35–38 ms with the format-3
  file; the two files differ only in the version byte and the header checksums (offsets 64, 96–103, 8192,
  8224–8231). Linux is not slower (43 s against 54 s before). Conclusion: the on-access antivirus scan of Windows
  costs more for these bytes; not an engine defect, the sweep's results stay valid, and it is slow only on this
  machine's Windows. Probe scratch removed.
- **L15** (2026-10-23, user, verbatim): "Nos falta una prueba, desactivar el antivirus o poner excepciones si es
  posible y volver a medir." — this session is not administrator and Defender has tamper protection, so it cannot
  add exclusions. User: "Dame el script de powershell para agregar exlusiones y lo ejecuto yo como administrador",
  then "mejor pon los scripts en el repo." Added `scripts/antivirus/Add-DefenderExclusion.ps1` and
  `Remove-DefenderExclusion.ps1` (one folder, default `%TEMP%\krummi-av-bench`; both refuse without administrator
  rights, checked) and their use in the README. Wait for the user to run the first one.
- **L16** (2026-10-23, user): "Listo!!" — exclusion added (Defender stored it with the short path
  `C:\Users\JESUS~1.LOP\...\krummi-av-bench`). Sample sweep with the tests' `TMP` in the folder by its long path:
  format 2 0.06 s, format 3 0.06–0.07 s (passed, 1 test); by its short path: 4.6 s and 15.2–15.9 s (the exclusion
  does not apply). The whole slowdown is the antivirus scan. The slow full sweep without the exclusion was stopped
  (it had run 58 min on the Pager). Full Windows AC10 sweeps in the excluded folder, on the crates of HEAD (clone at
  `d132208`, `crates` identical): Pager 17 s, transactions 32 s, journal 181 s, all green.
- **L17** RDD review of `d132208..4526f12` (risk, readability): approved, lineage `review-0784cc851286cc43`;
  WARNING R1-001 (unscanned folder while excluded) and SUGGESTION R2-tmp-not-restored fixed in `0a6c094`
  (assessed passive); WARNING R2-remove-path-fallback and SUGGESTION R2-duplicated-admin-check accepted. Final
  Windows checks on `0a6c094`: fmt and clippy clean, 164 passed, 0 failed, 3 ignored. Benchmark folder removed.
- **L18** Closing: report written; merge into `main` (fast-forward), then the external steps of R9 and R13, whose
  outcome is added to the report in a final commit on `main`.

## Closing

Report: [rename-to-krummi-report.md](rename-to-krummi-report.md).
