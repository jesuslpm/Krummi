# Multi-value trees (5.4) — measurement (M15) and threshold evaluation (M3)

Source data for the closing report (AC-M19, AC-M20). Task T4 of `odd/tasks/trees-multivalue.md`, measured on
2026-10-29 with the example of `c44fe75` (the engine of `2f7b39c`; `d605cc3` changes only damage detection).

## How to rerun

```text
cargo run --release -p krummi --example multi_timing -- [--folder <folder>] [--pairs 2048:1024,...] [--loads n1,zipf,...]
```

`crates/krummi/examples/multi_timing.rs` runs M15 exactly: on disk, durable commits, random 16-byte keys and values
with fixed seeds, 1,000,000 values per load. Loads by set size: keys of N values each for N = 1, 10, 100, 1,000,
10,000, 100,000 and 1,000,000. Skewed load: each value goes to one of 100,000 keys drawn with a Zipf distribution of
exponent 1 (80,504 keys got values). For each load and each threshold pair, in a new store: add every value in random
order, interleaving keys, in writes of 1,000 values; the store's size (P12) after the last commit; closed and
reopened without dropping the OS cache, Contains for every value at random in one read transaction; the scan of
every key; the scan of each key's values; and Remove value for every value at random, in writes of 1,000.
Oscillating load: in a new store, one key with just the values that leave it at the promotion threshold, then
100,000 operations alternating Add of a new value and Remove value of that value, in writes of 1,000. Release build.
No threshold (D5).

Besides what M15 asks, the example prints two columns that show space and reads more finely: the **pages in use**
after adding (the store's pages minus its free pages, as AC-A15 takes them; the size moves in steps because the file
grows by doubling) and the **pages per Contains**, the average number of pages a Contains reads, computed from the
height of the parent and of each key's subtree. Each load also has a **composite** row, the baseline asked in L8: a
normal tree whose 32-byte keys are the key followed by the value, with an empty value, using Try add, Get, a prefix
scan and Remove; it has no key scan with counts.

Runs: Windows with the built binary and the stores in `%TEMP%\krummi-m15`; Linux in WSL 2, built from
`/mnt/c/projects/Krummi` with `CARGO_TARGET_DIR=~/krummi-t5-target`, the stores in `~/krummi-m15-stores` (ext4).
One after the other, so they did not compete. Wall clock of one run: 62 min on Windows, about 86 min on Linux.

A first Windows run (14:04–17:29) was discarded: its composite rows took 85–108 µs per random add, against 27 µs
measured for the same load by 5.2 on this machine, and its last loads came back to 28–30 µs, so something outside the
engine slowed the machine down for most of it. The rerun (19:02–20:04), below, gives 26–34 µs.

## Machine

Both environments on the same machine (queried on 2026-10-29 with `Get-CimInstance` and `Get-PhysicalDisk`; the
same as in `trees-large-values-measurements.md`):

| Environment | Operating system | Processor | Memory | Disk |
|---|---|---|---|---|
| Windows | Windows 11 Pro 10.0.26200 | 13th Gen Intel Core i9-13980HX (32 threads) | 64 GiB | NVMe Samsung MZVL22T0HBLB-00B00 (SSD) |
| Linux | WSL 2 Ubuntu 24.04.1 LTS, kernel 5.15.133.1-microsoft-standard-WSL2, ext4 | the same | the same | WSL 2 virtual ext4 disk on the same NVMe |

## Summary

µs per value (Add and Contains) and pages in use after adding; the multi-value figure is the chosen pair
2048:1024. Pages are the same on both systems.

**Linux**

| Load | Add, multi | Add, composite | Contains, multi | Contains, composite | Pages, multi | Pages, composite | Pages per Contains, multi / composite |
|---|---|---|---|---|---|---|---|
| n1 | 48.1 | 44.3 | 4.4 | 4.0 | 7,729 | 6,964 | 3.00 / 3.00 |
| n10 | 42.1 | 42.7 | 3.5 | 3.9 | 3,806 | 6,835 | 3.00 / 3.00 |
| n100 | 40.3 | 40.8 | 3.1 | 3.8 | 3,442 | 7,022 | 3.00 / 3.00 |
| n1000 | 42.9 | 40.1 | 5.0 | 3.8 | 5,020 | 6,982 | 4.00 / 3.00 |
| n10000 | 43.6 | 40.1 | 5.1 | 3.7 | 3,440 | 7,110 | 4.00 / 3.00 |
| n100000 | 42.0 | 40.1 | 5.2 | 3.9 | 4,167 | 6,977 | 4.00 / 3.00 |
| n1000000 | 43.3 | 38.1 | 5.5 | 4.0 | 4,028 | 7,128 | 4.00 / 3.00 |
| zipf | 43.8 | 40.4 | 5.3 | 3.8 | 4,400 | 7,059 | 4.17 / 3.00 |

