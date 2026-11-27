# Vector search and vector indexes

A vector column holds an embedding per row. Krummi finds the **k nearest neighbours** of a query vector among the rows
of a table, combined with ordinary conditions on its other columns: the conditions are evaluated first, then the
distance decides among the rows that meet them. There are two searches:

- the **exact search**, which computes the distance of every row that passes and needs no index;
- the search **through a vector index**, an approximate IVF-SQ8 index that a background thread of the store trains,
  builds and keeps up to date.

Use the exact search for small tables or very selective filters, and a vector index when many rows pass and a search
must stay in milliseconds. Both return the same kind of result: up to k rows, nearest first, each with the values of its
primary key and its exact distance.

## Quick path

1. Give the table a vector column (see [data types](data-types.md)): `f32`, `f16` or `i8` elements for an index, `bit`
   only for the exact search; declare the cosine metric on the column if you compare by cosine.
2. Search exactly with `nearest_exact(&table, column, metric, &query)`, in a read or a write.
3. For large tables, create a vector index with `create_vector_index`; the store's worker trains it once the table has
   10,000 rows with a vector, builds it in the background, and keeps it up to date after each write.
4. Search through the index with `nearest(&table, index, &query)`. Until the index is ready, that search is exact.
5. Read the index's state and health with `vector_index_info`; ask for a rebuild with `rebuild_vector_index`, or let the
   worker rebuild it by itself when the data drifts.

```rust
use krummi::table::value::{ColumnType, Value, VectorElement, VectorType};
use krummi::{
    Column, FilterCondition, IndexMode, Nearest, Schema, Store, StoreOptions, TableMode, VectorIndexDefinition,
    VectorMetric,
};

fn vector(x: f32, y: f32) -> Value<'static> {
    Value::Vector([x.to_le_bytes(), y.to_le_bytes()].concat().into())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let embedding = VectorType { element: VectorElement::F32, dimension: 2, cosine: false };
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("lang", ColumnType::String),
            Column::new("v", ColumnType::Vector(embedding)),
        ],
        &["id"],
    );
    let table = write.open_table_with("docs", TableMode::CreateNew(&schema))?;
    for (id, lang, x) in [(1, "en", 0.0), (2, "es", 1.0), (3, "en", 3.0), (4, "en", 5.0)] {
        let row = [("id", Value::I64(id)), ("lang", Value::String(lang.into())), ("v", vector(x, 0.0))];
        write.insert_row(&table, &row)?;
    }
    let definition = VectorIndexDefinition::new("by_v", "v", VectorMetric::L2);
    write.create_vector_index(&table, &definition, IndexMode::CreateNew)?;
    write.commit()?;

    let read = store.read()?;
    let table = read.open_table("docs")?;
    let english = vec![FilterCondition::Equal { column: "lang", value: Value::String("en".into()) }];
    let query = Nearest::new(vector(2.5, 0.0), 2).with_filter(english);
    let exact = read.nearest_exact(&table, "v", VectorMetric::L2, &query)?;
    // Through the index: with 4 rows it is still training, so the search is exact too.
    let found = read.nearest(&table, "by_v", &query)?;
    assert_eq!(found, exact);
    assert!(found.exact);
    let keys: Vec<Value<'static>> = found.rows.iter().map(|n| n.key[0].clone()).collect();
    assert_eq!(keys, [Value::I64(3), Value::I64(1)]);
    assert_eq!(found.rows[0].distance, 0.5);
    Ok(())
}
```

## Distances

| Metric | Columns | Distance returned | Smaller is |
|---|---|---|---|
| `VectorMetric::L2` | `f32`, `f16`, `i8` without cosine | the Euclidean distance | nearer |
| `VectorMetric::Dot` | `f32`, `f16`, `i8` without cosine | the dot product, negated | nearer |
| `VectorMetric::Cosine` | `f32`, `f16`, `i8` declaring cosine | one minus the cosine | nearer |
| `VectorMetric::Hamming` | `bit`, exact search only | the number of differing bits | nearer |

Distances are `f64`. Every element is converted to `f64` exactly and added in a fixed order, so the same data gives
the same rows and the same distance bits on every machine. Rows at the same distance come in the order of their
primary keys; L2 compares the squares, so two rows whose Euclidean distances round to the same value keep the order of
their squares.

## The query and its filter

