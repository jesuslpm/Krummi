//! Test-only fault injection and operation recording for [`crate::sys`].
//!
//! State is per thread, so tests running in parallel do not interfere. A thread the store starts for itself (the
//! journal's writer) adopts the state of the thread that opened the store, so faults injected and operations
//! recorded by a test reach it too.

#![cfg_attr(not(test), allow(dead_code))]

use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

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
    static STATE: RefCell<Arc<Mutex<State>>> = RefCell::new(Arc::default());
}

/// Runs `f` on this thread's state.
fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow().lock().unwrap_or_else(PoisonError::into_inner)))
}

/// The fault state of a thread, for a thread the store starts to share it (see [`adopt`]).
#[derive(Clone, Default)]
pub struct Context(Arc<Mutex<State>>);

/// This thread's fault state.
pub fn context() -> Context {
    STATE.with(|s| Context(Arc::clone(&s.borrow())))
}

/// Makes this thread share the fault state `context`: faults, counters, recordings and skipped flushes.
pub fn adopt(context: &Context) {
    STATE.with(|s| *s.borrow_mut() = Arc::clone(&context.0));
}

/// Makes the `nth` (from 0) future call of `op` on this thread do `action`.
pub fn inject(op: Op, nth: usize, action: Action) {
    with(|s| s.faults.push((op, nth, action)));
}

/// Number of calls of `op` on this thread since the last [`reset`].
pub fn count(op: Op) -> usize {
    with(|s| s.counts.get(&op).copied().unwrap_or(0))
}

/// Makes the flushes to disk of this thread succeed without flushing, for tests that check damage detection and
/// not durability, such as the AC10 sweeps, which reopen a store tens of thousands of times.
pub fn skip_syncs(skip: bool) {
    with(|s| s.skip_syncs = skip);
}

/// Whether this thread skips its flushes to disk.
pub(crate) fn syncs_skipped() -> bool {
    with(|s| s.skip_syncs)
}

/// Kills the process before the operation number `point` (from 0, counting every operation of this thread).
pub fn abort_at(point: usize) {
    with(|s| s.abort_at_point = Some(point));
}

/// Number of operations this thread has gone through since the last [`reset`].
pub fn points() -> usize {
    with(|s| s.points)
}

/// Clears faults, counters and recordings of this thread, and of the threads that share its state.
pub fn reset() {
    with(|s| *s = State::default());
}

/// Starts recording the operations of this thread.
pub fn start_recording() {
    with(|s| s.recording = Some(Vec::new()));
}

/// Number of operations recorded so far, without stopping the recording.
pub fn recorded_len() -> usize {
    with(|s| s.recording.as_ref().map_or(0, Vec::len))
}

/// Stops recording and returns what was recorded.
pub fn take_recording() -> Vec<Recorded> {
    with(|s| s.recording.take().unwrap_or_default())
}

pub(crate) mod hooks {
    use super::*;

