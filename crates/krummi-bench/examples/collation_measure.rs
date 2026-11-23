//! The informative measurement of the collation of text columns (IC12 of `docs/prd/krummi/tables/tables-collation.prd.md`):
//! on a table of 100,000 rows whose key is a `string` of about 24 bytes of text with capitals and accents, generated
//! from a fixed seed, with a unique index on another such `string` column, it times inserting the rows in writes of
//! 1,000 (as W23), reading each row by its key, and looking each row up by its index. Each measure runs with both
//! columns `ci` and with both `binary`, its floor. It is not a load of the benchmark and has no threshold.
//!
//! Usage: `cargo run --release -p krummi-bench --example collation_measure -- <work folder> [rounds] [scale]`, where
//! the scale divides the row count (1 for the measurement, larger for a quick check). Prints one line per round and
//! measure, and the medians at the end. It deletes the subfolders `binary` and `ci` of the work folder before each
//! insert and after each round, whatever they hold: give it a folder of its own.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::time::Instant;

use krummi::table::value::{ColumnType, Value};
use krummi::{
    Collation, Column, IndexDefinition, IndexKey, IndexMode, Schema, Store, StoreOptions, TableMode,
};

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

const NAMES: [&str; 8] = [
    "Ñandú", "José", "Ángela", "Óscar", "Íñigo", "Úrsula", "Élodie", "Zoë",
];
const SURNAMES: [&str; 8] = [
    "Pérez", "Muñoz", "Gómez", "Ibáñez", "Núñez", "Güell", "López", "Díaz",
];

/// A text of about 24 bytes, distinct for each `n`.
fn text(n: u64, rng: &mut Rng) -> String {
    let name = NAMES[rng.below(8) as usize];
    let surname = SURNAMES[rng.below(8) as usize];
    format!("{name} {surname} {n:07}")
}

fn shuffled(rows: usize) -> Vec<usize> {
    let mut keys: Vec<usize> = (0..rows).collect();
    let mut rng = Rng(0x5EED);
    for i in (1..keys.len()).rev() {
        keys.swap(i, rng.below(i as u64 + 1) as usize);
    }
    keys
}

fn schema(collation: Collation) -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::String)
                .not_null()
                .with_collation(collation),
            Column::new("name", ColumnType::String).with_collation(collation),
        ],
        &["id"],
    )
}

type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn s(text: &str) -> Value<'_> {
    Value::String(Cow::Borrowed(text))
}

fn insert(folder: &Path, collation: Collation, ids: &[String], names: &[String]) -> Res<f64> {
    if folder.exists() {
        std::fs::remove_dir_all(folder)?;
    }
    let store = Store::open(folder, &StoreOptions::default())?;
    let mut write = store.write()?;
    let table = write.open_table_with("t", TableMode::OpenOrCreate(&schema(collation)))?;
    let index = IndexDefinition::new("ix", vec![IndexKey::new("name")]).unique();
    write.create_index(&table, &index, IndexMode::CreateNew)?;
    write.commit()?;
    let start = Instant::now();
    for (chunk_ids, chunk_names) in ids.chunks(1_000).zip(names.chunks(1_000)) {
        let mut write = store.write()?;
        let table = write.open_table("t")?;
        for (id, name) in chunk_ids.iter().zip(chunk_names) {
            write.insert_row(&table, &[("id", s(id)), ("name", s(name))])?;
        }
        write.commit()?;
    }
    let ns = start.elapsed().as_nanos() as f64 / ids.len() as f64;
    store.close()?;
    Ok(ns)
}

#[derive(Clone, Copy)]
enum Read {
    Key,
    Index,
}

fn read(folder: &Path, ids: &[String], names: &[String], what: Read) -> Res<f64> {
    let store = Store::open(folder, &StoreOptions::default())?;
    let order = shuffled(ids.len());
    let read = store.read()?;
    let table = read.open_table("t")?;
    let mut check = 0u64;
    let start = Instant::now();
    for &i in &order {
        check += match what {
            Read::Key => {
                let row = read.get_row(&table, &[s(&ids[i])])?.ok_or("missing row")?;
                row.text(1).ok_or("null")?.len() as u64
            }
            Read::Index => {
                let mut rows = read.index_rows(&table, "ix", &[s(&names[i])], ..)?;
                let row = rows.next().ok_or("missing row")??;
                row.text(0).ok_or("null")?.len() as u64
            }
        };
    }
    let ns = start.elapsed().as_nanos() as f64 / ids.len() as f64;
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
            .ok_or("usage: collation_measure <work folder> [rounds] [scale]")?,
    );
    let rounds: usize = args.next().map_or(Ok(10), |r| r.parse())?;
    let scale: usize = args.next().map_or(Ok(1), |s| s.parse())?;
    std::fs::create_dir_all(&work)?;
    let rows = (100_000 / scale.max(1)).max(1);
    let mut rng = Rng(0x4B52_554D_4D49_1C00);
    let ids: Vec<String> = (0..rows as u64).map(|n| text(n, &mut rng)).collect();
    let names: Vec<String> = (0..rows as u64).map(|n| text(n, &mut rng)).collect();
    println!("rows\tround\tmeasure\tns_per_op");
    let mut measures: Vec<(String, Vec<f64>)> = Vec::new();
    let mut record = |name: String, ns: f64, round: usize| {
        println!("{rows}\t{round}\t{name}\t{ns:.0}");
        match measures.iter_mut().find(|(n, _)| *n == name) {
            Some((_, values)) => values.push(ns),
            None => measures.push((name, vec![ns])),
        }
    };
    for round in 1..=rounds {
        for collation in [Collation::Binary, Collation::Ci] {
            let folder = work.join(collation.to_string());
            record(
                format!("insert {collation}"),
                insert(&folder, collation, &ids, &names)?,
                round,
            );
            record(
                format!("by key {collation}"),
                read(&folder, &ids, &names, Read::Key)?,
                round,
            );
            record(
                format!("by index {collation}"),
                read(&folder, &ids, &names, Read::Index)?,
                round,
            );
            std::fs::remove_dir_all(&folder)?;
        }
    }
    println!("measure\tmedian_ns_per_op\tratio_to_binary");
    let binary = |name: &str| {
        let floor = name.replace(" ci", " binary");
        let mut values = measures
            .iter()
            .find(|(n, _)| *n == floor)
            .map(|(_, v)| v.clone());
        values.as_mut().map(|v| median(v))
    };
    for (name, values) in &mut measures.clone() {
        let m = median(values);
        let ratio = binary(name).map_or(1.0, |floor| m / floor);
        println!("{name}\t{m:.0}\t{ratio:.3}");
    }
    Ok(())
}
