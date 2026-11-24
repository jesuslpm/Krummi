//! The in-memory experiment of design document 12 (*Benchmarks*, steps 1 and 2): Krummi's exact search as the
//! ground truth, the approximate SQ8 scan with rerank, IVF-Flat and IVF-SQ8. It is tooling, not part of the engine:
//! PRD 7.6 is written from its measurements and moves the kernels into the engine.

pub mod data;
pub mod files;
pub mod ivf;
pub mod kernels;
pub mod measure;
pub mod prepare;
pub mod run;
pub mod search;
pub mod sq8;
pub mod topk;
