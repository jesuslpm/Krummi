//! What the orchestrator and the workers say to each other (design D4). The Rust worker and the .NET program for
//! Voron print the same lines.
//!
//! A worker is started as `<worker> <command> <arguments>`:
//!
//! - `run <engine> <load> <scale> <store> [<source>]`: runs a load. `<store>` is a folder that does not exist yet,
//!   where the worker creates its repetitions' stores; `<source>` is the closed store the load starts from (a copy
//!   of the base store, or of the last measured repetition of W17, W19 or W21), which the worker copies or opens as
//!   the load needs. It leaves the store of its last repetition closed at `<store>/last` for the size of K9.
//! - `prepare <engine> <scale> <store>`: builds the base store (W2's data, not measured) and closes it.
//! - `versions`: prints the versions line.
//!
//! On success it prints only protocol lines on standard output and exits with `0`. On failure it prints one line
//! with the cause on standard error and exits with `1`; anything it printed before is ignored.

use crate::data::Digest;
use crate::measure::Repetition;
use crate::workload::Load;

/// One line of a worker's standard output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// `data <load> <digest>`: the digest of the data the load received (AC-K7), once per load and process.
    Data { load: Load, digest: u64 },
    /// `rep <index> <load> <operations> <nanoseconds>`: a repetition. W12's process prints W12 and W13 lines.
    Rep { load: Load, repetition: Repetition },
    /// `versions <dotnet or lmdb> <version>`: a version for the report.
    Version { component: String, version: String },
}

impl Line {
    pub fn data(load: Load, digest: Digest) -> String {
        format!("data {} {}", load.id(), digest.hex())
    }

    pub fn rep(load: Load, index: u32, operations: u64, nanos: u64) -> String {
        format!("rep {index} {} {operations} {nanos}", load.id())
    }

    pub fn parse(line: &str) -> Option<Line> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts.as_slice() {
            ["data", load, digest] => Some(Line::Data {
                load: Load::from_id(load)?,
                digest: u64::from_str_radix(digest, 16).ok()?,
            }),
            ["rep", index, load, operations, nanos] => Some(Line::Rep {
                load: Load::from_id(load)?,
                repetition: Repetition {
                    index: index.parse().ok()?,
                    operations: operations.parse().ok()?,
                    nanos: nanos.parse().ok()?,
                },
            }),
            ["versions", component, version] => Some(Line::Version {
                component: component.to_string(),
                version: version.to_string(),
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_round_trip() {
        let mut d = Digest::default();
        d.feed(b"k");
        assert_eq!(
            Line::parse(&Line::data(Load::W7, d)),
            Some(Line::Data {
                load: Load::W7,
                digest: d.value()
            })
        );
        assert_eq!(Line::rep(Load::W13, 2, 100_000, 5), "rep 2 W13 100000 5");
        assert_eq!(
            Line::parse("rep 2 W13 100000 5"),
            Some(Line::Rep {
                load: Load::W13,
                repetition: Repetition {
                    index: 2,
                    operations: 100_000,
                    nanos: 5
                }
            })
        );
        assert_eq!(
            Line::parse("versions lmdb 0.9.70"),
            Some(Line::Version {
                component: "lmdb".into(),
                version: "0.9.70".into()
            })
        );
        assert_eq!(Line::parse("rep x W1 1 1"), None);
        assert_eq!(Line::parse("hello"), None);
    }
}
