# Statistics and estimates

Krummi keeps statistics of the columns of each table, a summary of their values like those of SQL Server, and uses
them to **estimate** how many rows meet a condition: a value in a column, an interval of a column, or values in several
columns. This guide covers what the statistics hold, when they are built (by an estimate, by a background thread or by
**analyze**), how to read them, how to estimate, and what the estimates promise. Use it to decide between scanning a
table and scanning an index, or to size a result before reading it. Statistics are approximate and may be stale: they
are a guide, never an exact count.

## Quick path

1. Store rows in a table as usual (see [rows](rows.md)).
2. Ask for an estimate with `estimate(&table, &condition)`, in a read or a write. The first time, without statistics,
   it reads a sample of the table at once and answers with it.
3. Later estimates, in any transaction, reuse those statistics; a background thread stores them in the table.
4. Read the statistics themselves with `column_statistics` or `index_statistics` when you need the details.
5. When you want fresh statistics without waiting for them to go stale, call `Store::analyze` or
   `WriteTransaction::analyze`.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Condition, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("city", ColumnType::String)],
        &["id"],
    );
    let table = write.open_table_with("people", TableMode::CreateNew(&schema))?;
    let cities = ["Oslo", "Lima", "Oslo", "Rome", "Oslo"];
    for id in 0..1_000i64 {
        let city = Value::String(cities[(id % 5) as usize].into());
        write.insert_row(&table, &[("id", Value::I64(id)), ("city", city)])?;
    }
    write.commit()?;

    let read = store.read()?;
    let table = read.open_table("people")?;
    assert!(read.column_statistics(&table, "city")?.is_none());
    // No statistics yet: the estimate samples the table now and answers with what it read.
    let oslo = Condition::Equal { column: "city", value: Value::String("Oslo".into()) };
    let estimate = read.estimate(&table, &oslo)?;
    assert!(estimate.rows >= 0.0 && estimate.rows <= 1_000.0);
    assert!(!estimate.stale);
    // The sample stays in the store's memory for every transaction.
    assert!(read.column_statistics(&table, "city")?.is_some());
    Ok(())
}
```

## What Krummi keeps

Every column of every table, with an index or not, can have statistics, except vector and JSON columns. Each index
with two or more key columns also has statistics of the prefixes of its key. There are no other statistics of several
columns together.

The statistics and estimates of a `string` column with the `ci` or `ci_ai` collation work on folded forms: `Ana` and
`ANA` are one distinct value, the frequent values and histogram bounds you read are folded forms, an estimate gives
the same for any spelling, and a text is large when it or its folded form passes 2,048 bytes.

A table of up to 30,000 rows is read whole; a larger one through a sample. Statistics are built from one snapshot, the
**source snapshot**, and carry the table's row count and modification counter in it.

`ColumnStatistics`, the statistics of a column:

| Field | Holds |
|---|---|
| `origin_rows` | The table's rows in the source snapshot. |
| `rows_read` | The rows read to build them: all of them, up to 30,000. |
| `origin_counter` | The table's modification counter in the source snapshot. |
| `stale` | Whether they are stale for the transaction that reads them (see below). |
| `null_fraction` | The fraction of rows where the column is null, from 0 to 1. |
| `large_fraction` | The fraction of large values: `string` or `bytes` of more than 2,048 bytes. Zero in other columns. |
| `distinct` | The estimated number of distinct non-null values. |
| `frequent` | The most frequent values with their fractions, `Vec<(Value, f64)>`, from the most frequent; equal fractions in the column's order. May be empty. |
| `histogram` | The other non-null values, `Vec<HistogramBucket>`, in the column's order. May be empty. |

A `HistogramBucket` has `upper`, the greatest value of the bucket, `fraction`, its fraction of the rows, and
`shortened`, whether `upper` is only a prefix of that value (text and bytes bounds may be cut, text at a character
boundary).

Fractions are of the source rows. In a table with rows, the null fraction, the large fraction and the fractions of the
frequent values and of the buckets add up to 1, up to rounding. In a table of zero rows everything is zero and the
lists are empty. Values are returned in copies of their own, normalized as a row's (a `uuid` of SQL Server's order in
the byte order of RFC 9562).

Large values are never read: sampling only counts them. To estimate equalities on long texts precisely, add a column
with their hash and estimate on it.

`PrefixStatistics`, the statistics of a prefix of an index's key:

| Field | Holds |
|---|---|
| `columns` | The number of key columns of the prefix, from 2 up to all. |
| `origin_rows`, `rows_read`, `origin_counter`, `stale` | As in `ColumnStatistics`. |
| `distinct` | The estimated number of distinct combinations of the prefix's values, a null counting as one more value. |
| `frequent` | The most frequent combinations, `Vec<(Vec<Value>, f64)>`, each in the order of the prefix's columns (any value may be null), from the most frequent; equal fractions in the index's order. |

How many frequent values, buckets and frequent combinations are kept, the sample size of a large table and how far a
bound is shortened (always under 1,000 bytes) are chosen by Krummi and cannot be configured.

## When statistics are built

Statistics are built only when something asks for them. There is no option in `StoreOptions` or elsewhere to turn this
off or to tune it.

| Who | When | Where it reads | Where the result goes |
|---|---|---|---|
| An estimate | It finds no statistics it needs, or some of fewer than 500 rows when the table now has more than double | The committed snapshot the transaction sees (a write reads what a new read would see, without its own changes) | The store's memory at once; then the background thread stores them in the table |
| The background thread | An estimate found stale statistics | A new read snapshot | Memory, then stored in the table |
| `Store::analyze` | You call it | A new read snapshot | Memory, then stored with a short write of its own; it returns when they are stored |
| `WriteTransaction::analyze` | You call it | What the write sees, its changes included | Stored inside the write, committed or aborted with it; not to memory |

The background thread:

- Each store has at most one, started by the first request and ended when the store closes. Closing stops a sampling
  in progress and drops the queued work; what was only in memory is lost.
- It stores statistics with a short write of its own that waits for the write turn like any write, so a write of yours
  may wait for it, and that wait counts toward the write's time limit.
- It never blocks readers: sampling reads a snapshot like any read transaction.
- Its failures are silent: statistics in memory keep being used, and the next estimate asks again.
- In a store that does not accept writes, no work is requested.

Which statistics a transaction sees: of each column or prefix, those with the highest source counter among the stored
ones its snapshot sees and those in the store's memory. Statistics never replace newer ones. Each estimate looks again,
so two estimates in one transaction may use different statistics if newer ones appeared in between. A write that
samples a table it created or emptied, or a column or index it added, reads what it sees instead and keeps what it
builds to itself until it ends.

Statistics work the same in a temporary store, in memory.

## The modification counter and stale statistics

Each table has a modification counter that starts at zero and never decreases. It is committed with the rows it
counts, survives closing and crashes, and follows the table when it is renamed.

| Adds to the counter | Does not |
|---|---|
| Insert: one per row | An identical update |
| Update or upsert: one per row inserted or changed | Updating or deleting a row that is not there |
| Delete a row that was there: one | A failed operation |
| Delete a range: the rows deleted | Schema changes, creating or dropping an index |
| Empty the table: the rows removed | Renaming the table, sequences, analyze |
| Bulk load: the rows inserted | |

You do not read the counter directly: you see it as `origin_counter` and through `stale`.

Statistics are **stale** for a transaction when the counter it sees, minus their `origin_counter`, exceeds SQL
Server's threshold, computed with their `origin_rows`:

| `origin_rows` | Modifications that make them stale |
|---|---|
| Up to 500 | More than 500 |
| More than 500 | More than the smaller of 500 + 20 % of the rows and √(1,000 × rows) |

For example, statistics of 1,000 rows go stale after more than 700 modifications. A write counts its own changes, so
it can see statistics go stale before it commits. An estimate keeps using stale statistics, says so in its `stale`
field, and asks the background thread to refresh them without waiting.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Condition, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("n", ColumnType::I64)],
        &["id"],
    );
    let table = write.open_table_with("t", TableMode::CreateNew(&schema))?;
    for id in 0..1_000i64 {
        write.insert_row(&table, &[("id", Value::I64(id)), ("n", Value::I64(id % 10))])?;
    }
    write.analyze(&table, None)?;
    let stats = write.column_statistics(&table, "n")?.unwrap();
    assert_eq!((stats.origin_rows, stats.origin_counter, stats.stale), (1_000, 1_000, false));
    write.commit()?;

    // 800 more rows: more than the 700 modifications statistics of 1,000 rows allow.
    let mut write = store.write()?;
    let table = write.open_table("t")?;
    for id in 1_000..1_800i64 {
        write.insert_row(&table, &[("id", Value::I64(id)), ("n", Value::I64(id % 10))])?;
    }
    assert!(write.column_statistics(&table, "n")?.unwrap().stale);
    let estimate = write.estimate(&table, &Condition::Equal { column: "n", value: Value::I64(3) })?;
    assert!(estimate.stale);
    write.commit()?;

    // Analyze on the store builds them again from a new snapshot and stores them.
    store.analyze("t", None)?;
    let read = store.read()?;
    let table = read.open_table("t")?;
    let stats = read.column_statistics(&table, "n")?.unwrap();
    assert_eq!((stats.origin_rows, stats.stale), (1_800, false));
    Ok(())
}
```

