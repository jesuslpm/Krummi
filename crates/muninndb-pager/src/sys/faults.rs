//! Test-only fault injection and operation recording for [`crate::sys`].
//!
//! State is per thread, so tests running in parallel do not interfere.

#![cfg_attr(not(test), allow(dead_code))]

use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use super::Op;

/// A file-system operation that succeeded, as recorded for the power-cut simulation.
#[derive(Debug, Clone)]
pub enum Recorded {
    Create(PathBuf),
    Write(PathBuf, u64, Vec<u8>),
    SetLen(PathBuf, u64),
    Sync(PathBuf),
    Rename(PathBuf, PathBuf),
    Remove(PathBuf),
    SyncDir(PathBuf),
}

/// What an injected fault does.
#[derive(Debug, Clone, Copy)]
pub enum Action {
    /// The operation fails with this error kind and has no effect.
    Fail(io::ErrorKind),
    /// The thread panics before the operation, which has no effect.
    Panic,
}

#[derive(Default)]
struct State {
    faults: Vec<(Op, usize, Action)>,
    counts: HashMap<Op, usize>,
    abort_at_point: Option<usize>,
    points: usize,
    recording: Option<Vec<Recorded>>,
    skip_syncs: bool,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Makes the `nth` (from 0) future call of `op` on this thread do `action`.
pub fn inject(op: Op, nth: usize, action: Action) {
    STATE.with(|s| s.borrow_mut().faults.push((op, nth, action)));
}

/// Number of calls of `op` on this thread since the last [`reset`].
pub fn count(op: Op) -> usize {
    STATE.with(|s| s.borrow().counts.get(&op).copied().unwrap_or(0))
}

/// Makes the flushes to disk of this thread succeed without flushing, for tests that check damage detection and
/// not durability, such as the AC10 sweeps, which reopen a store tens of thousands of times.
pub fn skip_syncs(skip: bool) {
    STATE.with(|s| s.borrow_mut().skip_syncs = skip);
}

/// Whether this thread skips its flushes to disk.
pub(crate) fn syncs_skipped() -> bool {
    STATE.with(|s| s.borrow().skip_syncs)
}

/// Kills the process before the operation number `point` (from 0, counting every operation of this thread).
pub fn abort_at(point: usize) {
    STATE.with(|s| s.borrow_mut().abort_at_point = Some(point));
}

/// Number of operations this thread has gone through since the last [`reset`].
pub fn points() -> usize {
    STATE.with(|s| s.borrow().points)
}

/// Clears faults, counters and recordings of this thread.
pub fn reset() {
    STATE.with(|s| *s.borrow_mut() = State::default());
}

/// Starts recording the operations of this thread.
pub fn start_recording() {
    STATE.with(|s| s.borrow_mut().recording = Some(Vec::new()));
}

/// Number of operations recorded so far, without stopping the recording.
pub fn recorded_len() -> usize {
    STATE.with(|s| s.borrow().recording.as_ref().map_or(0, Vec::len))
}

/// Stops recording and returns what was recorded.
pub fn take_recording() -> Vec<Recorded> {
    STATE.with(|s| s.borrow_mut().recording.take().unwrap_or_default())
}

pub(crate) mod hooks {
    use super::*;

    pub(crate) fn before(op: Op, _path: &Path) -> io::Result<()> {
        let action = STATE.with(|s| {
            let mut s = s.borrow_mut();
            let point = s.points;
            s.points += 1;
            if s.abort_at_point == Some(point) {
                std::process::abort();
            }
            let count = s.counts.entry(op).or_insert(0);
            let nth = *count;
            *count += 1;
            s.faults
                .iter()
                .find(|(o, n, _)| *o == op && *n == nth)
                .map(|f| f.2)
        });
        match action {
            Some(Action::Fail(kind)) => Err(io::Error::new(kind, "injected fault")),
            Some(Action::Panic) => panic!("injected panic before {op:?}"),
            None => Ok(()),
        }
    }