    pub(crate) fn before(op: Op, _path: &Path) -> io::Result<()> {
        let action = with(|s| {
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
        with(|s| {
            if let Some(ops) = s.recording.as_mut() {
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
/// starting state, durable; recorded paths under `source` are rebased onto `target`. Subfolders are ignored. A
/// write, size change, flush or rename of a file that is not live panics with the operation, the path and the
/// operations that create, rename or remove it (a removal of a name that is not live changes nothing); a file
/// outside `source` that would survive panics with the file and the folder.
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
    let file = |live: &HashMap<PathBuf, usize>, at: usize, path: &Path| match live.get(path) {
        Some(&id) => id,
        None => not_live(ops, at, path),
    };
    for (at, op) in ops[..cut].iter().enumerate() {
        match op {
            Recorded::Create(p) => {
                files.push(SimFile::default());
                live.insert(p.clone(), files.len() - 1);
            }
            Recorded::Write(p, offset, data) => {
                files[file(&live, at, p)]
                    .pending
                    .push(Pending::Write(*offset, data.clone()));
            }
            Recorded::SetLen(p, len) => files[file(&live, at, p)]
                .pending
                .push(Pending::SetLen(*len)),
            Recorded::Sync(p) => {
                let id = file(&live, at, p);
                files[id].flush();
                if cfg!(windows) {
                    durable.insert(p.clone(), id);
                }
            }
            Recorded::Rename(from, to) => {
                let id = live.remove(from).unwrap_or_else(|| not_live(ops, at, from));
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
        let relative = path.strip_prefix(source).unwrap_or_else(|_| {
            panic!(
                "recorded file {} is not in the source folder {}: the recording holds operations of another folder",
                path.display(),
                source.display()
            )
        });
        std::fs::write(target.join(relative), files[id].after_cut(model)).unwrap();
    }
}

/// Panics because recorded operation `at` uses `path` while no live file has that name, with the operations of the
/// whole recording that create, rename or remove it, before and after it: the threads of a store record their
/// operations after making them, so a recording can hold two operations of different threads out of order.
fn not_live(ops: &[Recorded], at: usize, path: &Path) -> ! {
    let naming = |range: std::ops::Range<usize>| {
        let named: Vec<String> = range
            .filter(|&i| ops[i].names(path))
            .map(|i| format!("{i} {}", ops[i].summary()))
            .collect();
        if named.is_empty() {
            "none".to_owned()
        } else {
            named.join(", ")
        }
    };
    panic!(
        "recorded operation {at} of {} ({}) uses {}, which is not a live file there; operations that create, rename \
         or remove it, before: {}; after: {}",
        ops.len(),
        ops[at].summary(),
        path.display(),
        naming(0..at),
        naming(at + 1..ops.len())
    )
}

impl Recorded {
    /// Whether this operation creates, renames or removes the file `path`.
    fn names(&self, path: &Path) -> bool {
        match self {
            Recorded::Create(p) | Recorded::Remove(p) => p == path,
            Recorded::Rename(from, to) => from == path || to == path,
            _ => false,
        }
    }

    /// The operation, without the bytes it wrote.
    fn summary(&self) -> String {
        match self {
            Recorded::Create(p) => format!("Create {}", p.display()),
            Recorded::Write(p, offset, data) => {
                format!("Write {} at {offset}, {} bytes", p.display(), data.len())
            }
            Recorded::SetLen(p, len) => format!("SetLen {} to {len}", p.display()),
            Recorded::Sync(p) => format!("Sync {}", p.display()),
            Recorded::Rename(from, to) => format!("Rename {} to {}", from.display(), to.display()),
            Recorded::Remove(p) => format!("Remove {}", p.display()),
            Recorded::SyncDir(p) => format!("SyncDir {}", p.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The message of the panic of materializing the first `cut` operations of `ops`, recorded in `source`.
    fn failure(ops: &[Recorded], cut: usize, source: &Path) -> String {
        let target = tempfile::tempdir().unwrap();
        let panic = std::panic::catch_unwind(|| {
            materialize(ops, cut, source, target.path(), Model::Flushed);
        })
        .expect_err("materializing the recording panics");
        match panic.downcast::<String>() {
            Ok(message) => *message,
            Err(panic) => (*panic.downcast::<&str>().unwrap()).to_owned(),
        }
    }

    #[test]
    fn a_write_to_a_file_never_created_names_the_operation_and_that_nothing_named_the_file() {
        let source = Path::new("source");
        let (a, b) = (source.join("a"), source.join("b"));
        let ops = [
            Recorded::Create(a),
            Recorded::Write(b.clone(), 8, vec![1, 2, 3]),
        ];
        assert_eq!(
            failure(&ops, 2, source),
            format!(
                "recorded operation 1 of 2 (Write {} at 8, 3 bytes) uses {}, which is not a live file there; \
                 operations that create, rename or remove it, before: none; after: none",
                b.display(),
                b.display()
            )
        );
    }

    #[test]
    fn a_write_after_the_removal_of_its_file_names_the_creation_and_the_removal() {
        let source = Path::new("source");
        let a = source.join("a");
        let ops = [
            Recorded::Create(a.clone()),
            Recorded::Remove(a.clone()),
            Recorded::Write(a.clone(), 0, vec![7]),
        ];
        assert_eq!(
            failure(&ops, 3, source),
            format!(
                "recorded operation 2 of 3 (Write {a} at 0, 1 bytes) uses {a}, which is not a live file there; \
                 operations that create, rename or remove it, before: 0 Create {a}, 1 Remove {a}; after: none",
                a = a.display()
            )
        );
    }

    #[test]
    fn a_rename_recorded_after_the_flush_of_its_file_is_named_as_later() {
        let source = Path::new("source");
        let (a, b) = (source.join("a"), source.join("b"));
        let ops = [
            Recorded::Sync(a.clone()),
            Recorded::Create(b.clone()),
            Recorded::Rename(b.clone(), a.clone()),
        ];
        // Only the operations before the cut are materialized, but the later ones are named.
        assert_eq!(
            failure(&ops, 1, source),
            format!(
                "recorded operation 0 of 3 (Sync {a}) uses {a}, which is not a live file there; operations that \
                 create, rename or remove it, before: none; after: 2 Rename {b} to {a}",
                a = a.display(),
                b = b.display()
            )
        );
    }

    #[test]
    fn a_rename_and_a_size_change_of_a_file_that_is_not_live_name_the_operation() {
        let source = Path::new("source");
        let (a, b) = (source.join("a"), source.join("b"));
        let rename = [Recorded::Rename(a.clone(), b.clone())];
        assert!(failure(&rename, 1, source).starts_with(&format!(
            "recorded operation 0 of 1 (Rename {} to {}) uses {}, which is not a live file there;",
            a.display(),
            b.display(),
            a.display()
        )));
        let set_len = [Recorded::SetLen(a.clone(), 4096)];
        assert!(failure(&set_len, 1, source).starts_with(&format!(
            "recorded operation 0 of 1 (SetLen {} to 4096) uses {},",
            a.display(),
            a.display()
        )));
    }

    #[test]
    fn a_file_outside_the_source_folder_is_named_with_the_folder() {
        let source = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let stray = other.path().join("krummi.data");
        // Durable on Windows once the file is flushed, and on Unix once its folder is.
        let ops = [
            Recorded::Create(stray.clone()),
            Recorded::Sync(stray.clone()),
            Recorded::SyncDir(other.path().to_path_buf()),
        ];
        assert_eq!(
            failure(&ops, 3, source.path()),
            format!(
                "recorded file {} is not in the source folder {}: the recording holds operations of another \
                 folder",
                stray.display(),
                source.path().display()
            )
        );
    }

    #[test]
    fn a_removal_of_a_name_that_is_not_live_changes_nothing() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        std::fs::write(target.path().join("kept"), b"kept").unwrap();
        let ops = [
            Recorded::Remove(source.path().join("missing")),
            Recorded::SyncDir(source.path().to_path_buf()),
        ];
        materialize(&ops, 2, source.path(), target.path(), Model::Flushed);
        let names: Vec<_> = std::fs::read_dir(target.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["kept"]);
        assert_eq!(std::fs::read(target.path().join("kept")).unwrap(), b"kept");
    }
}