A `Nearest` is the query vector (a value of the column's type), k (1 to 10,000), the filter and, for a search through
an index, how many lists to probe first. The filter is a list of `FilterCondition`s joined with AND:

| Condition | Rows that pass |
|---|---|
| `Equal { column, value }` | the column equals the value |
| `Range { column, lower, upper }` | the column is within the bounds |
| `EqualColumns(pairs)` | each column equals its value |
| `In { column, values }` | the column is one of the values |
| `NotIn { column, values }` | the column is none of the values |

Values compare as in an index's key: normalized and with the column's collation, so `"RED"` finds `"Red"` in a `ci`
column, and a NaN in a list finds the rows with a NaN. Nulls follow SQL: a row whose column is null never passes a
condition on it; an equality with a null, a range with a null bound and a NOT IN whose list holds a null let no row
pass; a null in an IN list passes nothing by itself. A row whose vector is null is never a result.

When a secondary index leads with a column that an equality, an IN list or a range constrains, the search reads only
the rows of those entries; otherwise it reads the table. Conditions on vector or JSON columns are refused.

## Vector indexes

```rust
use krummi::table::value::{ColumnType, Value, VectorElement, VectorType};
use krummi::{
    Column, IndexMode, Nearest, Schema, Store, StoreOptions, TableMode, VectorIndexDefinition, VectorIndexState,
    VectorMetric,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let ty = VectorType { element: VectorElement::F32, dimension: 4, cosine: false };
    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("v", ColumnType::Vector(ty))],
        &["id"],
    );
    let table = write.open_table_with("points", TableMode::CreateNew(&schema))?;
    let point = |id: i64| {
        let values = [(id % 7) as f32, (id % 11) as f32, (id % 13) as f32, (id % 3) as f32];
        Value::Vector(values.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>().into())
    };
    for id in 0..10_000 {
        write.insert_row(&table, &[("id", Value::I64(id)), ("v", point(id))])?;
    }
    let definition = VectorIndexDefinition::new("near", "v", VectorMetric::L2);
    write.create_vector_index(&table, &definition, IndexMode::CreateNew)?;
    write.commit()?;

    // The worker trains and builds the index in the background; a search meanwhile is exact.
    loop {
        let read = store.read()?;
        let table = read.open_table("points")?;
        let info = read.vector_index_info(&table, "near")?;
        if info.state == VectorIndexState::Ready && info.pending == 0 {
            assert_eq!(info.lists, Some(10));
            let found = read.nearest(&table, "near", &Nearest::new(point(42), 5).with_probes(10))?;
            assert_eq!(found.rows.len(), 5);
            assert_eq!(found.rows[0].distance, 0.0);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    Ok(())
}
```

- **Definition.** A name in the same name space as the table's secondary indexes, a vector column and a metric it
  admits. A table has at most 64 indexes of both kinds; two vector indexes may share a column with different metrics.
  `drop_index` drops either kind; Describe lists the vector indexes apart, in `TableSchema::vector_indexes`.
- **Lists.** The index divides the vectors into lists by their nearest centroid and keeps an 8-bit code per component
  of each row. It has the rows with a vector divided by 1,000 lists, rounded down, up to 1 M rows, and the square root
  of the rows above.
- **States.** *Training* (from its creation, or from an emptying of the table, until the table has 10,000 rows with a
  vector), *building*, *ready*, and *ready and rebuilding* while a new generation is trained and built.
- **Writes.** A write that inserts, deletes or changes the vector of a row only marks it as pending, in the same
  transaction; the worker reconciles the pending rows in short batches of its own writes, each holding the write turn at
  most 15 ms. A search evaluates the pending rows exactly, so the worker's lag never hides a row.
- **Schema changes.** Renaming the column or the table keeps the index; dropping its column fails; emptying the table
  sends the index back to training; deleting the table or the index frees its pages at once.

## How a search through an index works

1. It evaluates the filter and counts the rows that pass (those that meet it and have a vector).
2. With at most 20,000 of them, it computes their exact distances: the result is exact (`Neighbours::exact`).
3. With more, it probes the lists nearest to the query (4 % of them unless the query says `with_probes`), considering
   only the rows that passed; while fewer than k were found it goes on to the next lists. It reranks the best candidates
   by their exact distances, adds the pending rows, and says approximate.

A search sees its transaction's snapshot: what the worker does after it began never changes its result. The result
misses a row only through the lists it does not probe and the 8-bit codes, never through the index's lag. To measure
the recall of an index on your data, compare its results with the exact search.

## Information and rebuilds

`vector_index_info` gives the state, the lists of the generation the searches use, the pending rows, and three
indicators of that generation: the fraction of codes with a component out of the quantizer's range, the largest list
against the mean, and the rows it reflects against those it was trained on (none when they cannot be computed). The
worker rebuilds an index by itself when an indicator passes its threshold and grew since the generation was published;
`rebuild_vector_index` asks for one in a write. A rebuild trains and builds a new generation while the searches use the
current one, then publishes it at once. Asking with a rebuild in progress, with the index not ready, or with fewer than
10,000 rows with a vector does nothing.

## The worker

The worker is one thread of the store. It starts when a transaction first opens a table with a vector index, and when
a write leaves work to it; it stops when the store closes, abandoning a training in progress. Its work is in pages: the
pending rows, the state and the progress of a build survive a close or a crash, and it goes on when the store opens
again. A batch that fails (damage, input and output, memory, a full disk) changes nothing and is retried later; the
searches stay right because its rows stay pending. A failure that makes the store refuse writes stops it.

## Errors

| Situation | Kind |
|---|---|
| A name of an index or a column without 1 to 1,024 bytes | `InvalidSize` |
| A definition over a missing column, not a vector, of `bit`, with a metric the column does not admit, equal to another vector index, or past 64 indexes | `InvalidSchema` |
| Create new with a name the table has; create if not exists with another definition or kind | `TreeAlreadyExists`; `DifferentSchema` |
| Searching, reading the information of or rebuilding a vector index the table does not have (a secondary index's name too); a filter on a column the table does not have | `DifferentSchema` |
| A filter on a vector or JSON column | `InvalidSchema` |
| No query vector, one of another size, with NaN or infinities, a zero vector with the cosine; k outside 1 to 10,000; zero probes; an exact search with a metric the column does not admit; a filter value of another type, an empty IN or NOT IN list, a column repeated in `EqualColumns` | `InvalidValue` |
| A handle of a table the write deleted | `TreeNotFound` |