    pub(crate) fn record(op: Recorded) {
        STATE.with(|s| {
            if let Some(ops) = s.borrow_mut().recording.as_mut() {
                ops.push(op);
            }
        });
    }
}

/// What survives of the writes made to a file since its last flush.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    /// Nothing: each file has what it had at its last flush.
    Flushed,
    /// The first half of the first write after the last flush, as a torn write.
    TornFirst,
    /// Every write after the last flush except the first one, as when the disk reorders them.
    SkipFirst,
    /// Every write, as when only the process dies and the system keeps what it was given; names are kept too.
    Written,
}

#[derive(Clone)]
enum Pending {
    Write(u64, Vec<u8>),
    SetLen(u64),
}

#[derive(Clone, Default)]
struct SimFile {
    durable: Vec<u8>,
    pending: Vec<Pending>,
}

impl SimFile {
    fn apply(content: &mut Vec<u8>, op: &Pending) {
        match op {
            Pending::Write(offset, data) => {
                let end = *offset as usize + data.len();
                if content.len() < end {
                    content.resize(end, 0);
                }
                content[*offset as usize..end].copy_from_slice(data);
            }
            Pending::SetLen(len) => content.resize(*len as usize, 0),
        }
    }

    fn flush(&mut self) {
        for op in std::mem::take(&mut self.pending) {
            SimFile::apply(&mut self.durable, &op);
        }
    }

    fn after_cut(&self, model: Model) -> Vec<u8> {
        let mut content = self.durable.clone();
        match model {
            Model::Flushed => {}
            Model::TornFirst => {
                if let Some(first) = self.pending.first() {
                    let torn = match first {
                        Pending::Write(offset, data) => {
                            Pending::Write(*offset, data[..data.len() / 2].to_vec())
                        }
                        other => other.clone(),
                    };
                    SimFile::apply(&mut content, &torn);
                }
            }
            Model::SkipFirst => {
                for op in self.pending.iter().skip(1) {
                    SimFile::apply(&mut content, op);
                }
            }
            Model::Written => {
                for op in &self.pending {
                    SimFile::apply(&mut content, op);
                }
            }
        }
        content
    }
}

/// Writes into `target` what survives a power cut after the first `cut` recorded operations under [`Model::Flushed`]
/// (see [`materialize`]).
pub fn materialize_power_cut(ops: &[Recorded], cut: usize, source: &Path, target: &Path) {
    materialize(ops, cut, source, target, Model::Flushed);
}

/// Writes into the folder `target` what survives a power cut after the first `cut` recorded operations, under a
/// pessimistic model: file contents survive up to their last flush, plus what `model` keeps of the later writes;
/// new names (creations, renames, removals) survive only after their folder is flushed on Unix, and on Windows
/// once the file itself is flushed (renames are written through). The files already in `target` are the
/// starting state, durable; recorded paths under `source` are rebased onto `target`. Subfolders are ignored.
pub fn materialize(ops: &[Recorded], cut: usize, source: &Path, target: &Path, model: Model) {
    let mut files: Vec<SimFile> = Vec::new();
    let mut live: HashMap<PathBuf, usize> = HashMap::new();
    for entry in std::fs::read_dir(target).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            files.push(SimFile {
                durable: std::fs::read(entry.path()).unwrap(),
                pending: Vec::new(),
            });
            live.insert(source.join(entry.file_name()), files.len() - 1);
        }
    }
    let mut durable = live.clone();
    for op in &ops[..cut] {
        match op {
            Recorded::Create(p) => {
                files.push(SimFile::default());
                live.insert(p.clone(), files.len() - 1);
            }
            Recorded::Write(p, offset, data) => {
                files[live[p]]
                    .pending
                    .push(Pending::Write(*offset, data.clone()));
            }
            Recorded::SetLen(p, len) => files[live[p]].pending.push(Pending::SetLen(*len)),
            Recorded::Sync(p) => {
                let id = live[p];
                files[id].flush();
                if cfg!(windows) {
                    durable.insert(p.clone(), id);
                }
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
            Recorded::SyncDir(_) => {
                if cfg!(unix) {
                    durable = live.clone();
                }
            }
        }
    }
    if model == Model::Written {
        durable = live;
    }
    for entry in std::fs::read_dir(target).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            std::fs::remove_file(entry.path()).unwrap();
        }
    }
    for (path, id) in durable {
        let relative = path.strip_prefix(source).unwrap();
        std::fs::write(target.join(relative), files[id].after_cut(model)).unwrap();
    }
}
