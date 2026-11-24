//! Vectors in memory, row after row, with the norm of each one, as a cosine column stores it.

use crate::kernels;

/// A set of vectors of the same dimension, identified by their position.
pub struct Vectors {
    dim: usize,
    values: Vec<f32>,
    norms: Vec<f64>,
}

impl Vectors {
    /// Takes `values`, whose length is a multiple of `dim`, and computes the norm of every row.
    pub fn new(dim: usize, values: Vec<f32>) -> Vectors {
        assert!(
            dim > 0 && values.len().is_multiple_of(dim),
            "values do not fill whole rows"
        );
        let norms = values.chunks_exact(dim).map(kernels::norm).collect();
        Vectors { dim, values, norms }
    }

    pub fn dim(&self) -> usize {
        self.dim
    }

    pub fn len(&self) -> usize {
        self.values.len() / self.dim
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn row(&self, id: usize) -> &[f32] {
        &self.values[id * self.dim..(id + 1) * self.dim]
    }

    pub fn norm(&self, id: usize) -> f64 {
        self.norms[id]
    }

    /// The first `rows` vectors.
    pub fn prefix(&self, rows: usize) -> Vectors {
        let rows = rows.min(self.len());
        Vectors {
            dim: self.dim,
            values: self.values[..rows * self.dim].to_vec(),
            norms: self.norms[..rows].to_vec(),
        }
    }

    pub fn values(&self) -> &[f32] {
        &self.values
    }
}
