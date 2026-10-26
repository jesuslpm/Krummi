# Trees and key operations: code review resolutions

Resolutions of [2026-10-25-code.md](2026-10-25-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-10-26

### R2 — major: the A15 measurement omits required hardware information

- Disposition: accepted; fixed in the closing report.
- Evidence: the processor and the disk of both environments were queried on the measuring machine on 2026-10-26
  (`Get-CimInstance Win32_Processor`, `Get-PhysicalDisk`, `Get-Partition -DriveLetter C | Get-Disk`, and in WSL
  `lscpu` and `df -hT`): 13th Gen Intel Core i9-13980HX (24 cores, 32 threads); NVMe Samsung MZVL22T0HBLB-00B00
  (SSD, 2 TB) for Windows; the WSL 2 ext4 virtual disk (`/dev/sde`), stored as a `.vhdx` on the same NVMe, for
  Linux. They were collected after the measurement, on the same machine and configuration.
- Fix: `odd/tasks/trees-keys-report.md`, section *Medición (A15)*, now has the equipment table; AC-A19's PASS rests
  on it. Commit: recorded in the next update with the R1 fix.

### R1 — major: listing trees aborts on allocation failure instead of returning OutOfMemory

- Disposition: accepted. The scope is wider than `list_trees`: every allocation the branch added for trees on an
  operation path (catalog list, splits and merges, freeing a tree, the facade's per-transaction table) becomes
  fallible and reports OutOfMemory, through the pager's `Op::Alloc` fault point, so tests can make it fail.
- Fix: in progress.

## 2026-10-26 (later)

### R2 — commit

- Fix commit: `62b1813` (equipment table in `odd/tasks/trees-keys-report.md`).

### R1 — fixed

- Disposition: fixed in `06ec986`.
- Fix: a fallible helper `krummi_pager::try_reserve` calls the pager's `Op::Alloc` fault point and maps a refusal to
  OutOfMemory (P13); the tree code reserves through it (catalog list names and vector, split node copy, cell list
  and separator key, merge copy, freeing a tree's children list), and the facade's per-transaction table became a
  sorted `Vec` reserved through it (`BTreeMap` has no fallible insert), as do the names it stores. A failure in a
  write is recorded as the write's failure through `WriteTransaction::fail` (T11). Left infallible on purpose: the
  data inside errors, the lossy conversion of a non-UTF-8 catalog name, and the pre-existing `BTreeMap`/`BTreeSet`
  inserts of `krummi-tx`.
- Evidence: `crates/krummi/tests/tree_memory.rs`, 9 tests that inject `Op::Alloc` refusals at every reservation of an
  operation until it succeeds: `list_trees` in a read (the review's scenario; the same read then lists), `list_trees`,
  opening, creating, removes that merge, deleting a multi-level tree and renaming in a write (each follows T11: the
  operation and the commit fail with OutOfMemory, nothing applies, a new write commits), sets that split and the
  commit of changed entries. RED observed for 7 of them (no failure seen before the fix); the split and commit cases
  passed before, because the pager reserves its page first. `cargo test --workspace`: 293 passed, 0 failed, 9 ignored.
  RDD review `review-99c3098713ed944a` (resilience, reliability) approved; its open items are in the closing report.
- Status: claimed fixed, unverified until a follow-up review.
