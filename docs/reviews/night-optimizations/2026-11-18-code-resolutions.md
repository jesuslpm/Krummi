# Night optimizations code review resolutions

Resolutions of [2026-11-18-code.md](2026-11-18-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-18

Artifact versions: reviewed range `2e45b31..a8052be`; fixes in `4cd6aa1`.

### R1 — minor: The draft reuses the current store-format number for an incompatible change

- *Disposition:* fixed.
- *Evidence:* `crates/krummi-pager/src/format.rs:41` is `pub const FORMAT_VERSION: u32 = 11;` (E20 of the tables
  with schema PRD).
- *Fix:* `4cd6aa1`. Option 2(a) now says the format becomes `12` and format-`11` stores stay as the earlier format,
  and *Fuera de alcance* says format `11`.
- *Status:* claimed, unverified until a follow-up review.

### R2 — minor: The draft reverses encoding and journal-space reservation

- *Disposition:* fixed.
- *Evidence:* `Journal::enqueue` calls `encode_record` and takes `record.len()` before it rolls the file and
  advances `current_len` (`crates/krummi-pager/src/journal.rs:694-723`).
- *Fix:* `4cd6aa1`. *Fuera de alcance* now says the commit encodes its record before reserving its place in the file,
  because the reservation needs its size; the exclusion itself stays.
- *Status:* claimed, unverified until a follow-up review.

### Deferred product questions

The five questions of the compression draft stay `pending` for the user. They are not defects of the range.

Review record: [2026-11-18-code.md](2026-11-18-code.md), wrapper status `findings` (R1, R2), thread
`01a1ece2-3616-7b20-b572-9ac1e052fe20`. Telemetry: coordinator `gpt-6.1-sol` with effort `high`, reviewer
`krummi-review` `gpt-6.1-sol` with effort `high`.

## 2026-11-18 (verification)

Follow-up report: [2026-11-18-code-02.md](2026-11-18-code-02.md), wrapper status `approved`, thread
`01a1ece9-62c2-7463-919d-0d71f4b02ba0`. Telemetry: coordinator `gpt-6.1-sol` with effort `high`, reviewer
`krummi-review` `gpt-6.1-sol` with effort `high`. Verdict: «Approved for the requested follow-up scope: R1 and R2
verified resolved; no findings remain open.» R1 and R2: resolved. The draft's five product questions stay for the
user.
