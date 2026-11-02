# Fixed-size trees (5.5) — measurement (X13)

Source data for the closing report (AC-X21). Task T4 of `odd/tasks/trees-fixed-size.md`, measured on 2026-11-02 with
the example of `edfe769` on the engine of `90b7bb1` (T3 added only tests).

## How to rerun

```text
cargo run --release -p krummi --example fixed_timing -- [--folder <folder>] [--entries 1000000] [--lengths 0,8,100]
```

`crates/krummi/examples/fixed_timing.rs` runs X13 exactly: on disk, durable commits, 1,000,000 distinct random keys
from `0` to `2^64−1` (SplitMix64 of the index, a bijection) and random value bytes, with fixed seeds, so every load
stores the same entries. For values of 0, 8 and 100 bytes, a fixed-size tree against a normal tree that stores the
same key as its 8 big-endian bytes; converting the key to bytes when storing and back to an integer when reading is
inside the timed loops. Per kind and length: store in key order in one new store and at random in another, both in
writes of 1,000 entries, with the store's size (P12) and its pages in use (pages minus free pages; the size moves in
steps because the file grows by doubling) after each load; then, in the store loaded at random, closed and reopened
without dropping the OS cache, read every entry at random in one read transaction and scan them all forward in
another, both folding the key and every value byte into a checksum; and remove every entry at random, in writes of
1,000. Times are microseconds per entry. Release build. No threshold (D5).

Runs, one after the other so they did not compete: Windows with the built binary and the stores in
`%TEMP%\krummi-x13` (11:31:34–11:36:36 UTC); Linux in WSL 2, built from `/mnt/c/projects/Krummi` with
`CARGO_TARGET_DIR=~/krummi-x13-target`, the stores in `~/krummi-x13-stores` on ext4 (11:37:37–11:47:38 UTC, after a
30 s build).

## Machine

Both environments on the same machine (queried on 2026-11-02 with `Get-CimInstance` and `Get-PhysicalDisk`, and
`/etc/os-release`, `uname -r` and `/proc/cpuinfo` in WSL):

| Environment | Operating system | Processor | Memory | Disk |
|---|---|---|---|---|
| Windows | Windows 11 Pro 10.0.26200 | 13th Gen Intel Core i9-13980HX (32 threads) | 64 GiB | NVMe Samsung MZVL22T0HBLB-00B00 (SSD) |
| Linux | WSL 2 Ubuntu 24.04.1 LTS, kernel 5.15.133.1-microsoft-standard-WSL2, ext4 | the same | the same | WSL 2 virtual ext4 disk on the same NVMe |

## Summary

- **Space.** The pages are the same on both systems. The fixed-size tree uses 43 % and 40 % fewer pages than the normal
  tree with 0-byte values (989 against 1,730 in key order, 1,343 against 2,220 at random), 27–31 % fewer with 8-byte
  values (1,971 against 2,716; 2,799 against 4,052), and 5 % fewer with 100-byte values (13,367 against 14,126; 19,398
  against 20,521): an entry costs 8 + N bytes against 14 + N, so the gain shrinks as the value grows. The file size
  halves with 0- and 8-byte values in key order and with 0 bytes at random; elsewhere the doubling steps hide the
  difference.
- **Loads.** On Windows, storing in key order is ×6.8, ×3.2 and ×1.6 faster for 0, 8 and 100 bytes, and at random ×1.7,
  ×1.8 and ×1.1. On Linux the durable commit dominates (about 9 ms per write of 1,000 entries even for the smallest
  load), so the gains are smaller: ×1.8, ×1.5 and none in key order, ×1.17, ×1.14 and none at random.
- **Reads.** Reading at random costs 1.2–1.8 µs on both systems: 3 % to 27 % less than the normal tree on Windows and 5
  % to 22 % less on Linux, the most with 8-byte values on Windows and with 0-byte values on Linux. A forward scan costs
  0.01 µs per entry against 0.01–0.02 µs, and 0.07–0.08 µs for both with 100-byte values, where reading the bytes
  dominates.