## Estimate

`estimate(&table, &condition)` exists on `ReadTransaction` and `WriteTransaction` and returns an `Estimate`:

| Field | Holds |
|---|---|
| `rows` | The estimated rows, an `f64` from zero to the number of rows the transaction sees. |
| `stale` | Whether any statistic it used is stale. |
| `joint` | Whether one statistic covers all its columns: always for a condition on one column; for several columns, when an index prefix covers them all. |

The `Condition`:

| Variant | Asks for |
|---|---|
| `Condition::Equal { column, value }` | The rows whose column equals `value`. `Value::Null` asks for the rows where it is null. |
| `Condition::Range { column, lower, upper }` | The rows whose column is within the `Bound`s. With any bound, null rows are out; with none, in. |
| `Condition::EqualColumns(vec![(column, value), ...])` | The rows where every listed column equals its value (nulls as in `Equal`). The columns are distinct. |

The estimated rows are the fraction the statistics give times the rows the transaction sees, so a write that
estimates counts its own rows, although the statistics do not see its changes. Values follow the rules of an index
scan's: normalized and rounded as a row's (`-0.0` estimates `+0.0`), bounds in the natural order of the type. Unlike a
scan, a value has no size limit, and a null equality in a column without nulls estimates zero instead of failing.

After computing, these rules always hold exactly:

- A table with no rows (as the transaction sees it) estimates zero, without sampling.
- An empty or inverted interval estimates zero.
- An interval without bounds estimates all the rows.
- Equal to null in a column that does not allow nulls estimates zero.
- In a column that is by itself the primary key or the key of a unique index, equal to a non-null value estimates at
  most one row. The same for `EqualColumns` that gives non-null values to every primary key column, or to every key
  column of a unique index.

Large values: an equality with a large value estimates their whole fraction, an upper bound; an equality with a short
value does not count them; and an interval with any bound always adds their fraction, since their place in the order is
not known.

Estimating is not a change of a write and ends no references. It reads pages of the rows only when it samples.

```rust
use std::ops::Bound;

use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Condition, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("age", ColumnType::I32).not_null(),
            Column::new("city", ColumnType::String),
        ],
        &["id"],
    );
    let table = write.open_table_with("people", TableMode::CreateNew(&schema))?;
    for id in 0..1_000i64 {
        // One city in ten is unknown.
        let city = if id % 10 == 9 { Value::Null } else { Value::String("Oslo".into()) };
        write.insert_row(&table, &[("id", Value::I64(id)), ("age", Value::I32((id % 80) as i32)), ("city", city)])?;
    }
    write.commit()?;

    let read = store.read()?;
    let table = read.open_table("people")?;
    let estimate = |condition: Condition<'_>| read.estimate(&table, &condition).map(|estimate| estimate.rows);

    // Exact rules.
    assert_eq!(estimate(Condition::Equal { column: "id", value: Value::I64(5) })?, 1.0);
    assert_eq!(estimate(Condition::Equal { column: "age", value: Value::Null })?, 0.0);
    let everything = Condition::Range { column: "age", lower: Bound::Unbounded, upper: Bound::Unbounded };
    assert_eq!(estimate(everything)?, 1_000.0);
    let inverted = Condition::Range {
        column: "age",
        lower: Bound::Included(Value::I32(50)),
        upper: Bound::Excluded(Value::I32(20)),
    };
    assert_eq!(estimate(inverted)?, 0.0);

    // Approximate answers: about a tenth of the cities are null, about an eighth of the ages are in [10, 20).
    let unknown = estimate(Condition::Equal { column: "city", value: Value::Null })?;
    assert!(unknown > 0.0 && unknown < 1_000.0);
    let teens = Condition::Range {
        column: "age",
        lower: Bound::Included(Value::I32(10)),
        upper: Bound::Excluded(Value::I32(20)),
    };
    let teens = estimate(teens)?;
    assert!(teens > 0.0 && teens < 1_000.0);
    Ok(())
}
```

