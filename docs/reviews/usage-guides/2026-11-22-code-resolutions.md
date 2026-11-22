# Usage guides code review resolutions

Resolutions of [2026-11-22-code.md](2026-11-22-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-22

Artifact versions after the fixes: commits `3cdd6db` (R1), `0bb936e` (R2) and `a172447` (the pre-existing advisory),
on top of the reviewed head `62e544d`.

### R1 — minor: Incorrect error rule for identifier string writes

- Disposition: fixed.
- Fix: `docs/guides/sequences-and-ids.md` now says that parsing is the only way from text to an identifier, and that
  a `Value::String` written to a `ulid` or `uuid` column fails with `ValueRule::WrongType`, even when its text is a
  valid identifier; `MalformedText` stays for the parsing of text (`FromStr`).
- Status: claimed fixed, unverified until a follow-up review.

### R2 — minor: Vector NaNs are incorrectly listed as normalized

- Disposition: fixed.
- Fix: the conversions table of `docs/guides/data-types.md` splits the row: `-0` as `+0` and every NaN as one NaN in
  `f32` and `f64`; inside vectors only `-0` as `+0`, since vectors never hold a NaN or an infinity
  (`VectorNotFinite`), as `normalize_vector` checks (`crates/krummi/src/table/value.rs:537-548`).
- Status: claimed fixed, unverified until a follow-up review.

### Pre-existing advisory — bulk load rollback promise

- Disposition: fixed, though outside the introduced findings.
- Fix: `docs/guides/errors.md` and `docs/guides/rows.md` limit the rollback to failures before the commit, and say
  that a failure of the commit's write to the disk (`ErrorKind::Io`) has an unknown outcome, never partial, as
  `WriteTransaction::commit` documents (`crates/krummi/src/lib.rs:1608-1614`). The rustdoc of `bulk.rs:32-33` is left
  as it is: changing code documentation is outside this run.
- Status: claimed fixed, unverified until a follow-up review.

### Checks before the follow-up

- `cargo test -p krummi --doc -- guides::`: 113 passed and 3 compile-fail passed, 0 failed.
- `cargo test -p krummi --test guides`: 1 passed.

Review record: [2026-11-22-code.md](2026-11-22-code.md), wrapper status `findings` (R1, R2), threadId
`01a201e3-ccc8-7863-9695-5d99a53ffbd2`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer `krummi-review`
`gpt-6.1-sol` / `high`.
