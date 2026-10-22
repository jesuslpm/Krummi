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
| T1 | S1–S3 | inline | pending | |
| T2 | S4 | inline | pending | |

## Log

- **L1** (2026-10-22, user, verbatim): "ya tenemos repo en GitHub https://github.com/jesuslpm/MuninnDB", then
  "Sí" to adding the workflow and "Sí, subir la rama (Recommended)".
