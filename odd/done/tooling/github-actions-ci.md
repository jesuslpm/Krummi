# GitHub Actions CI — ODD execution document

Objective: run the engine checks on real Linux and Windows runners on every push and pull request.
Branch: `ci/github-actions`. Delivery: one commit plus fixes; push of this branch only (authorized), never `main`.
Test runner: the workflow itself on GitHub Actions. Estimate: 30–60 min. Real start: 2026-10-22 16:55:15 +0100.

## Specs

- **S1** — Matrix `ubuntu-latest` + `windows-latest` on every push and pull request to `main`: `cargo fmt --check`,
  `cargo clippy -D warnings`, `cargo test --workspace` (user approval of the proposal: "Sí").
- **S2** — The full AC10 sweep runs on Linux as its own step; on Windows too if it is viable there.
- **S3** — Cargo cache, so each run takes minutes instead of building from scratch.
- **S4** — Verify by pushing `ci/github-actions` and following the run with `gh` until it passes on both
  systems ("Sí, subir la rama").

## Defaults taken

- Actions pinned to commit SHAs (`actions/checkout` v7.0.1, `Swatinem/rust-cache` v2.9.3); Rust pinned to
  1.99.0 in the workflow, the version used locally, so new lints or compiler messages (trybuild) do not break
  the build unannounced. No `rust-toolchain.toml`: local setups stay untouched.
- Read-only `GITHUB_TOKEN`; a newer push cancels the previous run of the same branch.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1–S3 | inline | done | `a64152d`, `ea2f292`, `b3a2995` |
| T2 | S4 | inline | done | runs `39809547401` … `39812202793` |

## Log

- **L1** (2026-10-22, user, verbatim): "ya tenemos repo en GitHub https://github.com/jesuslpm/MuninnDB", then
  "Sí" to adding the workflow and "Sí, subir la rama (Recommended)".
- **L2** (2026-10-22): run `39809547401` (`a64152d`): Linux green, AC10 full included; Windows failed
  `a_store_is_created_inspected_closed_and_reopened` (`NotAStore` instead of `StoreInUse`). `--no-fail-fast`
  added (`ea2f292`); run `39810106741` showed the cause in `ac_p6_*`: the data file was named
  `muninn.data` followed by garbage. RED reproduced locally with folder names of 1–12 characters (3 failed).
  **Pager bug, also in `main`:** the Windows rename by handle passed a name without a NUL terminator, and kernel32
  reads it as a NUL-terminated string. Fixed in `6a8d000`; run `39810982198` green on both systems; AC10 full on
  the runners: Linux 90.1 s, Windows 73.9 s (the local antivirus was what made it impractical, L7 of the Pager).
- **L3** (2026-10-22, RDD review): commits `4d37d64..c673e94`; lenses risk, resilience, reliability;
  **approved**; lineage `review-ca79ed5edb730044` (the first bound STATUS timed out with
  `operation_timeout`, `mutation_outcome: not_started`; the preflight was repeated and START replayed).
  Findings: reliability WARNING `R3-002` (README said "every push" but the workflow filtered branches) and
  SUGGESTION `R3-001` (the path-length test catches the over-read only by chance); both fixed in `b3a2995`
  (CI on every push; deterministic Windows unit test of the FILE_RENAME_INFO buffer, which fails without the
  terminator).
- **L4** (2026-10-22): run `39812202793` (`b3a2995`) green: Windows 2 min 50 s, Linux 1 min 07 s; AC10 full
  74.6 s and 42.6 s.
- **L5** (2026-10-22, RDD review): commit `c673e94..b3a2995`; lenses risk, reliability; **approved**, no
  findings; lineage `review-5ccd94683f19400d`.

## Closing

Report: [github-actions-ci-report.md](github-actions-ci-report.md).
