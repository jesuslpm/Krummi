# Large values (5.3) — measurement (V11) and leaf-threshold evaluation (V4)

Source data for the closing report (AC-V13, AC-V14). Task T5 of `odd/tasks/trees-large-values.md`, measured on
2026-10-27 on the working tree of branch `feat/trees-large-values` after `9000d9e`.

## How to rerun

```text
cargo run --release -p krummi --example value_timing [folder]
```

`crates/krummi/examples/value_timing.rs` runs V11 exactly: on disk, durable commits, random 16-byte keys and random
incompressible value bytes, both with fixed seeds. For each value size of 1, 2, 3, 4, 6 and 8 KiB, 64 KiB, 1 MiB and
8 MiB, a new store gets 256 MiB of values in random key order, in 16 writes of 16 MiB of values (16,384 entries per
write with 1 KiB, 2 with 8 MiB; with 3 and 6 KiB, which do not divide 16 MiB, the whole entries that fit, 5,461 and
2,730, so 255.98 and 255.94 MiB); the store's size comes from its information (P12) after the last commit; the store
is closed and reopened without dropping the OS cache, and every value is read at random in one read transaction,
comparing every byte with the stored one. Then, in another new store, the mixed load: 1,000,000 keys, 90% with
100-byte values and 10% with 2,048 to 8,192 bytes (every byte count equally likely, the large entries spread at
random; 601,705,554 bytes of values), in writes of 1,000 entries: the time to store, the store's size, and, reopened,
the time to read every key at random and to scan the whole tree forward reading every byte. Release build. No
threshold (D5).

