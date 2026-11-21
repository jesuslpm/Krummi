//! The informative measurement of JSON columns (JS13 of `docs/prd/krummi/tables/tables-json.prd.md`): on a table
//! with an integer key and one JSON column, and with documents generated from a fixed seed of about 200 bytes
//! (100,000 rows), 4 KB (100,000 rows) and 64 KB (10,000 rows), each with properties before and after a large array,
//! it times inserting them in writes of 1,000 rows (as W23), reading the whole document by key, and reading the
//! first and the last root property by key. Each measure is compared with the same table holding the same normalized
//! text in a `string` column, read whole, its floor. It is not a load of the benchmark and has no threshold.
//!
//! Usage: `cargo run --release -p krummi-bench --example json_measure -- <work folder> [rounds] [scale]`, where the
//! scale divides the row counts (1 for the measurement, larger for a quick check). Prints one line per round and
//! measure, and the medians at the end.

use std::path::{Path, PathBuf};
use std::time::Instant;

use krummi::table::json::Json;
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Schema, Store, StoreOptions, TableMode};

/// The column type of the document: the measured one or its floor.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Json,
    String,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Json => "json",
            Kind::String => "string",
        }
    }
}

/// SplitMix64, the generator of the table tests.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A document of about `target` bytes in normalized text: an order whose root properties are `id` (first), a
/// customer, a large `lines` array, a total and `note` (last).
fn document(id: u64, target: usize, rng: &mut Rng) -> String {
    let mut text = format!(
        r#"{{"id":{id},"status":"open","created":"2026-11-21T10:23:00Z","customer":{{"name":"Customer {}","city":"Las Palmas"}},"lines":["#,
        rng.below(100_000)
    );
    let tail = format!(
        r#"],"total":{}.{:02},"note":"last""#,
        rng.below(10_000),
        rng.below(100)
    );
    let mut first = true;
    while text.len() + tail.len() + 1 < target {
        if !first {
            text.push(',');
        }
        first = false;
        text.push_str(&format!(
            r#"{{"sku":"SKU-{:06}","qty":{},"price":{}.{:02}}}"#,
            rng.below(1_000_000),
            1 + rng.below(20),
            rng.below(500),
            rng.below(100)
        ));
    }
    text.push_str(&tail);
    text.push('}');
    text
}

/// The documents of one size, from a seed of their own.
fn documents(rows: usize, target: usize) -> Vec<String> {
    let mut rng = Rng(0x4B52_554D_4D49_0000 ^ target as u64);
    (0..rows as u64)
        .map(|id| document(id, target, &mut rng))
        .collect()
}

/// The keys in a fixed random order.
fn shuffled(rows: usize) -> Vec<u64> {
    let mut keys: Vec<u64> = (0..rows as u64).collect();
    let mut rng = Rng(0x5EED);
    for i in (1..keys.len()).rev() {
        keys.swap(i, rng.below(i as u64 + 1) as usize);
    }
    keys
}

fn schema(kind: Kind) -> Schema {
    let ty = match kind {
        Kind::Json => ColumnType::Json,
        Kind::String => ColumnType::String,
    };
    Schema::new(
        vec![
            Column::new("id", ColumnType::U64).not_null(),
            Column::new("doc", ty),
        ],
        &["id"],
    )
}

type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// Inserts the documents in writes of 1,000 rows into a new store at `folder`; returns nanoseconds per row.
fn insert(folder: &Path, kind: Kind, docs: &[String]) -> Res<f64> {
    if folder.exists() {
        std::fs::remove_dir_all(folder)?;
    }
    let store = Store::open(folder, &StoreOptions::default())?;
    let schema = schema(kind);
    let mut write = store.write()?;
    write.open_table_with("t", TableMode::OpenOrCreate(&schema))?;
    write.commit()?;
    let start = Instant::now();
    for (batch, chunk) in docs.chunks(1_000).enumerate() {
        let mut write = store.write()?;
        let table = write.open_table("t")?;
        for (j, doc) in chunk.iter().enumerate() {
            let id = Value::U64((batch * 1_000 + j) as u64);
            let doc = match kind {
                Kind::Json => Value::Json(Json::new(doc.as_str())),
                Kind::String => Value::String(doc.as_str().into()),
            };
            write.insert_row(&table, &[("id", id), ("doc", doc)])?;
        }
        write.commit()?;
    }
    let ns = start.elapsed().as_nanos() as f64 / docs.len() as f64;
    store.close()?;
    Ok(ns)
}