### Equality on several columns

`Condition::EqualColumns` combines statistics, biased to overestimate:

1. **Groups.** While some listed columns are uncovered, it takes the index prefix with the most columns whose columns
   are all in the condition and none already covered (with a tie, the index first by name). Each column no prefix
   covers is a group of its own.
2. **Fraction of each group,** from its statistics, as an equality.
3. **Combination.** With the fractions sorted from smallest to largest, f₁ ≤ f₂ ≤ f₃ ≤ f₄, it takes
   f₁ × f₂^(1/4) × f₃^(1/16) × f₄^(1/64), ignoring groups past the fourth. This lies between assuming the columns
   independent (the product) and assuming them fully correlated (the smallest fraction), close to the latter. With a
   single group the estimate is `joint`.
4. It multiplies by the rows the transaction sees, then applies the exact rules above.

So an index on the columns you filter together gives a joint, more precise estimate.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Condition, IndexDefinition, IndexKey, IndexMode, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("city", ColumnType::String),
            Column::new("age", ColumnType::I32),
        ],
        &["id"],
    );
    let table = write.open_table_with("people", TableMode::CreateNew(&schema))?;
    let cities = ["Oslo", "Lima", "Rome", "Oslo"];
    for id in 0..1_000i64 {
        let city = Value::String(cities[(id % 4) as usize].into());
        write.insert_row(&table, &[("id", Value::I64(id)), ("city", city), ("age", Value::I32((id % 80) as i32))])?;
    }
    let by_city = IndexDefinition::new("by_city_age", vec![IndexKey::new("city"), IndexKey::new("age")]);
    write.create_index(&table, &by_city, IndexMode::CreateNew)?;
    write.commit()?;

    let read = store.read()?;
    let table = read.open_table("people")?;
    let oslo = Value::String("Oslo".into());

    // The index prefix (city, age) covers both columns: one group, a joint estimate.
    let both = Condition::EqualColumns(vec![("city", oslo.clone()), ("age", Value::I32(10))]);
    assert!(read.estimate(&table, &both)?.joint);

    // age and id: no prefix covers them, so two groups are combined; id is the primary key, so at most one row.
    let combined = read.estimate(&table, &Condition::EqualColumns(vec![("age", Value::I32(10)), ("id", Value::I64(10))]))?;
    assert!(!combined.joint);
    assert!(combined.rows <= 1.0);

    // The statistics of the index prefixes.
    let prefixes = read.index_statistics(&table, "by_city_age")?.unwrap();
    assert_eq!(prefixes.len(), 1);
    assert_eq!(prefixes[0].columns, 2);
    assert_eq!(prefixes[0].rows_read, 1_000);
    Ok(())
}
```

## Read the statistics

| Operation | Returns |
|---|---|
| `column_statistics(&table, column)` | `Option<ColumnStatistics>`: `None` when the transaction sees no statistics of the column. |
| `index_statistics(&table, index)` | `Option<Vec<PrefixStatistics>>`, one per prefix from 2 key columns up to all, in that order: `None` when it sees none or the index has a single key column. |

Both exist on `ReadTransaction` and `WriteTransaction`. Reading the statistics never samples, never asks for work,
reads no page of the rows or the indexes, is not a change and ends no references.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("color", ColumnType::String)],
        &["id"],
    );
    let table = write.open_table_with("items", TableMode::CreateNew(&schema))?;
    for id in 0..100i64 {
        let color = match id % 10 {
            0..=5 => Value::String("red".into()),
            6..=8 => Value::String("blue".into()),
            _ => Value::Null,
        };
        write.insert_row(&table, &[("id", Value::I64(id)), ("color", color)])?;
    }
    write.analyze(&table, Some("color"))?;
    write.commit()?;

    let read = store.read()?;
    let table = read.open_table("items")?;
    let stats = read.column_statistics(&table, "color")?.unwrap();
    assert_eq!((stats.origin_rows, stats.rows_read), (100, 100));
    assert!(!stats.stale);
    assert_eq!(stats.large_fraction, 0.0);
    // The most frequent value comes first.
    assert_eq!(stats.frequent[0].0, Value::String("red".into()));
    // Nulls, large values, frequent values and buckets cover every row, up to rounding.
    let total = stats.null_fraction
        + stats.large_fraction
        + stats.frequent.iter().map(|(_, fraction)| fraction).sum::<f64>()
        + stats.histogram.iter().map(|bucket| bucket.fraction).sum::<f64>();
    assert!((total - 1.0).abs() < 1e-9);
    // Only the column analyzed has statistics.
    assert!(read.column_statistics(&table, "id")?.is_none());
    Ok(())
}
```

