//! Converts the downloaded data sets into the `fvecs` cache the experiment reads (S3): the base vectors, the queries
//! and, when the source has one, its own ground truth.

use std::fs;
use std::io;
use std::path::Path;

use crate::data::Vectors;

/// A deterministic xorshift generator: the same seed gives the same choices on every platform.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// A value in `0..n`.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// Takes `count` vectors chosen with `seed` out of `all` as the queries; the rest, in their order, is the base.
pub fn hold_out(all: &Vectors, count: usize, seed: u64) -> (Vectors, Vectors) {
    let n = all.len();
    let count = count.min(n);
    let mut order: Vec<usize> = (0..n).collect();
    let mut rng = Rng::new(seed);
    for i in 0..count {
        let j = i + rng.below(n - i);
        order.swap(i, j);
    }
    let mut is_query = vec![false; n];
    let mut queries = Vec::with_capacity(count * all.dim());
    for &id in &order[..count] {
        is_query[id] = true;
        queries.extend_from_slice(all.row(id));
    }
    let mut base = Vec::with_capacity((n - count) * all.dim());
    for id in (0..n).filter(|&id| !is_query[id]) {
        base.extend_from_slice(all.row(id));
    }
    (
        Vectors::new(all.dim(), base),
        Vectors::new(all.dim(), queries),
    )
}

/// What a text of GloVe vectors held.
pub struct Glove {
    pub vectors: Vectors,
    /// Rows dropped because every element is zero, which a cosine column does not admit (N4 of the tables PRD).
    pub zero_rows: usize,
}

/// Parses a GloVe text: one word per line followed by `dim` numbers separated by spaces. The word is dropped.
pub fn parse_glove(text: &str, dim: usize) -> io::Result<Glove> {
    let mut values = Vec::new();
    let mut zero_rows = 0;
    for (number, line) in text.lines().enumerate() {
        let mut fields = line.split(' ');
        fields.next();
        let start = values.len();
        for field in fields.filter(|f| !f.is_empty()) {
            let x: f32 = field.parse().map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("línea {}: número no válido {field:?}", number + 1),
                )
            })?;
            if !x.is_finite() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("línea {}: valor no finito", number + 1),
                ));
            }
            values.push(x);
        }
        if values.len() - start != dim {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "línea {}: {} valores en lugar de {dim}",
                    number + 1,
                    values.len() - start
                ),
            ));
        }
        if values[start..].iter().all(|&x| x == 0.0) {
            values.truncate(start);
            zero_rows += 1;
        }
    }
    Ok(Glove {
        vectors: Vectors::new(dim, values),
        zero_rows,
    })
}

pub fn read_glove(path: &Path, dim: usize) -> io::Result<Glove> {
    parse_glove(&fs::read_to_string(path)?, dim)
}

/// The rows of a VectorDBBench parquet file (the Cohere mirror, S3): an `id` column and a list column, in the order
/// of their ids, which must be `0..rows` so that a row's id is its position.
fn parquet_lists<T>(
    path: &Path,
    element: impl Fn(&parquet::record::List, usize) -> parquet::errors::Result<T>,
) -> io::Result<Vec<Vec<T>>> {
    use parquet::file::reader::{FileReader, SerializedFileReader};
    use parquet::record::RowAccessor;
    let failed = |e: parquet::errors::ParquetError| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: {e}", path.display()),
        )
    };
    let reader = SerializedFileReader::new(fs::File::open(path)?).map_err(failed)?;
    let mut rows = Vec::new();
    for row in reader.get_row_iter(None).map_err(failed)? {
        let row = row.map_err(failed)?;
        let id = row.get_long(0).map_err(failed)?;
        let list = row.get_list(1).map_err(failed)?;
        let values = (0..list.len())
            .map(|i| element(list, i))
            .collect::<parquet::errors::Result<Vec<T>>>()
            .map_err(failed)?;
        rows.push((id, values));
    }
    rows.sort_by_key(|(id, _)| *id);
    if rows.iter().enumerate().any(|(i, (id, _))| *id != i as i64) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: los ids no son 0..filas", path.display()),
        ));
    }
    Ok(rows.into_iter().map(|(_, values)| values).collect())
}

/// Reads the vectors of a VectorDBBench parquet file, row `i` being id `i`.
pub fn read_parquet_vectors(path: &Path) -> io::Result<Vectors> {
    use parquet::record::ListAccessor;
    let lists = parquet_lists(path, |list, i| list.get_float(i))?;
    let dim = lists.first().map_or(0, Vec::len);
    if dim == 0 || lists.iter().any(|v| v.len() != dim) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: vectores de distinta dimensión", path.display()),
        ));
    }
    Ok(Vectors::new(dim, lists.into_iter().flatten().collect()))
}

/// Reads the ground truth of a VectorDBBench parquet file: the ids of the neighbours of query `i`, nearest first.
pub fn read_parquet_neighbours(path: &Path) -> io::Result<Vec<Vec<u32>>> {
    use parquet::record::ListAccessor;
    let lists = parquet_lists(path, |list, i| list.get_long(i))?;
    lists
        .into_iter()
        .map(|ids| {
            ids.into_iter()
                .map(|id| {
                    u32::try_from(id).map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "id de vecino fuera de rango")
                    })
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hold_out_splits_every_row_once_and_is_repeatable() {
        let all = Vectors::new(2, (0..200).map(|i| i as f32).collect());
        let (base, queries) = hold_out(&all, 10, 7);
        assert_eq!((base.len(), queries.len()), (90, 10));
        let mut seen: Vec<f32> = base
            .values()
            .iter()
            .chain(queries.values())
            .copied()
            .collect();
        seen.sort_by(f32::total_cmp);
        assert_eq!(seen, all.values());
        let (_, again) = hold_out(&all, 10, 7);
        assert_eq!(again.values(), queries.values());
        let firsts: Vec<f32> = (0..base.len()).map(|i| base.row(i)[0]).collect();
        assert!(firsts.is_sorted(), "the base keeps its order");
    }

    #[test]
    fn glove_drops_the_word_and_zero_rows() {
        let text = "hola 0.5 -1 2\n<unk> 0 0 0\n, 1e-3 0.0 -0.25\n";
        let glove = parse_glove(text, 3).unwrap();
        assert_eq!(glove.zero_rows, 1);
        assert_eq!(glove.vectors.values(), [0.5, -1.0, 2.0, 0.001, 0.0, -0.25]);
        assert!(parse_glove("a 1 2\n", 3).is_err());
        assert!(parse_glove("a 1 x 3\n", 3).is_err());
    }
}