/// What a read by key does with the document.
#[derive(Clone, Copy)]
enum Read {
    Whole,
    First,
    Last,
}

/// Reads every row of the store at `folder` by key, in a fixed random order; returns nanoseconds per read.
fn read(folder: &Path, kind: Kind, rows: usize, what: Read) -> Res<f64> {
    let store = Store::open(folder, &StoreOptions::default())?;
    let keys = shuffled(rows);
    let read = store.read()?;
    let table = read.open_table("t")?;
    let mut check = 0u64;
    let start = Instant::now();
    for &key in &keys {
        let row = read
            .get_row(&table, &[Value::U64(key)])?
            .ok_or("missing row")?;
        check += match kind {
            Kind::String => row.text(1).ok_or("null")?.len() as u64,
            Kind::Json => {
                let json = row.json(1).ok_or("null")?;
                let doc = json.document().ok_or("not read back")?;
                match what {
                    Read::Whole => doc.text().len() as u64,
                    Read::First => doc.get("id").and_then(|v| v.as_u64()).ok_or("no id")?,
                    Read::Last => doc
                        .get("note")
                        .and_then(|v| v.as_text())
                        .ok_or("no note")?
                        .len() as u64,
                }
            }
        };
    }
    let ns = start.elapsed().as_nanos() as f64 / rows as f64;
    std::hint::black_box(check);
    drop(table);
    drop(read);
    store.close()?;
    Ok(ns)
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    }
}

fn main() -> Res<()> {
    let mut args = std::env::args().skip(1);
    let work = PathBuf::from(
        args.next()
            .ok_or("usage: json_measure <work folder> [rounds] [scale]")?,
    );
    let rounds: usize = args.next().map_or(Ok(10), |r| r.parse())?;
    let scale: usize = args.next().map_or(Ok(1), |s| s.parse())?;
    std::fs::create_dir_all(&work)?;
    println!("size\trows\tround\tmeasure\tns_per_op");
    let mut summary = Vec::new();
    for (label, target, rows) in [
        ("200B", 200, 100_000),
        ("4KB", 4_096, 100_000),
        ("64KB", 65_536, 10_000),
    ] {
        let rows = (rows / scale).max(1);
        let docs = documents(rows, target);
        let mut measures: Vec<(String, Vec<f64>)> = Vec::new();
        let mut record = |name: String, ns: f64, round: usize| {
            println!("{label}\t{rows}\t{round}\t{name}\t{ns:.0}");
            match measures.iter_mut().find(|(n, _)| *n == name) {
                Some((_, values)) => values.push(ns),
                None => measures.push((name, vec![ns])),
            }
        };
        for round in 1..=rounds {
            for kind in [Kind::String, Kind::Json] {
                let folder = work.join(format!("{label}-{}", kind.name()));
                record(
                    format!("insert {}", kind.name()),
                    insert(&folder, kind, &docs)?,
                    round,
                );
                record(
                    format!("read whole {}", kind.name()),
                    read(&folder, kind, rows, Read::Whole)?,
                    round,
                );
                if kind == Kind::Json {
                    record(
                        "read first json".to_owned(),
                        read(&folder, kind, rows, Read::First)?,
                        round,
                    );
                    record(
                        "read last json".to_owned(),
                        read(&folder, kind, rows, Read::Last)?,
                        round,
                    );
                }
            }
        }
        for (name, values) in &mut measures {
            summary.push((label, rows, name.clone(), median(values)));
        }
    }
    println!("\nmedians\nsize\trows\tmeasure\tns_per_op\tratio_to_string_floor");
    for (label, rows, name, ns) in &summary {
        let floor_name = if name.starts_with("insert") {
            "insert string"
        } else {
            "read whole string"
        };
        let floor = summary
            .iter()
            .find(|(l, _, n, _)| l == label && n == floor_name)
            .map_or(f64::NAN, |s| s.3);
        println!("{label}\t{rows}\t{name}\t{ns:.0}\t{:.2}", ns / floor);
    }
    Ok(())
}