**Windows**

| Load | Add, multi | Add, composite | Contains, multi | Contains, composite |
|---|---|---|---|---|
| n1 | 37.2 | 33.6 | 4.5 | 4.2 |
| n10 | 28.1 | 29.7 | 3.6 | 4.4 |
| n100 | 23.4 | 28.9 | 3.4 | 4.3 |
| n1000 | 30.7 | 27.4 | 5.2 | 4.1 |
| n10000 | 30.9 | 26.8 | 5.5 | 4.2 |
| n100000 | 29.2 | 26.3 | 5.4 | 4.1 |
| n1000000 | 29.8 | 25.9 | 5.6 | 4.1 |
| zipf | 32.8 | 27.9 | 5.7 | 4.1 |

- **Add**: a random add in writes of 1,000 is dominated by the commit, which writes every leaf the write touched, as
  for a normal tree (5.2 measured 27 µs on Windows and 37.5 µs on Linux per random set on this machine). While a
  key's values are in its entry (n10, n100), the multi-value tree costs the same or less (up to 19% less on Windows);
  with subtrees, and with one value per key, it costs 5–18% more (Linux 5–14%, Windows 11–18%).
- **Contains** reads one page per level. While a key's values are in its entry (n10, n100), the multi-value tree is
  the faster, 10–21%, because its parent is smaller; when they are in a subtree, it reads the parent's levels and
  the subtree's, one page more than the composite tree, and costs 27–40% more (5.0–5.7 µs against 3.7–4.2 µs).
- **Space**: the multi-value tree takes 28–52% fewer pages in every load but n1, where each key with one value
  carries a blob header and takes 11% more.
- Only the multi-value tree counts a key's values without reading them and scans keys with their counts (about
  6 ms for 100,000 keys, against reading 1,000,000 entries in the composite tree).

## Threshold evaluation (M3)

Six pairs, as M3 names them: a low promotion threshold (512 bytes), an intermediate one (2,048) and the highest that
keeps an inline entry in its leaf (4,074, `MAX_PROMOTION`), each with a demotion threshold close to it (7/8) and one
around its half. The threshold unit is the key's volume in bytes (D2).

**Complexity.** The same for every pair: the thresholds are two constants, and the code paths (promotion, demotion,
hysteresis) are the same for all of them. What changes is the size of the inline blob an Add or a Remove rewrites,
which is at most the promotion threshold, and how few entries a parent leaf may hold (two with 4,074).

**What the data separates** (both systems agree):

- **512 is the worst in space** wherever a key's values pass 512 bytes but would stay small: in n100 (about 1.8 KB
  per key) each key moves to a subtree of its own, one page mostly empty, so the store takes 10,141 pages against
  3,442 (×2.9) and its file 128 MiB against 32 MiB; Contains costs 4.0 µs against 3.1–3.2 µs on Linux and 4.3 µs
  against 3.4–3.5 µs on Windows, and Add 28.3–28.6 µs against 23.2–23.4 µs on Windows (the same on Linux). In zipf,
  6,288 pages against 4,400 (+43%).
- **4,074 against 2,048**: 5% fewer pages in zipf (4,188 against 4,400) and 4.11 pages per Contains against 4.17; Add
  in zipf 2–6% faster (Windows 30.8 µs against 32.2–32.8 µs; Linux 42.8–44.3 µs against 43.8–44.2 µs), no difference
  beyond the noise elsewhere. In the oscillating load, each operation at the threshold costs more the larger the
  inline blob it rewrites: with 2,048, 10.7–11.6 µs on Linux and 1.5–1.6 µs on Windows; with 4,074, 11.9–15.5 µs and
  2.6–2.7 µs (+70% on Windows); with 512, 10.2–10.4 µs and 1.1–1.2 µs.
