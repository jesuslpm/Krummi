//! The bounded top-k of document 5 and the exact search over every vector, which is the ground truth.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::data::Vectors;
use crate::kernels::Metric;

/// A vector of the base and its distance to the query.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Neighbour {
    pub distance: f64,
    pub id: u32,
}

impl Neighbour {
    /// Nearer first; at the same distance, the lower id first (document 12: "ties by primary key").
    pub fn order(&self, other: &Neighbour) -> Ordering {
        self.distance
            .total_cmp(&other.distance)
            .then(self.id.cmp(&other.id))
    }
}

/// A heap entry ordered by [`Neighbour::order`], so the heap's top is the worst one kept.
struct Entry(Neighbour);

impl PartialEq for Entry {
    fn eq(&self, other: &Entry) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Entry {}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Entry) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Entry {
    fn cmp(&self, other: &Entry) -> Ordering {
        self.0.order(&other.0)
    }
}

/// The `k` nearest neighbours seen so far.
pub struct TopK {
    k: usize,
    heap: BinaryHeap<Entry>,
}

impl TopK {
    pub fn new(k: usize) -> TopK {
        TopK {
            k,
            heap: BinaryHeap::with_capacity(k + 1),
        }
    }

    /// Offers a candidate; it is kept if fewer than `k` are kept or it is better than the worst kept.
    pub fn push(&mut self, distance: f64, id: u32) {
        let candidate = Neighbour { distance, id };
        if self.heap.len() < self.k {
            self.heap.push(Entry(candidate));
        } else if let Some(mut worst) = self.heap.peek_mut()
            && candidate.order(&worst.0) == Ordering::Less
        {
            *worst = Entry(candidate);
        }
    }

    /// The kept neighbours, nearest first.
    pub fn into_sorted(self) -> Vec<Neighbour> {
        self.heap
            .into_sorted_vec()
            .into_iter()
            .map(|e| e.0)
            .collect()
    }
}

/// The exact `k` nearest neighbours of `query` among every vector of `base`, with the kernels of document 5.
pub fn exact(base: &Vectors, query: &[f32], metric: Metric, k: usize) -> Vec<Neighbour> {
    let query_norm = crate::kernels::norm(query);
    let mut top = TopK::new(k);
    for id in 0..base.len() {
        let distance = metric.distance(base.row(id), query, base.norm(id), query_norm);
        top.push(distance, id as u32);
    }
    top.into_sorted()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_k_nearest_with_ties_by_id() {
        let mut top = TopK::new(3);
        for (distance, id) in [(5.0, 0), (1.0, 7), (3.0, 2), (1.0, 4), (9.0, 1), (3.0, 1)] {
            top.push(distance, id);
        }
        let ids: Vec<u32> = top.into_sorted().iter().map(|n| n.id).collect();
        assert_eq!(ids, [4, 7, 1]);
    }

    #[test]
    fn exact_search_finds_the_same_as_sorting_every_distance() {
        let dim = 19;
        let values: Vec<f32> = (0..dim * 300)
            .map(|i| ((i * 7919 % 1013) as f32 - 500.0) / 37.0)
            .collect();
        let base = Vectors::new(dim, values);
        for metric in [Metric::L2Squared, Metric::Dot, Metric::Cosine] {
            let query = base.row(17).to_vec();
            let mut all: Vec<Neighbour> = (0..base.len())
                .map(|id| Neighbour {
                    distance: metric.distance(
                        base.row(id),
                        &query,
                        base.norm(id),
                        crate::kernels::norm(&query),
                    ),
                    id: id as u32,
                })
                .collect();
            all.sort_by(Neighbour::order);
            assert_eq!(exact(&base, &query, metric, 10), all[..10], "{metric:?}");
        }
    }
}
