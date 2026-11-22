# Krummi usage guides

Step-by-step guides to using Krummi from a Rust application: how to open a store, run transactions, and
store data in trees and tables, with every operation and data type explained. Start with
[Getting started](getting-started.md), then read the guide for the structure you need.

Every Rust example in these guides is a complete program. Each one runs as a doctest of the `krummi` crate, so
`cargo test --workspace` checks that it still compiles and gives the results it shows.

## Reading order

| # | Guide | Read it to learn how to |
|---|---|---|
| 1 | [Getting started](getting-started.md) | add Krummi to a project, write and read your first data, and close the store |
| 2 | [Stores](stores.md) | open a store on disk or in memory, choose its options, inspect it, close it, and recover it after a crash |
| 3 | [Transactions](transactions.md) | use read snapshots and the single write transaction, commit or roll back, and control durability |
| 4 | [Errors](errors.md) | tell the errors apart by their kind and know what to do about each one |

## Trees: keys and values in bytes

| Guide | Use it when |
|---|---|
| [Trees](trees.md) | each byte key holds one value of up to 256 MiB, and you scan keys in order, by range or by prefix |
| [Multi-value trees](multi-value-trees.md) | each key holds a sorted set of small values, for example a hand-made index |
| [Fixed-size trees](fixed-size-trees.md) | keys are unsigned 64-bit integers and every value has the same length |

## Tables: typed rows

| Guide | Use it when |
|---|---|
| [Tables and schemas](tables.md) | you define, open, describe and change tables of typed columns with a primary key |
| [Rows](rows.md) | you insert, update, read, delete, scan, bulk load and project rows |
| [Data types](data-types.md) | you choose a column type and need its values, limits and sort order |
| [JSON columns](json.md) | you store JSON documents, then navigate them and read their values |
| [Secondary indexes](indexes.md) | you look up or scan rows by columns other than the primary key |
| [Sequences and identifiers](sequences-and-ids.md) | you generate keys: sequences, `ulid` and `uuid` values |
| [Statistics and estimates](statistics.md) | you need to know how many rows a key range or condition covers |

## Where to go next

- The behavior of each module, in full, is in its PRD, under [docs/prd/krummi/](../prd/krummi/krummi.prd.en.md).
- The API reference is the crate documentation: `cargo doc -p krummi --open`.