## Analyze

| Operation | Builds from | Stores |
|---|---|---|
| `Store::analyze(table, column)` | A new snapshot, outside any transaction | With a short write of its own; waits for the write turn; returns when the statistics are stored or discarded as older than those there |
| `Store::analyze_with_timeout(table, column, limit)` | The same | Waits for the turn at most `limit`, counted from the end of the sampling, then fails with a timeout |
| `WriteTransaction::analyze(&table, column)` | What the write sees, its changes included | Inside the write, without asking for another turn; an abort leaves the statistics as they were |

`column` is `None` for every column of the table and the prefixes of its indexes, or `Some(name)` for that column
only. Analyze returns nothing. Analyzing a table without rows stores statistics of zero rows. There is no analyze in a
read transaction.

`Store::analyze` called on a thread that has a write open on that store fails at once with `ErrorKind::Misuse`. If it
fails after sampling (waiting for the turn or storing), what it built stays in the store's memory, and the next estimate
that uses it asks for it to be stored. `WriteTransaction::analyze` is a change of the write when it stores, and it ends
the references read before, even when it fails.

## Statistics and table changes

| Change | Its statistics |
|---|---|
| Rename a column, allow nulls in it | Kept. |
| Add a column | It has none until they are built. |
| Drop a column | Its statistics are removed in that write. |
| Empty the table | All removed; the counter adds the rows removed. |
| Rename the table | Kept, with its counter. |
| Delete the table | Deleted with it. |
| Create an index | Column statistics unchanged; the new index has no prefix statistics until they are built. |
| Drop an index | Its prefix statistics are removed in that write. |

## What the estimates promise

- **Approximate.** Statistics of a sampled table describe the sample, and even those of a table read whole summarize
  its values. Only the exact rules above are guaranteed; assert nothing else about `rows`.
