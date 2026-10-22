//! Test-only fault injection and operation recording for [`crate::sys`].
//!
//! State is per thread, so tests running in parallel do not interfere.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use super::Op;

/// A file-system operation that succeeded, as recorded for the power-cut simulation.
#[derive(Debug, Clone)]
pub(crate) enum Recorded {
    Create(PathBuf),
    Write(PathBuf, u64, Vec<u8>),
    SetLen(PathBuf, u64),
    Sync(PathBuf),
    Rename(PathBuf, PathBuf),
    Remove(PathBuf),
    SyncDir,
}

/// What an injected fault does.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Action {
    /// The operation fails with this error kind and has no effect.
    Fail(io::ErrorKind),
}

#[derive(Default)]
struct State {
    faults: Vec<(Op, usize, Action)>,
    counts: HashMap<Op, usize>,
    abort_at_point: Option<usize>,
    points: usize,
    recording: Option<Vec<Recorded>>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Makes the `nth` (from 0) future call of `op` on this thread do `action`.
pub(crate) fn inject(op: Op, nth: usize, action: Action) {
    STATE.with(|s| s.borrow_mut().faults.push((op, nth, action)));
}

/// Kills the process before the operation number `point` (from 0, counting every operation of this thread).
pub(crate) fn abort_at(point: usize) {
    STATE.with(|s| s.borrow_mut().abort_at_point = Some(point));
}

/// Number of operations this thread has gone through since the last [`reset`].
pub(crate) fn points() -> usize {
    STATE.with(|s| s.borrow().points)
}

/// Clears faults, counters and recordings of this thread.
pub(crate) fn reset() {
    STATE.with(|s| *s.borrow_mut() = State::default());
}

/// Starts recording the operations of this thread.
pub(crate) fn start_recording() {
    STATE.with(|s| s.borrow_mut().recording = Some(Vec::new()));
}

/// Stops recording and returns what was recorded.
pub(crate) fn take_recording() -> Vec<Recorded> {
    STATE.with(|s| s.borrow_mut().recording.take().unwrap_or_default())
}

pub(crate) mod hooks {
    use super::*;

    pub(crate) fn before(op: Op, _path: &Path) -> io::Result<()> {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let point = s.points;
            s.points += 1;
            if s.abort_at_point == Some(point) {
                std::process::abort();
            }
            let count = s.counts.entry(op).or_insert(0);
            let nth = *count;
            *count += 1;
            match s
                .faults
                .iter()
                .find(|(o, n, _)| *o == op && *n == nth)
                .map(|f| f.2)
            {
                Some(Action::Fail(kind)) => Err(io::Error::new(kind, "injected fault")),
                None => Ok(()),
            }
        })
    }

    pub(crate) fn record(op: Recorded) {
        STATE.with(|s| {
            if let Some(ops) = s.borrow_mut().recording.as_mut() {
                ops.push(op);
            }
        });
    }
}

/// Writes into the empty folder `target` what survives a power cut after the first `cut` recorded operations,
/// under a pessimistic model: file contents survive only up to their last flush, and new names (creations,
/// renames, removals) survive only after their folder is flushed, except renames on Windows, which the Pager
/// writes through. Recorded paths under `source` are rebased onto `target`.
pub(crate) fn materialize_power_cut(ops: &[Recorded], cut: usize, source: &Path, target: &Path) {
    let mut contents: Vec<(Vec<u8>, Vec<u8>)> = Vec::new(); // (volatile, durable) per file id
    let mut live: HashMap<PathBuf, usize> = HashMap::new();
    let mut durable: HashMap<PathBuf, usize> = HashMap::new();
    for op in &ops[..cut] {
        match op {
            Recorded::Create(p) => {
                contents.push((Vec::new(), Vec::new()));
                live.insert(p.clone(), contents.len() - 1);
            }
            Recorded::Write(p, offset, data) => {
                let file = &mut contents[live[p]].0;
                let end = *offset as usize + data.len();
                if file.len() < end {
                    file.resize(end, 0);
                }
                file[*offset as usize..end].copy_from_slice(data);
            }
            Recorded::SetLen(p, len) => contents[live[p]].0.resize(*len as usize, 0),
            Recorded::Sync(p) => {
                let id = live[p];
                contents[id].1 = contents[id].0.clone();
            }
            Recorded::Rename(from, to) => {
                let id = live.remove(from).unwrap();
                live.insert(to.clone(), id);
                if cfg!(windows) {
                    durable.remove(from);
                    durable.insert(to.clone(), id);
                }
            }
            Recorded::Remove(p) => {
                live.remove(p);
            }
            Recorded::SyncDir => {
                if cfg!(unix) {
                    durable = live.clone();
                }
            }
        }
    }
    for (path, id) in durable {
        let relative = path.strip_prefix(source).unwrap();
        std::fs::write(target.join(relative), &contents[id].1).unwrap();
    }
}
