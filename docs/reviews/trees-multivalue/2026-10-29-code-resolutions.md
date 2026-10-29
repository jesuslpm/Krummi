# Multi-value trees code review resolutions

Resolutions of [2026-10-29-code.md](2026-10-29-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-10-29

Fix commits: `4ecf488` (R1), `dc317ec` (R2). Reviewed head of the original report: `c455c3e`.

### R1 — blocker: Public handle supertraits expose normal operations on multi-value trees

- Disposition: fixed.
- Evidence: verified against the code: `AsReadTree` and `AsWriteTree` (`crates/krummi/src/lib.rs`, module `sealed`)
  could not be named outside the crate, but their `as_tree()` method was callable through the public bounds
  `ReadHandle` and `WriteHandle`, which inherit them, and returned the normal handle inside a multi-value handle.
- Fix (`4ecf488`): the method takes a `sealed::Token` whose only field is `pub(crate)`, so only the engine can build
  one; the bounds keep working inside the crate (`len`, `delete_tree` and the other operations both kinds share).
- Regression: compile-fail case `crates/krummi/tests/ui/trees/handle_bounds_give_no_normal_tree_of_a_multi_value_tree.rs`
  (run by `crates/krummi/tests/trees_compile.rs`), with the report's generic functions for both bounds: one calls
  `as_tree()` without a token, the other builds `krummi::sealed::Token(())`. RED before the fix: `Expected test case
  to fail to compile, but it succeeded.` GREEN after it, with E0061 (the token is missing) and E0603 (module
  `sealed` is private) pinned in its `.stderr`.
- Checks: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
  on Windows: 408 passed, 0 failed.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — minor: Accepted measurement counts can index past the allocated keys

- Disposition: fixed.
- Evidence: the report's command reproduced the panic before the fix.
- Fix (`dc317ec`): a load by set size takes the whole sets that fit in `--values`, dropping a partial last set, as
  the measurement of 5.3 did with value sizes that did not divide its load; the load's header already prints the
  number of values used, and the example's documentation says so. The recorded runs, with 1,000,000 values, are
  unchanged: every set size divides it.
- Regression: `cargo run -p krummi --example multi_timing -- --values 11 --loads n10 --pairs 2048:1024 --folder <an
  existing folder>` now exits 0 and measures `n10: 10 values, keys with values: 1`. The example has no test suite;
  this command is the check.
- Status: claimed fixed, unverified until a follow-up review.

**Review record**: [2026-10-29-code.md](2026-10-29-code.md), wrapper status `findings`, `threadId`
`01a18948-0d24-73c1-8ac3-8042b744938d`, coordinator `gpt-6.1-sol`/`high`, reviewer `krummi-review-critical`
`gpt-6-astra`/`xhigh` (from the wrapper's telemetry).
