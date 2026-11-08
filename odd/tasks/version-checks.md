# Remembered layout check of temporary-area versions, and reads that do not wait for each other

Objective: implement [pager-version-checks.prd.md](../../docs/prd/krummi/pager-version-checks.prd.md) (U1–U5), item 2
of the investigation of 2026-11-08. Branch `perf/cheap-gaps`, after the cheap gaps run
([cheap-gaps.md](cheap-gaps.md)). Delivery: single branch, unmerged. Test runner: `cargo test`. Phase log and closing
report shared with the cheap gaps run.

## Specs

- **S1** (U1, verbatim): "La primera lectura de un nodo desde una versión confirmada que está en el área temporal
  comprueba su organización, como hoy, la haga una transacción de lectura o de escritura. Si la pasa, las lecturas
  siguientes de esa misma versión, de cualquier transacción, no vuelven a comprobarla mientras la versión siga en el
  área temporal." Failures are never remembered; a copy to modify is a change (S1 of the writer checks); the memory
  reserves nothing and does not count for the maximum size.
- **S2** (U3, verbatim): "encontrar la página que ve una transacción no hace esperar a una lectura por otras lecturas,
  de la misma transacción o de otras."
- **S3** (U4): no visible change; format `9`.
- **S4** (acceptance criteria AC-U1–AC-U7 of the PRD), in a store on disk and in temporary mode, in a normal tree, a
  fixed-size tree and a multivalue subtree.
- **S5** (U5 and AC-U8): single-load pairs before and after, Windows and WSL 2; W12 improves on both; none of W2, W7,
  W10, W12, W13, W14, W19 gets worse.

## Tasks

| ID | Specs | Route | Status | Evidence |
|---|---|---|---|---|
| V1 | S1, S3, S4 | inline: per-version layout memory in the slot, `PageCheck::Version` | done | `5199efc`, L3 |
| V2 | S2, S3 | inline: shared lock for page lookups in the scratch area | done | `a289acf`, L4, L5 |
| V3 | S5 | inline: single-load rotations with the other parts | done | L6 |

## Log

- **L1** (2026-11-08): user's request, verbatim: "Cuando termines haz el 1 y 2 sin preguntar y cambia el PRD con tus
  recomendaciones sin preguntar también todo de seguido. Voy a estar fuera todo el día." PRD written with the
  reviewer's findings resolved by the agent's defaults (authorized by that message); Codex's PRD review failed with its
  usage limit until 2026-11-11 06:49 (`codex-review` log `20261108-072807-pager-version-checks-prd`), pending.
  Start: 2026-11-08 07:29:24 UTC.
- **L2** (2026-11-08): routes. Both units inline: sequential and small. V2 touches concurrency (high-risk item 4): it
  gets an independent verifier and its own RDD review.
