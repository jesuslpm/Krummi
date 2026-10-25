//! Helpers shared by the transaction tests.

#![allow(dead_code)]

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use krummi_pager::testing::{self, Model, Recorded};
use krummi_tx::{Engine, OpenOptions, WriteTransaction};
use tempfile::TempDir;

pub fn options() -> OpenOptions {
    OpenOptions::default()
}

pub fn disk(folder: &Path) -> Engine {
    Engine::open(folder, &options()).unwrap()
}

pub fn temporary() -> Engine {
    Engine::open_temporary(&options()).unwrap()
}

/// Runs `test` on a store on disk and on a temporary one.
pub fn both(test: impl Fn(&Engine)) {
    let tmp = tempfile::tempdir().unwrap();
    test(&disk(tmp.path()));
    test(&temporary());
}

/// Replaces the payload of `page` with `text`.
pub fn put(tx: &mut WriteTransaction<'_>, page: u64, text: &str) {
    let payload = tx.page_mut(page).unwrap();
    payload.fill(0);
    payload[..text.len()].copy_from_slice(text.as_bytes());
}

/// The text at the start of a payload.
pub fn text(payload: &[u8]) -> String {
    let end = payload
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(payload.len());
    String::from_utf8(payload[..end].to_vec()).unwrap()
}

/// Reserves a page holding `text` and commits it.
pub fn new_page(engine: &Engine, text: &str) -> u64 {
    let mut tx = engine.write(None).unwrap();
    let page = tx.allocate().unwrap();
    put(&mut tx, page, text);
    tx.commit().unwrap();
    page
}

/// Commits `text` into an existing page.
pub fn set(engine: &Engine, page: u64, text: &str) {
    let mut tx = engine.write(None).unwrap();
    put(&mut tx, page, text);
    tx.commit().unwrap();
}

/// What a new read transaction sees in `page`.
pub fn read_text(engine: &Engine, page: u64) -> String {
    text(engine.read().unwrap().page(page).unwrap())
}

/// Waits until `condition` holds, failing the test after 10 s.
pub fn wait_until(condition: impl Fn() -> bool) {
    let start = Instant::now();
    while !condition() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "condition never held"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Runs `commit` on a new thread that shares this thread's fault state and recording, and waits until the
/// journal's queue holds `queued` commits.
pub fn queue_on_thread<'s, T: Send + 's>(
    s: &'s std::thread::Scope<'s, '_>,
    engine: &'s Engine,
    queued: usize,
    commit: impl FnOnce() -> T + Send + 's,
) -> std::thread::ScopedJoinHandle<'s, T> {
    let context = krummi_pager::testing::context();
    let handle = s.spawn(move || {
        krummi_pager::testing::adopt(&context);
        commit()
    });
    wait_until(|| engine.journal_queued() == queued);
    handle
}

/// The write transaction, once the commit that passed its point of no return on another thread has released the
/// turn (it still holds it for an instant after entering the journal's queue).
pub fn next_writer(engine: &Engine) -> WriteTransaction<'_> {
    let start = Instant::now();
    loop {
        match engine.write(Some(Duration::ZERO)) {
            Ok(tx) => return tx,
            Err(e) if e.kind() == krummi_tx::ErrorKind::Timeout => {
                assert!(
                    start.elapsed() < Duration::from_secs(10),
                    "the turn was never released"
                );
                std::thread::yield_now();
            }
            Err(e) => panic!("{e}"),
        }
    }
}

pub const POWER_CUTS: [Model; 3] = [Model::Flushed, Model::TornFirst, Model::SkipFirst];

/// A recorded run: the operations, the folder they were made in, and the number of operations recorded when
/// each commit had returned.
pub struct Run {
    pub ops: Vec<Recorded>,
    pub source: TempDir,
    pub returned: Vec<usize>,
}

impl Run {
    pub fn start() -> (TempDir, Engine) {
        let source = Run::begin();
        let engine = disk(source.path());
        (source, engine)
    }

    /// Starts recording this thread's operations and returns a new folder for the store of the run (the facade's
    /// tests open a `Store` there; this crate's, an `Engine`).
    pub fn begin() -> TempDir {
        testing::reset();
        testing::start_recording();
        tempfile::tempdir().unwrap()
    }

    pub fn finish(source: TempDir, returned: Vec<usize>) -> Run {
        let ops = testing::take_recording();
        testing::reset();
        Run {
            ops,
            source,
            returned,
        }
    }

    /// What survives a crash after the first `cut` operations, in a new folder.
    pub fn crash(&self, cut: usize, model: Model) -> TempDir {
        let target = tempfile::tempdir().unwrap();
        testing::materialize(&self.ops, cut, self.source.path(), target.path(), model);
        target
    }

    /// What survives when the process dies at the end.
    pub fn killed(&self) -> TempDir {
        self.crash(self.ops.len(), Model::Written)
    }

    /// Commits that had returned at `cut`.
    pub fn returned_at(&self, cut: usize) -> usize {
        self.returned.iter().filter(|&&r| r <= cut).count()
    }
}

/// Copies the files of a store folder into a new temporary folder.
pub fn copy_folder(from: &Path) -> TempDir {
    let to = tempfile::tempdir().unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            fs::copy(&path, to.path().join(path.file_name().unwrap())).unwrap();
        }
    }
    to
}