- **Removals at random** follow the loads at random: ×1.7, ×1.4 and ×1.1 faster on Windows; ×1.2, ×1.09 and the same on
  Linux.

## Windows

### Values of 0 bytes

| Tree | Store in order (µs) | Size in order (bytes) | Pages in use in order | Store at random (µs) | Size at random (bytes) | Pages in use at random | Read at random (µs) | Scan forward (µs) | Remove at random (µs) |
|---|---|---|---|---|---|---|---|---|---|
| fixed-size | 0.76 | 8388608 | 989 | 14.36 | 16777216 | 1343 | 1.34 | 0.01 | 14.17 |
| normal, 8-byte big-endian keys | 5.16 | 16777216 | 1730 | 24.04 | 33554432 | 2220 | 1.38 | 0.01 | 24.66 |

### Values of 8 bytes

| Tree | Store in order (µs) | Size in order (bytes) | Pages in use in order | Store at random (µs) | Size at random (bytes) | Pages in use at random | Read at random (µs) | Scan forward (µs) | Remove at random (µs) |
|---|---|---|---|---|---|---|---|---|---|
| fixed-size | 1.29 | 16777216 | 1971 | 16.01 | 33554432 | 2799 | 1.24 | 0.01 | 17.67 |
| normal, 8-byte big-endian keys | 4.15 | 33554432 | 2716 | 29.18 | 33554432 | 4052 | 1.71 | 0.02 | 24.85 |

### Values of 100 bytes

| Tree | Store in order (µs) | Size in order (bytes) | Pages in use in order | Store at random (µs) | Size at random (bytes) | Pages in use at random | Read at random (µs) | Scan forward (µs) | Remove at random (µs) |
|---|---|---|---|---|---|---|---|---|---|
| fixed-size | 2.72 | 134217728 | 13367 | 26.27 | 268435456 | 19398 | 1.81 | 0.07 | 24.56 |
| normal, 8-byte big-endian keys | 4.32 | 134217728 | 14126 | 29.05 | 268435456 | 20521 | 2.07 | 0.08 | 27.05 |

## Linux

### Values of 0 bytes

| Tree | Store in order (µs) | Size in order (bytes) | Pages in use in order | Store at random (µs) | Size at random (bytes) | Pages in use at random | Read at random (µs) | Scan forward (µs) | Remove at random (µs) |
|---|---|---|---|---|---|---|---|---|---|
| fixed-size | 9.40 | 8388608 | 989 | 38.11 | 16777216 | 1343 | 1.20 | 0.01 | 36.24 |
| normal, 8-byte big-endian keys | 16.92 | 16777216 | 1730 | 44.41 | 33554432 | 2220 | 1.54 | 0.02 | 43.97 |

### Values of 8 bytes

| Tree | Store in order (µs) | Size in order (bytes) | Pages in use in order | Store at random (µs) | Size at random (bytes) | Pages in use at random | Read at random (µs) | Scan forward (µs) | Remove at random (µs) |
|---|---|---|---|---|---|---|---|---|---|
| fixed-size | 12.19 | 16777216 | 1971 | 38.65 | 33554432 | 2799 | 1.33 | 0.01 | 38.83 |
| normal, 8-byte big-endian keys | 18.40 | 33554432 | 2716 | 43.98 | 33554432 | 4052 | 1.42 | 0.02 | 42.33 |

### Values of 100 bytes

| Tree | Store in order (µs) | Size in order (bytes) | Pages in use in order | Store at random (µs) | Size at random (bytes) | Pages in use at random | Read at random (µs) | Scan forward (µs) | Remove at random (µs) |
|---|---|---|---|---|---|---|---|---|---|
| fixed-size | 20.24 | 134217728 | 13367 | 41.92 | 268435456 | 19398 | 1.82 | 0.08 | 41.00 |
| normal, 8-byte big-endian keys | 19.89 | 134217728 | 14126 | 41.54 | 268435456 | 20521 | 1.92 | 0.08 | 41.45 |
