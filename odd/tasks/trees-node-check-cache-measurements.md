# Remembered node layout check — measurement (C9)

Source data for the closing report (AC-C10). Task T3 of `odd/tasks/trees-node-check-cache.md`, measured on
2026-10-30 with the example of the T3 commit on the engine of `6a1396b`. Run by hand; neither the tests nor CI run it.

## How to rerun

```text
cargo run --release -p krummi --example node_check_timing -- [--folder <folder>] [--reads N] [--values N] [--rounds N]
```

Stores on disk, durable commits, closed and reopened before every round so that each round starts with nothing
remembered. Each case alternates rounds without and with remembering (the hidden switch
`krummi_pager::set_remember_checked`) and reports the fastest of three of each. **Leaf**: a normal tree whose only
node is its root leaf, with 1, 100 or 370 entries (keys of 16, 72 and 16 bytes, empty values), and 1,000,000 Gets
of its keys at random in one read transaction. **Contains**: 200,000 values of 8 bytes in keys of 64 bytes with 1,000
values each (every key in a subtree), checked at random in one read transaction, against the same values in a
normal tree with the key followed by the value as its key (Get). No threshold (D5).

## Machine

The same as `trees-multivalue-measurements.md`: Windows 11 Pro 10.0.26200 and WSL 2 Ubuntu 24.04.1 LTS (ext4 on a
virtual disk) on one machine, 13th Gen Intel Core i9-13980HX (32 threads), 64 GiB, NVMe Samsung MZVL22T0HBLB-00B00.
Windows ran 09:34:46–09:37:16 with the stores in `work/nct/stores` and again 09:38:17–09:40:44 with them in the
system's temporary folder; Linux ran 09:37:17–09:38:00, one after the other.

## Reading

- **Dense leaves**: remembering removes most of the cost of a Get on a leaf of 370 entries (−86% on Linux, −88% and
  −90% on Windows) and about half on one of 100 entries (−64% on Linux, −47% and −59% on Windows).
- **Contains**: −66% on Linux and −71% to −72% on Windows in the multi-value tree with its values in subtrees, and
  −44% on Linux and −36% to −55% on Windows in the composite tree. With remembering, the multi-value tree costs about
  the same as the composite one (1.85 against 1.61 µs on Linux; 7.39 against 5.85–7.19 µs on Windows), where without
  it it cost about twice as much.
- **A leaf of one entry**: its layout check costs less than the lookup, so remembering adds 40 ns on Linux (+13%,
  311 against 351 ns) and is within the noise on Windows (−4% and +5%). Such leaves are rare (a leaf holds at least
  two entries of the largest size), so every node uses the memory, without a minimum number of entries.
- **Windows against Linux**: Windows takes 4 to 5 times as long in both modes, and what remembering saves on a leaf of
  370 entries on Windows (12.5 and 14.0 µs per Get in the two runs) is more than the isolated cost of the layout check measured before
  the PRD (2.5–2.9 µs per node on Windows). On Linux the saving (2.5 µs) matches it. The cause is not established;
  the relative effect agrees on both systems.

## Size of the table

65,536 slots per open store (512 KiB), one per page number modulo that count: enough for the nodes of a hot working
set of 512 MiB without two of them sharing a slot. The loads here touch far fewer pages, so they do not separate
sizes; the size stays, and the measurement shows no cost from it.

## Windows (stores in `work/nct/stores`)

### Leaf: Get of the only node of a tree

| Entries in the leaf | ns per Get, without | ns per Get, with | Change |
|---|---|---|---|
| 1 | 381 | 365 | -4% |
| 100 | 4013 | 1641 | -59% |
| 370 | 14264 | 1779 | -88% |

### Contains: every value at random, keys of 1000 values in subtrees

| Tree | ns per check, without | ns per check, with | Change |
|---|---|---|---|
| multi-value | 25729 | 7387 | -71% |
| composite | 11261 | 7194 | -36% |

## Windows, rerun (stores in the system's temporary folder)

### Leaf: Get of the only node of a tree

| Entries in the leaf | ns per Get, without | ns per Get, with | Change |
|---|---|---|---|
| 1 | 358 | 375 | +5% |
| 100 | 3389 | 1788 | -47% |
| 370 | 15646 | 1615 | -90% |

### Contains: every value at random, keys of 1000 values in subtrees

| Tree | ns per check, without | ns per check, with | Change |
|---|---|---|---|
| multi-value | 26613 | 7386 | -72% |
| composite | 13027 | 5845 | -55% |

## Linux (WSL 2 Ubuntu)

### Leaf: Get of the only node of a tree

| Entries in the leaf | ns per Get, without | ns per Get, with | Change |
|---|---|---|---|
| 1 | 311 | 351 | +13% |
| 100 | 1128 | 405 | -64% |
| 370 | 2888 | 418 | -86% |

### Contains: every value at random, keys of 1000 values in subtrees

| Tree | ns per check, without | ns per check, with | Change |
|---|---|---|---|
| multi-value | 5394 | 1848 | -66% |
| composite | 2858 | 1613 | -44% |