- **Possibly stale.** They describe the source snapshot, not the rows you see now. A stale estimate says so in
  `stale` and triggers a refresh in the background; the estimate does not wait for it.
- **Never a guess.** Without statistics, an estimate samples and waits for the sample instead of answering with a
  default.
- **Biased to overestimate** when it combines several columns, since underestimating a result is usually worse than
  overestimating it.

## Limits and errors

| Situation | Error |
|---|---|
| `Store::analyze` on a thread with a write open on the store | `ErrorKind::Misuse` |
| A table, column or index name that does not have 1 to 1,024 bytes | `Error::InvalidSize` |
| A handle of a table the write deleted | `Error::TreeNotFound` |
| `Store::analyze` of a table that does not exist, or of a name that is another kind of tree | `Error::TreeNotFound`, `Error::DifferentTreeType` |
| A column the table does not have | `Error::DifferentSchema` (`SchemaDifference::MissingColumn`) |
| `index_statistics` of an index the table does not have | `Error::DifferentIndex` with `SchemaDifference::MissingIndex` (class `ErrorKind::DifferentSchema`) |
| A vector column; a JSON column | `Error::InvalidSchema` with `SchemaRule::VectorColumn`; `SchemaRule::JsonColumn` |
| A value of another type or out of its domain | `Error::InvalidValue` (`ValueRule::WrongType`, `ValueRule::OutOfDomain`) |
| A null as a bound; an empty `EqualColumns` | `Error::InvalidValue`, `ValueRule::InvalidRange` |
| A column repeated in `EqualColumns` | `Error::InvalidValue`, `ValueRule::RepeatedColumn` |
| A failure reading the sample | Damage, input/output or out of memory (in a write, the write can no longer commit) |
| `analyze_with_timeout` past its limit | `ErrorKind::Timeout` |

An estimate without statistics, with stale statistics, on an empty table or on an empty interval is not an error.

```rust
use std::ops::Bound;

use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Condition, Error, ErrorKind, Schema, SchemaRule, Store, StoreOptions, TableMode, ValueRule};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I64),
            Column::new("doc", ColumnType::Json),
        ],
        &["id"],
    );
    let table = write.open_table_with("t", TableMode::CreateNew(&schema))?;

    let missing = Condition::Equal { column: "nope", value: Value::I64(1) };
    assert_eq!(write.estimate(&table, &missing).unwrap_err().kind(), ErrorKind::DifferentSchema);
    match write.column_statistics(&table, "doc") {
        Err(Error::InvalidSchema { rule, .. }) => assert_eq!(rule, SchemaRule::JsonColumn),
        other => panic!("unexpected {other:?}"),
    }
    let rule = |condition: Condition<'_>| match write.estimate(&table, &condition) {
        Err(Error::InvalidValue { rule, .. }) => rule,
        other => panic!("unexpected {other:?}"),
    };
    assert_eq!(rule(Condition::Equal { column: "n", value: Value::String("one".into()) }), ValueRule::WrongType);
    let null_bound = Condition::Range { column: "n", lower: Bound::Included(Value::Null), upper: Bound::Unbounded };
    assert_eq!(rule(null_bound), ValueRule::InvalidRange);
    assert_eq!(rule(Condition::EqualColumns(vec![])), ValueRule::InvalidRange);
    let repeated = Condition::EqualColumns(vec![("n", Value::I64(1)), ("n", Value::I64(2))]);
    assert_eq!(rule(repeated), ValueRule::RepeatedColumn);

    // Analyze on the store while this thread holds a write.
    assert_eq!(store.analyze("t", None).unwrap_err().kind(), ErrorKind::Misuse);
    write.commit()?;
    Ok(())
}
```

See also: [tables](tables.md) · [indexes](indexes.md) · [rows](rows.md) · [data types](data-types.md) ·
[JSON](json.md) · [transactions](transactions.md) · [errors](errors.md)