Besides the store's size, which grows by doubling the file and so moves in steps, the example prints the **pages in
use** (the store's pages minus its free pages, as AC-A15 takes them), which show the space each choice takes more
finely.

Runs: Windows with the built binary and the store folder in `%TEMP%\krummi-t5`; Linux in WSL 2, built from
`/mnt/c/projects/Krummi` with `CARGO_TARGET_DIR=~/krummi-t5-target`, the stores in `~/krummi-t5-stores` (ext4). Wall
clock of one run: 70 s to 78 s on either system.

## Machine

Both environments on the same machine (queried on 2026-10-27 with `Get-CimInstance`, `Get-PhysicalDisk`, `uname`,
`lscpu`, `df` and `/etc/os-release`):

| Environment | Operating system | Processor | Memory | Disk |
|---|---|---|---|---|
| Windows | Windows 11 Pro 10.0.26200 | 13th Gen Intel Core i9-13980HX (32 threads) | 64 GiB | NVMe Samsung MZVL22T0HBLB-00B00 (SSD) |
| Linux | WSL 2 Ubuntu 24.04.1 LTS, kernel 5.15.133.1-microsoft-standard-WSL2, ext4 | the same (`lscpu`: 32 CPUs) | the same | WSL 2 virtual ext4 disk (`/dev/sde`, 1,007 GB) on the same NVMe |

## V11 measurement (the engine as delivered: half-page threshold)

Final runs of the shipped code (threshold knob and full-page prototype removed): Windows 21:15 UTC, Linux 21:16 UTC.

### Values of one size, 256 MiB per store

| Value size | Entries | Windows store (s) | Windows read at random (s) | Linux store (s) | Linux read at random (s) | Store size (bytes) | Pages in use | Pages in use per value |
|---|---|---|---|---|---|---|---|---|
| 1 KiB | 262,144 | 6.24 | 1.22 | 5.58 | 1.24 | 536,870,912 | 51,921 | 0.198 |
| 2 KiB | 131,072 | 4.23 | 0.69 | 3.55 | 0.61 | 536,870,912 | 56,466 | 0.431 |
| 3 KiB | 87,376 | 3.56 | 0.52 | 2.97 | 0.44 | 536,870,912 | 58,392 | 0.668 |
| 4 KiB | 65,536 | 1.73 | 0.46 | 1.46 | 0.35 | 1,073,741,824 | 66,011 | 1.007 |
| 6 KiB | 43,680 | 1.22 | 0.33 | 1.08 | 0.21 | 536,870,912 | 43,965 | 1.007 |
| 8 KiB | 32,768 | 1.47 | 0.32 | 1.24 | 0.18 | 1,073,741,824 | 65,775 | 2.007 |
| 64 KiB | 4,096 | 0.83 | 0.14 | 0.93 | 0.07 | 536,870,912 | 36,901 | 9.009 |
| 1 MiB | 256 | 0.69 | 0.10 | 0.83 | 0.03 | 283,115,520 | 33,032 | 129.031 |
| 8 MiB | 32 | 0.64 | 0.10 | 0.82 | 0.04 | 270,270,464 | 32,806 | 1,025.188 |

Throughput of the load, in MiB of values per second: Windows 41.0, 60.5, 71.8, 148.1, 209.7, 173.7, 310.3, 369.2 and
402.0; Linux 45.9, 72.1, 86.3, 175.5, 236.0, 206.2, 276.3, 307.8 and 310.7 (same order of sizes). Microseconds per
read: Windows 4.65, 5.25, 5.97, 7.09, 7.55, 9.79, 33.45, 392.89 and 3,249.30; Linux 4.73, 4.66, 5.06, 5.39, 4.80,
5.57, 16.60, 136.38 and 1,329.89. The store size and the pages in use are the same on both systems.

### Mixed load (1,000,000 entries, 10% of 2 to 8 KiB)

| Operation | Windows (s) | Windows (µs per entry) | Linux (s) | Linux (µs per entry) |
|---|---|---|---|---|
| Store at random, writes of 1,000 | 33.17 | 33.17 | 44.69 | 44.69 |
| Read every key at random | 4.79 | 4.79 | 4.68 | 4.68 |
| Scan the whole tree forward | 0.43 | 0.43 | 0.23 | 0.23 |

Store size after the load: 1,073,741,824 bytes on both; pages in use 107,046 (0.1070 per entry).

### Observations

- From 1 to 3 KiB a value stays in its leaf; from 4 KiB on it lives off its leaf (an entry with a 16-byte key stays
  inline up to 4,058 bytes of value). The load gets faster with the value size, because the cost per entry (the
  tree descent and the leaf copy) is spread over more bytes; reads cost about 5 µs per key up to 8 KiB.
- A value of exactly 8 KiB or 64 KiB takes one page more than its bytes (2.007 and 9.009 pages): the run's 16-byte
  header (D1) makes 8,192 + 16 bytes need two pages. It is within V5's bound (bytes / 8,192 rounded up, plus one).
- The store's size moves by doubling the file: 256 MiB of values take 512 MiB or 1 GiB depending on whether the
  pages in use pass 65,536. The pages in use are the finer measure of space.
- Run-to-run variation, from the repeated half-page runs of the same day: Windows 4 KiB load 1.84, 1.84 and 1.73 s,
  mixed load 34.02, 32.83 and 33.17 s; Linux mixed load 45.15, 44.93 and 44.69 s. Reads of the same size vary up to
  about 15% between runs (Linux 1 KiB: 1.24 s and 1.42 s with identical placement), so read differences under that
  are noise.

## V4 evaluation: leaf threshold

### Alternatives

- **Half page (shipped in T3).** An entry stays in its leaf when its cell, `4 + key + value`, is at most 4,078 bytes
  (`MAX_LEAF_CELL`, half a node), so a leaf always holds two of the longest cells and the 2-way split by bytes always
  fits (D4).
- **Full page (prototype).** An entry stays in its leaf when its cell fits in an empty leaf: at most 8,158 bytes
  (`PAYLOAD_SIZE - HEADER - 2`). A leaf may then hold a single entry, and an insertion between two entries of a leaf
  that no 2-way split can share needs a split that produces three nodes.

The prototype was built on the working tree after the L9 fixes: the threshold knob raised to 8,158, `is_valid`
accepting leaf cells up to 8,158 bytes (format 7 unchanged, readable both ways), `split_point` returning `None` when
no 2-way split fits, `split` then splitting the leaf at the insertion point into three nodes (the cells before the new
one, the new one alone, the cells after it, each a part of a page that held them), `Outcome::Split` carrying a second
sibling, parents inserting one or two branch cells (`insert_cells`, which splits the branch 2-way when they do not
fit; with branch cells of at most 2,060 bytes this always fits) and a new root built with two cells. Two tests,
observed RED first (both panicked at `split_point`'s "both halves fit" assertion) and then GREEN: a 3-way split of a
root leaf, and a random model test against `BTreeMap` with values of 0 to 8,199 bytes under the full-page threshold
that grows to three or more levels and shrinks back (23 three-way splits in it). The whole tree crate passed with the
prototype.

### Data (paired runs: same binary, back to back, only the threshold differs)

Only the sizes whose placement differs between the thresholds change: 4 KiB and 6 KiB values, and the mixed load's
values of 4,059 to 8,138 bytes (with 16-byte keys). The other sizes have the same placement and pages in both, and
their differences are run-to-run noise.

Windows (half 21:01 UTC, full 21:02 UTC):

| Load | Half: store (s) | Full: store (s) | Half: read (s) | Full: read (s) | Half: pages in use | Full: pages in use |
|---|---|---|---|---|---|---|
| 4 KiB values | 1.84 | 3.33 (+81%) | 0.46 | 0.40 (−13%) | 66,011 | 65,858 (−0.2%) |
| 6 KiB values | 1.22 | 2.22 (+82%) | 0.34 | 0.29 (−15%) | 43,965 | 43,917 (−0.1%) |
| Mixed load | 32.83 | 36.22 (+10%) | 4.77 | 5.34 (+12%) | 107,046 | 110,805 (+3.5%) |
| Mixed load, scan forward | — | — | 0.41 | 0.45 (+10%) | — | — |

Linux (half 21:07 UTC, full 21:08 UTC):

| Load | Half: store (s) | Full: store (s) | Half: read (s) | Full: read (s) | Half: pages in use | Full: pages in use |
|---|---|---|---|---|---|---|
| 4 KiB values | 1.39 | 2.47 (+78%) | 0.32 | 0.38 (+19%) | 66,011 | 65,858 (−0.2%) |
| 6 KiB values | 1.14 | 2.04 (+79%) | 0.22 | 0.23 (+5%) | 43,965 | 43,917 (−0.1%) |
| Mixed load | 44.93 | 46.67 (+4%) | 4.86 | 5.28 (+9%) | 107,046 | 110,805 (+3.5%) |
| Mixed load, scan forward | — | — | 0.24 | 0.22 (−8%) | — | — |

Full tables of these four runs (all sizes): Windows half — 1 KiB 6.34 s / 1.23 s, 2 KiB 4.47 / 0.71, 3 KiB 3.69 /
0.51, 8 KiB 1.47 / 0.31, 64 KiB 0.81 / 0.12, 1 MiB 0.67 / 0.11, 8 MiB 0.69 / 0.10; Windows full — 6.51 / 1.26, 4.44 /
0.72, 3.80 / 0.53, 1.49 / 0.34, 0.84 / 0.13, 0.73 / 0.11, 0.62 / 0.10; Linux half — 5.51 / 1.24, 3.55 / 0.63, 3.02 /
0.41, 1.24 / 0.20, 0.83 / 0.06, 0.84 / 0.04, 0.86 / 0.05; Linux full — 5.39 / 1.42, 3.78 / 0.74, 3.17 / 0.48, 1.27 /
0.21, 0.91 / 0.07, 0.88 / 0.05, 0.88 / 0.05 (store / read at random, seconds); pages in use identical between the
thresholds for these sizes.

### Reading of the data

- **Performance.** Storing values that the full page keeps inline (4 and 6 KiB) is about 80% slower on both
  systems: such an entry fills its leaf, so nearly every insertion splits a leaf and adds a branch cell, copying a
  whole leaf, where the half page writes the value into a new run and inserts a 36-byte cell. Random reads of those
  values gain at most 15% on Windows and lose up to 19% on Linux, within the run-to-run noise. In the mixed load, the
  realistic case, the full page is slower everywhere but in one scan: store +4% to +10%, read +9% to +12%, scan +10%
  on Windows and −8% on Linux.
- **Space.** For values of one size both take the same pages (one per value; the full page saves the 36-byte cells,
  0.2%). In the mixed load the full page takes 3.5% more pages: a leaf with a large inline entry has little room
  for small ones, and a 3-way split can leave the cells before and after the new entry in two partly empty leaves.
- **Complexity.** The half page adds nothing beyond T3: the threshold is one constant and the 2-way split is
  unchanged. The full page added 84 lines and removed 38 in `lib.rs` and `node.rs` (measured against the tree with
  the L9 fixes), plus 81 lines of tests; new code paths: the 3-way leaf split, a split outcome with two siblings,
  parents inserting two cells (and splitting while doing so), a root built with two cells, and wider leaf-cell
  validation. It also drops D4's invariant that a leaf holds at least two entries and lets a split leave underfull
  leaves. Keeping it would need tests of the 3-way split at every level (root leaf, leaf under a branch, a branch
  split by two cells, a root split in three), merges of one-entry leaves, older reads across 3-way splits, and the
  crash and T11 sweeps over the extra allocation of a 3-way split, plus re-deriving the AC-V2 boundaries.

### Choice

**Half page.** It is faster to store (about 80% on the values it places differently, 4% to 10% on the mixed load),
no slower to read beyond noise, 3.5% smaller on the mixed load and equal on single sizes, and it adds no code or
tests. The full page gains nothing measurable that would pay for a 3-way split and the loss of D4's two-entries
invariant. The full-page prototype was removed, and so was the threshold knob (`set_inline_limit`), which only
served this evaluation: the threshold is the constant `MAX_LEAF_CELL` (4,078 bytes of cell), and the evaluation data
stays in this file.