- In the other loads (n1, n10, n1000 and above) every pair keeps the same representation for every key, so pages and
  times are the same within the noise (n10000 differs by one page level, 3.00 against 4.00 pages per Contains, from
  the parent's history while the keys were inline, M10; the times do not separate).
- **Demotion close or at half**: the data does not separate them. The oscillating load ends in a subtree with every
  pair and never crosses either demotion threshold; the sizes are the same and the times within the noise.

**Chosen: 2,048:1,024**, the provisional defaults, which stay. 512 wastes space for keys of a few hundred values;
4,074 saves 5% of pages and 2–6% of Add time in the skewed load and nothing elsewhere, at a higher cost per operation
near the threshold and with leaves of as few as two entries. Between the two demotion thresholds the data does not choose, so the
simpler one is taken, half the promotion threshold, which also leaves the widest gap against keys that move back
and forth by more than one value.

## Linux

### Load n1: 1000000 values, keys with values: 1000000

| Promotion:demotion | Add (ms) | µs per add | Size after add (bytes) | Pages in use after add | Pages per Contains | Contains (ms) | Scan keys (ms) | Scan values (ms) | Remove (ms) | Size after remove (bytes) |
|---|---|---|---|---|---|---|---|---|---|---|
| 512:448 | 47069.2 | 47.07 | 67108864 | 7729 | 3.00 | 4799.6 | 37.1 | 4869.6 | 45246.0 | 67108864 |
| 512:256 | 48520.2 | 48.52 | 67108864 | 7729 | 3.00 | 4599.3 | 39.5 | 4802.3 | 45878.3 | 67108864 |
| 2048:1792 | 47771.7 | 47.77 | 67108864 | 7729 | 3.00 | 4577.5 | 39.9 | 4764.2 | 45784.2 | 67108864 |
| 2048:1024 | 48088.9 | 48.09 | 67108864 | 7729 | 3.00 | 4438.0 | 34.9 | 4668.1 | 44771.4 | 67108864 |
| 4074:3564 | 48113.3 | 48.11 | 67108864 | 7729 | 3.00 | 4542.0 | 34.8 | 4758.0 | 47273.3 | 67108864 |
| 4074:2037 | 47128.8 | 47.13 | 67108864 | 7729 | 3.00 | 4412.8 | 34.4 | 4659.3 | 46685.4 | 67108864 |
| composite | 44252.5 | 44.25 | 67108864 | 6964 | 3.00 | 3974.9 | — | 4170.0 | 41931.0 | 67108864 |

### Load n10: 1000000 values, keys with values: 100000

| Promotion:demotion | Add (ms) | µs per add | Size after add (bytes) | Pages in use after add | Pages per Contains | Contains (ms) | Scan keys (ms) | Scan values (ms) | Remove (ms) | Size after remove (bytes) |
|---|---|---|---|---|---|---|---|---|---|---|
| 512:448 | 41005.9 | 41.01 | 33554432 | 3806 | 3.00 | 3464.1 | 6.4 | 422.7 | 41095.5 | 33554432 |
| 512:256 | 42737.1 | 42.74 | 33554432 | 3806 | 3.00 | 3391.2 | 6.6 | 401.7 | 43189.1 | 33554432 |
| 2048:1792 | 42951.0 | 42.95 | 33554432 | 3806 | 3.00 | 3381.0 | 6.6 | 398.3 | 43114.6 | 33554432 |
| 2048:1024 | 42079.8 | 42.08 | 33554432 | 3806 | 3.00 | 3475.0 | 7.7 | 380.2 | 42487.4 | 33554432 |
| 4074:3564 | 41504.6 | 41.50 | 33554432 | 3806 | 3.00 | 3404.7 | 5.9 | 400.6 | 41622.9 | 33554432 |
| 4074:2037 | 42812.9 | 42.81 | 33554432 | 3806 | 3.00 | 3452.6 | 6.1 | 389.9 | 41433.8 | 33554432 |
| composite | 42660.6 | 42.66 | 67108864 | 6835 | 3.00 | 3930.8 | — | 455.0 | 41704.6 | 67108864 |

### Load n100: 1000000 values, keys with values: 10000

| Promotion:demotion | Add (ms) | µs per add | Size after add (bytes) | Pages in use after add | Pages per Contains | Contains (ms) | Scan keys (ms) | Scan values (ms) | Remove (ms) | Size after remove (bytes) |
|---|---|---|---|---|---|---|---|---|---|---|
| 512:448 | 40080.0 | 40.08 | 134217728 | 10141 | 3.00 | 4007.5 | 0.3 | 66.5 | 40355.7 | 134217728 |
| 512:256 | 40207.6 | 40.21 | 134217728 | 10141 | 3.00 | 4068.1 | 0.3 | 64.7 | 40653.7 | 134217728 |
| 2048:1792 | 39909.9 | 39.91 | 33554432 | 3442 | 3.00 | 3225.2 | 3.7 | 59.9 | 40854.2 | 33554432 |
| 2048:1024 | 40302.7 | 40.30 | 33554432 | 3442 | 3.00 | 3141.9 | 4.0 | 60.4 | 39635.5 | 33554432 |
| 4074:3564 | 42688.1 | 42.69 | 33554432 | 3442 | 3.00 | 3238.6 | 4.5 | 52.1 | 40310.9 | 33554432 |
| 4074:2037 | 39902.8 | 39.90 | 33554432 | 3442 | 3.00 | 3205.5 | 5.2 | 53.1 | 40232.8 | 33554432 |
| composite | 40753.7 | 40.75 | 67108864 | 7022 | 3.00 | 3772.3 | — | 70.3 | 40629.6 | 67108864 |

### Load n1000: 1000000 values, keys with values: 1000

| Promotion:demotion | Add (ms) | µs per add | Size after add (bytes) | Pages in use after add | Pages per Contains | Contains (ms) | Scan keys (ms) | Scan values (ms) | Remove (ms) | Size after remove (bytes) |
|---|---|---|---|---|---|---|---|---|---|---|
| 512:448 | 44254.0 | 44.25 | 67108864 | 5020 | 4.00 | 4911.8 | 0.0 | 34.5 | 44944.8 | 67108864 |
| 512:256 | 43318.1 | 43.32 | 67108864 | 5020 | 4.00 | 5168.0 | 0.0 | 32.8 | 43324.3 | 67108864 |
| 2048:1792 | 42596.5 | 42.60 | 67108864 | 5020 | 4.00 | 4915.2 | 0.0 | 36.1 | 42101.0 | 67108864 |
| 2048:1024 | 42905.9 | 42.91 | 67108864 | 5020 | 4.00 | 5016.4 | 0.0 | 33.8 | 44379.5 | 67108864 |
| 4074:3564 | 43741.8 | 43.74 | 67108864 | 5029 | 4.00 | 4976.0 | 0.0 | 34.0 | 43879.7 | 67108864 |
| 4074:2037 | 44293.7 | 44.29 | 67108864 | 5029 | 4.00 | 4983.0 | 0.0 | 33.8 | 42967.0 | 67108864 |
| composite | 40121.2 | 40.12 | 67108864 | 6982 | 3.00 | 3755.5 | — | 34.3 | 39258.7 | 67108864 |

### Load n10000: 1000000 values, keys with values: 100

| Promotion:demotion | Add (ms) | µs per add | Size after add (bytes) | Pages in use after add | Pages per Contains | Contains (ms) | Scan keys (ms) | Scan values (ms) | Remove (ms) | Size after remove (bytes) |
|---|---|---|---|---|---|---|---|---|---|---|
| 512:448 | 44033.2 | 44.03 | 33554432 | 3438 | 3.00 | 5015.6 | 0.0 | 29.3 | 43223.5 | 33554432 |
| 512:256 | 43609.2 | 43.61 | 33554432 | 3438 | 3.00 | 4960.8 | 0.0 | 28.8 | 42614.7 | 33554432 |
| 2048:1792 | 43370.3 | 43.37 | 33554432 | 3440 | 4.00 | 5260.9 | 0.0 | 30.3 | 41419.9 | 33554432 |
| 2048:1024 | 43600.0 | 43.60 | 33554432 | 3440 | 4.00 | 5130.0 | 0.0 | 29.7 | 41901.7 | 33554432 |
| 4074:3564 | 43460.0 | 43.46 | 33554432 | 3440 | 4.00 | 5347.3 | 0.0 | 31.2 | 44416.1 | 33554432 |
| 4074:2037 | 44734.2 | 44.73 | 33554432 | 3440 | 4.00 | 5298.8 | 0.0 | 33.1 | 42685.4 | 33554432 |
| composite | 40054.0 | 40.05 | 67108864 | 7110 | 3.00 | 3719.4 | — | 29.0 | 38878.4 | 67108864 |

### Load n100000: 1000000 values, keys with values: 10

| Promotion:demotion | Add (ms) | µs per add | Size after add (bytes) | Pages in use after add | Pages per Contains | Contains (ms) | Scan keys (ms) | Scan values (ms) | Remove (ms) | Size after remove (bytes) |
|---|---|---|---|---|---|---|---|---|---|---|
| 512:448 | 43188.0 | 43.19 | 67108864 | 4167 | 4.00 | 5152.3 | 0.0 | 28.5 | 40685.5 | 67108864 |
| 512:256 | 41643.6 | 41.64 | 67108864 | 4167 | 4.00 | 5220.2 | 0.0 | 28.3 | 41850.9 | 67108864 |
| 2048:1792 | 43062.8 | 43.06 | 67108864 | 4167 | 4.00 | 5110.1 | 0.0 | 30.3 | 40490.9 | 67108864 |
| 2048:1024 | 42028.5 | 42.03 | 67108864 | 4167 | 4.00 | 5232.3 | 0.0 | 28.7 | 39789.8 | 67108864 |
| 4074:3564 | 41843.4 | 41.84 | 67108864 | 4167 | 4.00 | 5960.1 | 0.0 | 29.2 | 40282.2 | 67108864 |
| 4074:2037 | 41432.1 | 41.43 | 67108864 | 4167 | 4.00 | 5218.7 | 0.0 | 29.4 | 42363.2 | 67108864 |
| composite | 40124.6 | 40.12 | 67108864 | 6977 | 3.00 | 3940.4 | — | 34.0 | 40615.1 | 67108864 |

### Load n1000000: 1000000 values, keys with values: 1

| Promotion:demotion | Add (ms) | µs per add | Size after add (bytes) | Pages in use after add | Pages per Contains | Contains (ms) | Scan keys (ms) | Scan values (ms) | Remove (ms) | Size after remove (bytes) |
|---|---|---|---|---|---|---|---|---|---|---|
| 512:448 | 43040.5 | 43.04 | 33554432 | 4028 | 4.00 | 5602.5 | 0.0 | 34.1 | 40248.4 | 33554432 |
| 512:256 | 43657.4 | 43.66 | 33554432 | 4028 | 4.00 | 5594.0 | 0.0 | 29.5 | 40187.8 | 33554432 |
| 2048:1792 | 42722.4 | 42.72 | 33554432 | 4028 | 4.00 | 5371.5 | 0.0 | 30.6 | 41357.7 | 33554432 |
| 2048:1024 | 43323.6 | 43.32 | 33554432 | 4028 | 4.00 | 5533.4 | 0.0 | 28.9 | 41673.0 | 33554432 |
| 4074:3564 | 44398.2 | 44.40 | 33554432 | 4028 | 4.00 | 5476.4 | 0.0 | 32.3 | 42110.9 | 33554432 |
| 4074:2037 | 43348.0 | 43.35 | 33554432 | 4028 | 4.00 | 5433.4 | 0.0 | 32.8 | 40424.8 | 33554432 |
| composite | 38090.7 | 38.09 | 67108864 | 7128 | 3.00 | 3966.0 | — | 30.4 | 39927.9 | 67108864 |

### Load zipf: 1000000 values, keys with values: 80504

| Promotion:demotion | Add (ms) | µs per add | Size after add (bytes) | Pages in use after add | Pages per Contains | Contains (ms) | Scan keys (ms) | Scan values (ms) | Remove (ms) | Size after remove (bytes) |
|---|---|---|---|---|---|---|---|---|---|---|
| 512:448 | 45788.6 | 45.79 | 67108864 | 6288 | 4.29 | 5304.8 | 3.7 | 315.5 | 47438.3 | 67108864 |
| 512:256 | 45920.5 | 45.92 | 67108864 | 6288 | 4.29 | 5520.1 | 4.0 | 314.7 | 45902.2 | 67108864 |
| 2048:1792 | 44206.4 | 44.21 | 67108864 | 4400 | 4.17 | 5239.6 | 4.5 | 353.6 | 43223.2 | 67108864 |
| 2048:1024 | 43812.4 | 43.81 | 67108864 | 4400 | 4.17 | 5339.0 | 5.2 | 346.8 | 46045.3 | 67108864 |
| 4074:3564 | 44293.3 | 44.29 | 67108864 | 4188 | 4.11 | 5182.5 | 4.9 | 367.1 | 43672.7 | 67108864 |
| 4074:2037 | 42849.9 | 42.85 | 67108864 | 4188 | 4.11 | 5402.4 | 5.2 | 340.4 | 45323.6 | 67108864 |
| composite | 40412.3 | 40.41 | 67108864 | 7059 | 3.00 | 3798.5 | — | 379.2 | 40949.1 | 67108864 |

### Load oscillating: 100000 operations on one key at the promotion threshold

| Promotion:demotion | Values at the threshold | Time (ms) | µs per operation | Size (bytes) | Ends in a subtree |
|---|---|---|---|---|---|
| 512:448 | 27 | 1024.4 | 10.24 | 65536 | yes |
| 512:256 | 27 | 1035.2 | 10.35 | 65536 | yes |
| 2048:1792 | 112 | 1155.0 | 11.55 | 65536 | yes |
| 2048:1024 | 112 | 1073.3 | 10.73 | 65536 | yes |
| 4074:3564 | 225 | 1549.9 | 15.50 | 65536 | yes |
| 4074:2037 | 225 | 1187.0 | 11.87 | 65536 | yes |

## Windows

### Load n1: 1000000 values, keys with values: 1000000

| Promotion:demotion | Add (ms) | µs per add | Size after add (bytes) | Pages in use after add | Pages per Contains | Contains (ms) | Scan keys (ms) | Scan values (ms) | Remove (ms) | Size after remove (bytes) |
|---|---|---|---|---|---|---|---|---|---|---|
| 512:448 | 38559.9 | 38.56 | 67108864 | 7729 | 3.00 | 5043.3 | 36.3 | 5192.9 | 35909.6 | 67108864 |
| 512:256 | 36707.1 | 36.71 | 67108864 | 7729 | 3.00 | 4330.7 | 32.1 | 4670.8 | 33873.9 | 67108864 |
| 2048:1792 | 35141.9 | 35.14 | 67108864 | 7729 | 3.00 | 4299.4 | 33.5 | 4614.1 | 33114.6 | 67108864 |
| 2048:1024 | 37181.5 | 37.18 | 67108864 | 7729 | 3.00 | 4533.2 | 39.3 | 4923.5 | 36512.3 | 67108864 |
| 4074:3564 | 38927.9 | 38.93 | 67108864 | 7729 | 3.00 | 4279.7 | 33.4 | 4747.4 | 34238.5 | 67108864 |
| 4074:2037 | 36364.4 | 36.36 | 67108864 | 7729 | 3.00 | 4446.3 | 36.3 | 4876.5 | 40002.4 | 67108864 |
| composite | 33568.7 | 33.57 | 67108864 | 6964 | 3.00 | 4195.2 | — | 4267.6 | 30511.5 | 67108864 |

### Load n10: 1000000 values, keys with values: 100000

| Promotion:demotion | Add (ms) | µs per add | Size after add (bytes) | Pages in use after add | Pages per Contains | Contains (ms) | Scan keys (ms) | Scan values (ms) | Remove (ms) | Size after remove (bytes) |
|---|---|---|---|---|---|---|---|---|---|---|
| 512:448 | 28377.3 | 28.38 | 33554432 | 3806 | 3.00 | 3544.0 | 6.1 | 395.8 | 29996.8 | 33554432 |
| 512:256 | 27737.5 | 27.74 | 33554432 | 3806 | 3.00 | 3559.1 | 5.9 | 408.1 | 31360.4 | 33554432 |
| 2048:1792 | 27570.5 | 27.57 | 33554432 | 3806 | 3.00 | 3522.5 | 6.3 | 395.9 | 30004.2 | 33554432 |
| 2048:1024 | 28061.7 | 28.06 | 33554432 | 3806 | 3.00 | 3573.0 | 6.5 | 411.3 | 30205.3 | 33554432 |
| 4074:3564 | 28047.7 | 28.05 | 33554432 | 3806 | 3.00 | 3792.4 | 6.2 | 415.1 | 30176.3 | 33554432 |
| 4074:2037 | 28687.7 | 28.69 | 33554432 | 3806 | 3.00 | 3787.3 | 6.9 | 421.9 | 31719.3 | 33554432 |
| composite | 29685.4 | 29.69 | 67108864 | 6835 | 3.00 | 4421.8 | — | 477.3 | 29262.0 | 67108864 |

### Load n100: 1000000 values, keys with values: 10000

| Promotion:demotion | Add (ms) | µs per add | Size after add (bytes) | Pages in use after add | Pages per Contains | Contains (ms) | Scan keys (ms) | Scan values (ms) | Remove (ms) | Size after remove (bytes) |
|---|---|---|---|---|---|---|---|---|---|---|
| 512:448 | 28554.6 | 28.55 | 134217728 | 10141 | 3.00 | 4300.4 | 0.4 | 69.0 | 30767.1 | 134217728 |
| 512:256 | 28303.0 | 28.30 | 134217728 | 10141 | 3.00 | 4345.1 | 0.4 | 70.2 | 29784.2 | 134217728 |
| 2048:1792 | 23215.4 | 23.22 | 33554432 | 3442 | 3.00 | 3528.2 | 4.0 | 52.4 | 25464.9 | 33554432 |
| 2048:1024 | 23383.6 | 23.38 | 33554432 | 3442 | 3.00 | 3382.0 | 3.9 | 57.8 | 26385.1 | 33554432 |
| 4074:3564 | 23880.0 | 23.88 | 33554432 | 3442 | 3.00 | 3402.2 | 4.3 | 56.7 | 27562.5 | 33554432 |
| 4074:2037 | 24282.7 | 24.28 | 33554432 | 3442 | 3.00 | 3548.4 | 4.1 | 54.8 | 26317.0 | 33554432 |
| composite | 28875.0 | 28.88 | 67108864 | 7022 | 3.00 | 4284.2 | — | 85.6 | 27408.7 | 67108864 |

### Load n1000: 1000000 values, keys with values: 1000

| Promotion:demotion | Add (ms) | µs per add | Size after add (bytes) | Pages in use after add | Pages per Contains | Contains (ms) | Scan keys (ms) | Scan values (ms) | Remove (ms) | Size after remove (bytes) |
|---|---|---|---|---|---|---|---|---|---|---|
| 512:448 | 31100.8 | 31.10 | 67108864 | 5020 | 4.00 | 5211.1 | 0.0 | 33.3 | 31512.7 | 67108864 |
| 512:256 | 31291.5 | 31.29 | 67108864 | 5020 | 4.00 | 5174.4 | 0.0 | 34.4 | 32703.6 | 67108864 |
| 2048:1792 | 31873.1 | 31.87 | 67108864 | 5020 | 4.00 | 5397.4 | 0.0 | 35.0 | 31167.0 | 67108864 |
| 2048:1024 | 30749.5 | 30.75 | 67108864 | 5020 | 4.00 | 5233.0 | 0.0 | 32.5 | 32398.6 | 67108864 |
| 4074:3564 | 31179.6 | 31.18 | 67108864 | 5029 | 4.00 | 5176.6 | 0.0 | 33.1 | 30612.8 | 67108864 |
| 4074:2037 | 30628.3 | 30.63 | 67108864 | 5029 | 4.00 | 5178.1 | 0.0 | 34.3 | 30711.5 | 67108864 |
| composite | 27393.1 | 27.39 | 67108864 | 6982 | 3.00 | 4082.2 | — | 45.7 | 26719.6 | 67108864 |

### Load n10000: 1000000 values, keys with values: 100

| Promotion:demotion | Add (ms) | µs per add | Size after add (bytes) | Pages in use after add | Pages per Contains | Contains (ms) | Scan keys (ms) | Scan values (ms) | Remove (ms) | Size after remove (bytes) |
|---|---|---|---|---|---|---|---|---|---|---|
| 512:448 | 30469.6 | 30.47 | 33554432 | 3438 | 3.00 | 5237.8 | 0.0 | 28.4 | 29354.7 | 33554432 |
| 512:256 | 30226.9 | 30.23 | 33554432 | 3438 | 3.00 | 5238.8 | 0.0 | 28.8 | 28970.7 | 33554432 |
| 2048:1792 | 30973.7 | 30.97 | 33554432 | 3440 | 4.00 | 5432.1 | 0.0 | 28.9 | 29670.6 | 33554432 |
| 2048:1024 | 30853.9 | 30.85 | 33554432 | 3440 | 4.00 | 5502.1 | 0.0 | 28.9 | 29984.6 | 33554432 |
| 4074:3564 | 30898.9 | 30.90 | 33554432 | 3440 | 4.00 | 5534.5 | 0.0 | 31.0 | 29759.7 | 33554432 |
| 4074:2037 | 31079.9 | 31.08 | 33554432 | 3440 | 4.00 | 5512.5 | 0.0 | 29.7 | 30997.9 | 33554432 |
| composite | 26840.9 | 26.84 | 67108864 | 7110 | 3.00 | 4214.9 | — | 40.5 | 26108.2 | 67108864 |

### Load n100000: 1000000 values, keys with values: 10

| Promotion:demotion | Add (ms) | µs per add | Size after add (bytes) | Pages in use after add | Pages per Contains | Contains (ms) | Scan keys (ms) | Scan values (ms) | Remove (ms) | Size after remove (bytes) |
|---|---|---|---|---|---|---|---|---|---|---|
| 512:448 | 29127.6 | 29.13 | 67108864 | 4167 | 4.00 | 5400.2 | 0.0 | 26.9 | 28671.1 | 67108864 |
| 512:256 | 29021.4 | 29.02 | 67108864 | 4167 | 4.00 | 5363.6 | 0.0 | 29.1 | 28469.8 | 67108864 |
| 2048:1792 | 29988.5 | 29.99 | 67108864 | 4167 | 4.00 | 5325.5 | 0.0 | 31.5 | 28268.7 | 67108864 |
| 2048:1024 | 29155.3 | 29.16 | 67108864 | 4167 | 4.00 | 5363.5 | 0.0 | 29.2 | 28352.6 | 67108864 |
| 4074:3564 | 29141.7 | 29.14 | 67108864 | 4167 | 4.00 | 5470.3 | 0.0 | 28.9 | 28436.1 | 67108864 |
| 4074:2037 | 29078.1 | 29.08 | 67108864 | 4167 | 4.00 | 5413.9 | 0.0 | 29.7 | 28528.5 | 67108864 |
| composite | 26260.4 | 26.26 | 67108864 | 6977 | 3.00 | 4075.8 | — | 37.4 | 26366.7 | 67108864 |

### Load n1000000: 1000000 values, keys with values: 1

| Promotion:demotion | Add (ms) | µs per add | Size after add (bytes) | Pages in use after add | Pages per Contains | Contains (ms) | Scan keys (ms) | Scan values (ms) | Remove (ms) | Size after remove (bytes) |
|---|---|---|---|---|---|---|---|---|---|---|
| 512:448 | 29311.0 | 29.31 | 33554432 | 4028 | 4.00 | 5601.3 | 0.0 | 28.7 | 28287.1 | 33554432 |
| 512:256 | 29585.7 | 29.59 | 33554432 | 4028 | 4.00 | 6372.9 | 0.0 | 30.5 | 28392.9 | 33554432 |
| 2048:1792 | 29229.1 | 29.23 | 33554432 | 4028 | 4.00 | 5616.0 | 0.0 | 29.3 | 28309.8 | 33554432 |
| 2048:1024 | 29848.1 | 29.85 | 33554432 | 4028 | 4.00 | 5612.9 | 0.0 | 29.8 | 29077.0 | 33554432 |
| 4074:3564 | 29501.9 | 29.50 | 33554432 | 4028 | 4.00 | 5613.7 | 0.0 | 28.4 | 28611.0 | 33554432 |
| 4074:2037 | 29215.6 | 29.22 | 33554432 | 4028 | 4.00 | 5552.1 | 0.0 | 29.3 | 28559.7 | 33554432 |
| composite | 25888.4 | 25.89 | 67108864 | 7128 | 3.00 | 4118.9 | — | 35.8 | 26112.6 | 67108864 |

### Load zipf: 1000000 values, keys with values: 80504

| Promotion:demotion | Add (ms) | µs per add | Size after add (bytes) | Pages in use after add | Pages per Contains | Contains (ms) | Scan keys (ms) | Scan values (ms) | Remove (ms) | Size after remove (bytes) |
|---|---|---|---|---|---|---|---|---|---|---|
| 512:448 | 34354.2 | 34.35 | 67108864 | 6288 | 4.29 | 5363.4 | 4.0 | 318.6 | 35841.5 | 67108864 |
| 512:256 | 34361.0 | 34.36 | 67108864 | 6288 | 4.29 | 5616.8 | 4.2 | 332.7 | 36452.5 | 67108864 |
| 2048:1792 | 32182.7 | 32.18 | 67108864 | 4400 | 4.17 | 5462.5 | 4.4 | 325.6 | 34158.6 | 67108864 |
| 2048:1024 | 32847.2 | 32.85 | 67108864 | 4400 | 4.17 | 5670.3 | 4.5 | 334.6 | 34795.2 | 67108864 |
| 4074:3564 | 30845.5 | 30.85 | 67108864 | 4188 | 4.11 | 5362.4 | 4.6 | 345.4 | 33487.0 | 67108864 |
| 4074:2037 | 30773.7 | 30.77 | 67108864 | 4188 | 4.11 | 5475.4 | 4.6 | 325.9 | 33756.7 | 67108864 |
| composite | 27870.1 | 27.87 | 67108864 | 7059 | 3.00 | 4052.3 | — | 371.8 | 27082.5 | 67108864 |

### Load oscillating: 100000 operations on one key at the promotion threshold

| Promotion:demotion | Values at the threshold | Time (ms) | µs per operation | Size (bytes) | Ends in a subtree |
|---|---|---|---|---|---|
| 512:448 | 27 | 112.1 | 1.12 | 65536 | yes |
| 512:256 | 27 | 117.5 | 1.18 | 65536 | yes |
| 2048:1792 | 112 | 152.0 | 1.52 | 65536 | yes |
| 2048:1024 | 112 | 157.4 | 1.57 | 65536 | yes |
| 4074:3564 | 225 | 258.3 | 2.58 | 65536 | yes |
| 4074:2037 | 225 | 266.1 | 2.66 | 65536 | yes |
