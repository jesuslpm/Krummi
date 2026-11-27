//! The `fvecs` and `ivecs` formats of the TEXMEX corpus (SIFT1M), which the experiment also uses as the cache of
//! every other data set: each vector is its dimension as a little-endian `i32`, then its elements as little-endian
//! `f32` (`fvecs`) or `i32` (`ivecs`).

use std::fs;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::data::Vectors;

fn invalid(path: &Path, why: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{}: {why}", path.display()),
    )
}

/// Splits a file of records `dim, element × dim` of 4-byte values into its dimension and its elements.
fn records(path: &Path) -> io::Result<(usize, Vec<[u8; 4]>)> {
    let bytes = fs::read(path)?;
    let (words, rest) = bytes.as_chunks::<4>();
    if !rest.is_empty() || words.is_empty() {
        return Err(invalid(path, "length is not a whole number of records"));
    }
    let dim = i32::from_le_bytes(words[0]);
    let dim = usize::try_from(dim)
        .ok()
        .filter(|&d| d > 0)
        .ok_or_else(|| invalid(path, "dimension is not positive"))?;
    if !words.len().is_multiple_of(dim + 1) {
        return Err(invalid(path, "length is not a whole number of records"));
    }
    let mut elements = Vec::with_capacity(words.len() / (dim + 1) * dim);
    for record in words.chunks_exact(dim + 1) {
        if record[0] != words[0] {
            return Err(invalid(path, "records of different dimension"));
        }
        elements.extend_from_slice(&record[1..]);
    }
    Ok((dim, elements))
}

pub fn read_fvecs(path: &Path) -> io::Result<Vectors> {
    let (dim, elements) = records(path)?;
    let values = elements.into_iter().map(f32::from_le_bytes).collect();
    Ok(Vectors::new(dim, values))
}

/// Reads an `ivecs` file as one list of ids per record.
pub fn read_ivecs(path: &Path) -> io::Result<Vec<Vec<u32>>> {
    let (dim, elements) = records(path)?;
    let ids: Vec<u32> = elements.into_iter().map(u32::from_le_bytes).collect();
    Ok(ids.chunks_exact(dim).map(<[u32]>::to_vec).collect())
}

fn write_records<T: Copy>(
    path: &Path,
    dim: usize,
    rows: impl Iterator<Item = Vec<T>>,
    bytes: impl Fn(T) -> [u8; 4],
) -> io::Result<()> {
    let header = i32::try_from(dim)
        .map_err(|_| invalid(path, "dimension too large"))?
        .to_le_bytes();
    let temporary = path.with_extension("partial");
    let mut out = BufWriter::new(fs::File::create(&temporary)?);
    for row in rows {
        if row.len() != dim {
            return Err(invalid(path, "records of different dimension"));
        }
        out.write_all(&header)?;
        for x in row {
            out.write_all(&bytes(x))?;
        }
    }
    out.into_inner().map_err(|e| e.into_error())?.sync_all()?;
    fs::rename(temporary, path)
}

/// Writes `vectors` as `fvecs`, through a temporary file renamed at the end, so a half-written cache never exists.
pub fn write_fvecs(path: &Path, vectors: &Vectors) -> io::Result<()> {
    write_records(
        path,
        vectors.dim(),
        (0..vectors.len()).map(|i| vectors.row(i).to_vec()),
        f32::to_le_bytes,
    )
}

/// Writes lists of ids of the same length as `ivecs`.
pub fn write_ivecs(path: &Path, lists: &[Vec<u32>]) -> io::Result<()> {
    let dim = lists.first().map_or(1, Vec::len);
    write_records(path, dim, lists.iter().cloned(), u32::to_le_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fvecs_and_ivecs_round_trip() {
        let dir = std::env::temp_dir().join(format!("krummi-ann-lab-files-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let vectors = Vectors::new(3, vec![1.0, -2.5, 3.0, 0.0, f32::MIN_POSITIVE, -0.0]);
        let fvecs = dir.join("v.fvecs");
        write_fvecs(&fvecs, &vectors).unwrap();
        let back = read_fvecs(&fvecs).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back.values(), vectors.values());
        assert_eq!(back.row(1)[2].to_bits(), (-0.0f32).to_bits());
        let lists = vec![vec![3, 1], vec![0, u32::MAX]];
        let ivecs = dir.join("g.ivecs");
        write_ivecs(&ivecs, &lists).unwrap();
        assert_eq!(read_ivecs(&ivecs).unwrap(), lists);
        fs::write(&ivecs, [2, 0, 0, 0, 1, 0, 0]).unwrap();
        assert!(read_ivecs(&ivecs).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
}
