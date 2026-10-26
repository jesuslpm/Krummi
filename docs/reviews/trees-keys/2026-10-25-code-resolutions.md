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