- **L3** (2026-11-08): V1. The flag lives in the temporary-area `Slot` (a new `Slot` per published version, so it starts
  clear and goes with it); `Snapshot::page_and_code` returns `Origin::Store(code)` or `Origin::Version { checked }`,
  mapped to `PageCheck::Version`; `node_at_with` skips the check of a checked version and reports every check through
  `layout_checked(page, node, passed)`. `Snapshot::version_checked` marks the version only when its payload is `node`
  (pointer comparison), so a pass is never given to another version. RED: AC-S7 failed on the change
  (`[Check, Check]` expected, one check seen); AC-U1–AC-U7 written in `version_check_tests.rs`, AC-S7 removed (AC-U1
  replaces it). AC-U6 first expected one remembered node and saw two (the catalog's root is read to open the tree);
  the test was fixed, not the code. Workspace: 577 tests pass.
- **L4** (2026-11-08): V2. `Shared::state` is an `RwLock`; `lock()` takes it for writing as before, and the three read
  paths (`page_and_code`, `run`, `version_checked`) take it shared. Each drops the guard before reading the store's
  pages, so no thread takes the lock twice. Clippy and 577 tests pass. No deterministic test of U3 (AC-U8 and AC-U7
  cover it, as the PRD says).
- **L5** (2026-11-08): verification. Independent verifier (sonnet, high) of `5199efc` and `a289acf`: S1–S4 met, no
  blockers. AC-U1, AC-U3, AC-U4 and AC-U5 fail on `d96a92b` with the test file and a counter added; a mutant that
  remembers failures fails AC-U2; AC-U7 passed 6 of 6 runs; fmt, clippy and `cargo test --workspace` pass in its copy
  (removed). Advisories: only AC-U1 covers the three kinds of tree (the PRD's preamble now says so); the PRD's
  annotations of C3 and S1 were missing (added in `ca7c425`); the counter of version checks ran in production builds
  (gated in `7e4d872`). RDD review of `d96a92b..e08b646`: escalated with item 4 (the read-write lock), lenses risk,
  resilience and reliability, approved, lineage `review-42c285b88cc58af2`. Findings: R4-001 (SUGGESTION, the counter in
  production): already fixed in `7e4d872`. R3-version-pointer-guard-untested (SUGGESTION): fixed in `c980732`
  (`a_layout_pass_is_kept_only_by_the_version_that_passed`, which fails without the pointer comparison).
  R3-u3-no-deterministic-lock-test (WARNING, inferential): dismissed with evidence. `Snapshot::page_and_code`,
  `Snapshot::run` and `Snapshot::version_checked` drop their read guard before any other call
  (`crates/krummi-pager/src/scratch.rs`, `drop(state)` before `pager.run_with` in `run`), and the verifier checked every
  lock site for nesting; U3 is measured by AC-U8, as the PRD says.
- **L6** (2026-11-08): V3. Binaries: `base` = `e0ad729`, `cheap` = `d96a92b`, `vc` = `e08b646`, `pd` = `7192dc7`,
  built in release on each platform; each binary measures every load in its own work folder, from its own base store.
  Rotation (`work/cg/final.sh`, `rot-win.out`, `rot-wsl.out`): two rounds of each load over the four binaries, the
  second in reverse order, bench power plan, Windows 10:10–11:06 and WSL 2 11:06–13:40 UTC, nothing else running. Each
  load flagged as lower in both rounds was measured again in two new rounds (`work/cg/confirm.sh`, `conf-win.out`,
  `conf-wsl.out`, Windows 13:42–13:52, WSL 2 13:52–15:13 UTC); what was lower again got five alternating rounds with
  fresh stores (`alt.sh`, `alt-win.out`, 15:15–15:23; `alt-wsl.sh`, `alt-wsl.out`, 15:24–15:28 UTC). Change `vc` /
  `cheap`, first round / second round (new rounds in brackets where a load was lower in both):

  | Load | Windows | WSL 2 |
  |---|---|---|
  | W2 | +4 % / +5 % | +6 % / −2.5 % |
  | W7 | −1 % / −3.5 % [+4 % / +4 %] | +1 % / −4 % |
  | W10 | −1 % / −8 % [−1.4 % / −0.6 %; alternating: higher in 3 of 5] | +7 % / +1 % |
  | W12 | +65 % / +71 % | +32 % / +35 % [+26 % / +40 %] |
  | W13 | +2 % / +10 % | +2 % / −4 % [−3 % / +2 %] |
  | W14 | +2 % / +4 % | +4 % / +5 % |
  | W19 | +8 % / +9 % | +5 % / +3.5 % |
  | W21 | −1 % / −1 % [−2.6 % / −1.8 %; alternating: lower in 5 of 5, median −1.5 %] | 0 % / −0.5 % |

  W12 rises from 983,249 to 1,620,447 op/s on Windows and from 1,788,045 to 2,350,506 on WSL 2 (first round). S5
  holds: W12 improves on both, and of its list only W7 and W10 on Windows were lower in both rounds, neither again
  (W10's five alternating rounds, `cheap` 17.4–19.1 million and `vc` 17.6–19.2 million, show two modes in both
  binaries). Outside the list, W21 on Windows is lower in nine of nine rounds, about −1.5 %, and not on WSL 2: a cost
  of the change, recorded for the closing report (hypothesis, not measured: for a fixed-size node the check it saves
  is cheaper than the extra shared lock that remembers it). Other loads lower in the first pair and not in the new
  one: Windows W4, W17 and W18; WSL 2 W1, W15 and W18.
