//! Recovery after crashes and power cuts (Journal PRD: J4, J5, J8–J14; parent AC1–AC5, AC12).
//!
//! Crashes are simulated by recording the file-system operations of a run and writing into a fresh folder what
//! survives at a cut point: under `Model::Written` everything written survives (the process died, the system did
//! not); under the other models only what was flushed, plus a torn or reordered unflushed write (a power cut).

mod common;

use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};

use common::{put, text};
use krummi_pager::format::{HEADER_COPY_OFFSETS, header_copies};
use krummi_pager::testing::{self, Action, Model, Op, Recorded};
use krummi_pager::{DATA_FILE, JOURNAL_PREFIX, TEMP_DIR};
use krummi_tx::{Engine, ErrorKind, OpenOptions};
use tempfile::TempDir;

const PAGES_PER_COMMIT: usize = 3;
const POWER_CUTS: [Model; 3] = [Model::Flushed, Model::TornFirst, Model::SkipFirst];

/// A recorded run: the operations, the folder they were made in, and the number of operations recorded when
/// each commit had returned.
struct Run {
    ops: Vec<Recorded>,
    source: TempDir,
    returned: Vec<usize>,
}

impl Run {
    fn start() -> (TempDir, Engine) {
        testing::reset();
        testing::start_recording();
        let source = tempfile::tempdir().unwrap();
        let engine = common::disk(source.path());
        (source, engine)
    }

    fn finish(source: TempDir, returned: Vec<usize>) -> Run {
        let ops = testing::take_recording();
        testing::reset();
        Run {
            ops,
            source,
            returned,
        }
    }

    /// What survives a crash after the first `cut` operations, in a new folder.
    fn crash(&self, cut: usize, model: Model) -> TempDir {
        let target = tempfile::tempdir().unwrap();
        testing::materialize(&self.ops, cut, self.source.path(), target.path(), model);
        target
    }

    /// What survives when the process dies at the end.
    fn killed(&self) -> TempDir {
        self.crash(self.ops.len(), Model::Written)
    }

    /// Commits that had returned at `cut`.
    fn returned_at(&self, cut: usize) -> usize {
        self.returned.iter().filter(|&&r| r <= cut).count()
    }
}

/// Commits `value` into `pages` (all of them, so a partial commit shows) and returns the recorded count.
fn commit_all(engine: &Engine, pages: &[u64], value: &str, durable: bool) -> usize {
    let mut tx = engine.write(None).unwrap();
    for &page in pages {
        put(&mut tx, page, value);
    }
    if durable {
        tx.commit().unwrap();
    } else {
        tx.commit_deferred().unwrap();
    }
    testing::recorded_len()
}

/// Reserves `count` pages holding `value` in one commit.
fn reserve(engine: &Engine, count: usize, value: &str) -> Vec<u64> {
    let mut tx = engine.write(None).unwrap();
    let pages: Vec<u64> = (0..count).map(|_| tx.allocate().unwrap()).collect();
    for &page in &pages {
        put(&mut tx, page, value);
    }
    tx.commit().unwrap();
    pages
}

/// The texts of `pages`, or `None` for a page that cannot be read (never committed).
fn texts(engine: &Engine, pages: &[u64]) -> Vec<Option<String>> {
    let read = engine.read().unwrap();
    pages.iter().map(|&p| read.page(p).ok().map(text)).collect()
}

fn open(folder: &Path) -> Engine {
    common::disk(folder)
}

fn open_err(folder: &Path) -> krummi_tx::Error {
    match Engine::open(folder, &OpenOptions::default()) {
        Ok(_) => panic!("opening {} should fail", folder.display()),
        Err(e) => e,
    }
}

fn journal_files(folder: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(folder)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(JOURNAL_PREFIX)
        })
        .collect();
    files.sort();
    files
}

/// Offsets of the records of a journal file (its layout: 64-byte headers, records aligned to 4 KiB).
fn record_offsets(file: &Path) -> Vec<usize> {
    let bytes = fs::read(file).unwrap();
    let mut offsets = Vec::new();
    let mut offset = 0;
    while offset + 64 <= bytes.len() && bytes[offset..offset + 8] == *b"MuninnJ1" {
        offsets.push(offset);
        let stored =
            u64::from_le_bytes(bytes[offset + 32..offset + 40].try_into().unwrap()) as usize;
        offset += (64 + stored).next_multiple_of(4096);
    }
    offsets
}

fn flip(file: &Path, offset: usize) {
    let mut bytes = fs::read(file).unwrap();
    bytes[offset] ^= 0x01;
    fs::write(file, bytes).unwrap();
}

fn snapshot(folder: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if path.is_dir() {
                out.push((name + "/", Vec::new()));
                walk(base, &path, out);
            } else {
                out.push((name, fs::read(&path).unwrap()));
            }
        }
    }
    walk(folder, folder, &mut out);
    out.sort();
    out
}

fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// A run of durable commits, each writing `c<i>` into the same three pages.
fn durable_run(commits: usize) -> (Run, Vec<u64>) {
    let (source, engine) = Run::start();
    let pages = reserve(&engine, PAGES_PER_COMMIT, "c0");
    let mut returned = vec![testing::recorded_len()];
    for i in 1..=commits {
        returned.push(commit_all(&engine, &pages, &format!("c{i}"), true));
    }
    (Run::finish(source, returned), pages)
}

/// AC1, AC2, AC4 (J4, J5, G1): after a crash or a power cut at every point, every commit that returned is there,
/// the one in progress is entire or absent, and nothing is ever half applied.
#[test]
fn ac1_ac2_ac4_returned_commits_survive_any_crash_and_none_is_half_applied() {
    let (run, pages) = durable_run(5);
    for cut in 0..=run.ops.len() {
        for model in POWER_CUTS.into_iter().chain([Model::Written]) {
            let folder = run.crash(cut, model);
            if !folder.path().join(DATA_FILE).exists() {
                continue; // The creation itself did not survive: the Pager's AC-P9.
            }
            let engine = open(folder.path());
            let seen = texts(&engine, &pages);
            assert!(
                seen.windows(2).all(|w| w[0] == w[1]),
                "cut {cut} {model:?}: {seen:?}"
            );
            let returned = run.returned_at(cut);
            let allowed: Vec<Option<String>> = [returned, returned + 1]
                .iter()
                .map(|&n| (n >= 1).then(|| format!("c{}", n - 1)))
                .collect();
            assert!(
                allowed.contains(&seen[0]),
                "cut {cut} {model:?}: {seen:?}, {returned} returned"
            );
            engine.close().unwrap();
            assert_eq!(names(folder.path()), vec![DATA_FILE], "cut {cut} {model:?}");
        }
    }
}

/// AC3: a write transaction that never started to commit leaves nothing after a crash.
#[test]
fn ac3_an_open_write_leaves_nothing_after_a_crash() {
    let (source, engine) = Run::start();
    let pages = reserve(&engine, 2, "kept");
    let mut tx = engine.write(None).unwrap();
    put(&mut tx, pages[0], "lost");
    let run = Run::finish(source, vec![]);
    drop(tx);
    let folder = run.killed();
    let engine = open(folder.path());
    assert_eq!(texts(&engine, &pages), vec![Some("kept".into()); 2]);
}

/// AC5 (G5): a recovery interrupted at any point, by the process dying or a power cut, ends like an
/// uninterrupted one at the next opening.
#[test]
fn ac5_an_interrupted_recovery_is_repeated_with_the_same_result() {
    let (run, pages) = durable_run(4);
    let crashed = run.killed();
    let expected = {
        let copy = copy_folder(crashed.path());
        let engine = open(copy.path());
        texts(&engine, &pages)
    };
    assert_eq!(expected[0].as_deref(), Some("c4"));
    testing::reset();
    testing::start_recording();
    let source = copy_folder(crashed.path());
    drop(open(source.path()));
    let recovery = testing::take_recording();
    testing::reset();
    assert!(!recovery.is_empty());
    for cut in 0..=recovery.len() {
        for model in POWER_CUTS.into_iter().chain([Model::Written]) {
            let target = copy_folder(crashed.path());
            testing::materialize(&recovery, cut, source.path(), target.path(), model);
            let engine = open(target.path());
            assert_eq!(texts(&engine, &pages), expected, "cut {cut} {model:?}");
        }
    }
}

fn copy_folder(from: &Path) -> TempDir {
    let to = tempfile::tempdir().unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            fs::copy(&path, to.path().join(path.file_name().unwrap())).unwrap();
        }
    }
    to
}

/// AC-J7 (J10, J11): with deferred commits, any crash keeps a prefix of the commits that includes every durable
/// one that returned; a process crash keeps them all, and so does J11 or a close before a power cut.
#[test]
fn ac_j7_deferred_commits_survive_as_a_prefix() {
    let durable = [true, false, false, true, false, false];
    let (source, engine) = Run::start();
    let pages = reserve(&engine, durable.len(), "-");
    let mut returned = Vec::new();
    for (i, &mode) in durable.iter().enumerate() {
        let mut tx = engine.write(None).unwrap();
        put(&mut tx, pages[i], &format!("c{i}"));
        if mode {
            tx.commit().unwrap();
        } else {
            tx.commit_deferred().unwrap();
        }
        returned.push(testing::recorded_len());
    }
    let run = Run::finish(source, returned);
    for cut in 0..=run.ops.len() {
        for model in POWER_CUTS {
            let folder = run.crash(cut, model);
            if !folder.path().join(DATA_FILE).exists() {
                continue;
            }
            let engine = open(folder.path());
            let seen = texts(&engine, &pages);
            let present: Vec<bool> = seen
                .iter()
                .enumerate()
                .map(|(i, t)| t.as_deref() == Some(format!("c{i}").as_str()))
                .collect();
            let prefix = present.iter().take_while(|&&p| p).count();
            assert!(
                present[prefix..].iter().all(|&p| !p),
                "cut {cut} {model:?}: {present:?}"
            );
            let must = (0..durable.len())
                .filter(|&i| durable[i] && run.returned[i] <= cut)
                .map(|i| i + 1)
                .max()
                .unwrap_or(0);
            assert!(
                prefix >= must,
                "cut {cut} {model:?}: {present:?}, durable up to {must}"
            );
        }
    }
    let folder = run.killed();
    let engine = open(folder.path());
    assert!(
        texts(&engine, &pages)
            .iter()
            .enumerate()
            .all(|(i, t)| t.as_deref() == Some(format!("c{i}").as_str()))
    );

    // J11 or a clean close before the cut keeps every deferred commit.
    for close in [false, true] {
        let (source, engine) = Run::start();
        let pages = reserve(&engine, 1, "a");
        commit_all(&engine, &pages, "deferred", false);
        if close {
            engine.close().unwrap();
        } else {
            engine.make_durable().unwrap();
        }
        let run = Run::finish(source, vec![]);
        for model in POWER_CUTS {
            let folder = run.crash(run.ops.len(), model);
            let engine = open(folder.path());
            assert_eq!(
                texts(&engine, &pages)[0].as_deref(),
                Some("deferred"),
                "close {close} {model:?}"
            );
        }
    }
}

/// AC-J2 (J5, J8): commits a long read kept from the data file are recovered after a crash, and the next close
/// leaves only `muninn.data`.
#[test]
fn ac_j2_commits_held_back_by_a_read_are_recovered() {
    let (source, engine) = Run::start();
    let pages = reserve(&engine, 2, "a");
    let read = engine.read().unwrap();
    for i in 0..5 {
        commit_all(&engine, &pages, &format!("v{i}"), true);
    }
    let run = Run::finish(source, vec![]);
    drop(read);
    let folder = run.killed();
    let engine = open(folder.path());
    assert_eq!(texts(&engine, &pages), vec![Some("v4".into()); 2]);
    engine.close().unwrap();
    assert_eq!(names(folder.path()), vec![DATA_FILE]);
}

/// A crashed store whose journal has `commits` durable commits of `c<i>` into one page.
fn crashed_with(commits: usize, file_limit: Option<u64>) -> (TempDir, Vec<u64>) {
    let (source, engine) = Run::start();
    if let Some(limit) = file_limit {
        engine.set_journal_file_limit(limit);
    }
    let pages = reserve(&engine, 1, "c0");
    let hold = engine.read().unwrap();
    for i in 1..=commits {
        commit_all(&engine, &pages, &format!("c{i}"), true);
    }
    let run = Run::finish(source, vec![]);
    drop(hold);
    (run.killed(), pages)
}

/// AC-J3 (J9): a damaged or incomplete last commit is discarded; a damaged commit with a durable one after it is
/// damage, and the opening touches nothing.
#[test]
fn ac_j3_a_damaged_tail_is_discarded_and_damage_before_a_durable_commit_is_reported() {
    for damage in ["truncate", "content", "length"] {
        let (folder, pages) = crashed_with(3, None);
        let file = journal_files(folder.path()).pop().unwrap();
        let last = *record_offsets(&file).last().unwrap();
        match damage {
            "truncate" => {
                let bytes = fs::read(&file).unwrap();
                fs::write(&file, &bytes[..last + 100]).unwrap();
            }
            "content" => flip(&file, last + 64 + 10),
            _ => flip(&file, last + 33),
        }
        let engine = open(folder.path());
        assert_eq!(texts(&engine, &pages)[0].as_deref(), Some("c2"), "{damage}");
    }
    for damage in ["content", "length"] {
        let (folder, _) = crashed_with(3, None);
        fs::create_dir(folder.path().join(TEMP_DIR)).unwrap();
        fs::write(
            folder.path().join(TEMP_DIR).join("scratch.0000000001"),
            b"left",
        )
        .unwrap();
        let file = journal_files(folder.path()).pop().unwrap();
        let offsets = record_offsets(&file);
        let middle = offsets[offsets.len() - 2];
        flip(
            &file,
            if damage == "content" {
                middle + 64 + 10
            } else {
                middle + 33
            },
        );
        let before = snapshot(folder.path());
        assert_eq!(
            open_err(folder.path()).kind(),
            ErrorKind::Damaged,
            "{damage}"
        );
        assert_eq!(snapshot(folder.path()), before, "{damage}");
    }
}

/// AC-J4 (J9): a missing journal file in the middle, or a reserved name of the wrong kind, is damage.
#[test]
fn ac_j4_a_missing_journal_file_or_a_reserved_name_of_the_wrong_kind_is_damage() {
    // Each commit changes its own page while a read holds them all back, so every file stays needed.
    let (source, engine) = Run::start();
    engine.set_journal_file_limit(4096);
    let pages = reserve(&engine, 6, "-");
    let hold = engine.read().unwrap();
    for (i, &page) in pages.iter().enumerate() {
        commit_all(&engine, &[page], &format!("c{i}"), true);
    }
    let run = Run::finish(source, vec![]);
    drop(hold);
    let folder = run.killed();
    let files = journal_files(folder.path());
    assert!(files.len() >= 3, "{files:?}");
    fs::remove_file(&files[1]).unwrap();
    let before = snapshot(folder.path());
    assert_eq!(open_err(folder.path()).kind(), ErrorKind::Damaged);
    assert_eq!(snapshot(folder.path()), before);

    let (folder, _) = crashed_with(2, None);
    fs::write(folder.path().join(TEMP_DIR), b"not a folder").unwrap();
    assert_eq!(open_err(folder.path()).kind(), ErrorKind::Damaged);

    let (folder, _) = crashed_with(2, None);
    fs::create_dir(folder.path().join(format!("{JOURNAL_PREFIX}{:019}", 99))).unwrap();
    let before = snapshot(folder.path());
    assert_eq!(open_err(folder.path()).kind(), ErrorKind::Damaged);
    assert_eq!(snapshot(folder.path()), before);
}

/// A store whose commits grew `muninn.data`, cut by a power failure that lost the growth.
fn crashed_after_growth(pages: usize) -> (TempDir, Vec<u64>) {
    let (source, engine) = Run::start();
    let reserved = reserve(&engine, pages, "grown");
    let run = Run::finish(source, vec![]);
    (run.crash(run.ops.len(), Model::Flushed), reserved)
}

/// AC-J5 (J8): a disk full during recovery fails the opening, keeps the journal, and the next opening with space
/// recovers everything; damage wins over the lack of space.
#[test]
fn ac_j5_disk_full_during_recovery() {
    let (folder, pages) = crashed_after_growth(40);
    let data_len = fs::metadata(folder.path().join(DATA_FILE)).unwrap().len();
    assert!(data_len < (pages[39] + 1) * 8192, "the growth was lost");
    testing::reset();
    testing::inject(
        Op::Reserve,
        0,
        Action::Fail(std::io::ErrorKind::StorageFull),
    );
    assert_eq!(open_err(folder.path()).kind(), ErrorKind::DiskFull);
    testing::reset();
    assert!(!journal_files(folder.path()).is_empty());
    let engine = open(folder.path());
    assert!(
        texts(&engine, &pages)
            .iter()
            .all(|t| t.as_deref() == Some("grown"))
    );
    drop(engine);

    let (folder, _) = crashed_with(3, None);
    let file = journal_files(folder.path()).pop().unwrap();
    let offsets = record_offsets(&file);
    flip(&file, offsets[offsets.len() - 2] + 64 + 10);
    testing::reset();
    testing::inject(
        Op::Reserve,
        0,
        Action::Fail(std::io::ErrorKind::StorageFull),
    );
    testing::inject(Op::Write, 0, Action::Fail(std::io::ErrorKind::StorageFull));
    assert_eq!(open_err(folder.path()).kind(), ErrorKind::Damaged);
    testing::reset();
}

/// AC-J10 (J8): recovery restores `muninn.data` even beyond the maximum size asked when opening.
#[test]
fn ac_j10_recovery_ignores_the_maximum_size() {
    let (folder, pages) = crashed_after_growth(200);
    let options = OpenOptions {
        max_size: Some(1 << 20),
    };
    let engine = Engine::open(folder.path(), &options).unwrap();
    assert!(engine.info().size_bytes > 1 << 20);
    assert!(
        texts(&engine, &pages)
            .iter()
            .all(|t| t.as_deref() == Some("grown"))
    );
    let mut tx = engine.write(None).unwrap();
    assert_eq!(tx.allocate().unwrap_err().kind(), ErrorKind::MaxSize);
}

/// AC-J12 (J8, P6): with either header copy damaged, recovery still brings back every commit.
#[test]
fn ac_j12_either_damaged_header_copy_still_recovers_everything() {
    for newest in [true, false] {
        let (source, engine) = Run::start();
        engine.set_journal_file_limit(4096);
        let pages = reserve(&engine, 1, "c0");
        for i in 1..=8 {
            commit_all(&engine, &pages, &format!("c{i}"), true);
        }
        let hold = engine.read().unwrap();
        for i in 9..=12 {
            commit_all(&engine, &pages, &format!("c{i}"), true);
        }
        let run = Run::finish(source, vec![]);
        drop(hold);
        let folder = run.killed();
        let data = folder.path().join(DATA_FILE);
        let bytes = fs::read(&data).unwrap();
        let copies = header_copies(&bytes);
        let revisions = copies.map(|c| c.unwrap().revision);
        assert!(
            revisions[0] != revisions[1],
            "the header was rewritten: {revisions:?}"
        );
        let pick = if newest {
            usize::from(revisions[1] > revisions[0])
        } else {
            usize::from(revisions[1] < revisions[0])
        };
        flip(&data, HEADER_COPY_OFFSETS[pick] as usize + 9);
        let engine = open(folder.path());
        assert_eq!(
            texts(&engine, &pages)[0].as_deref(),
            Some("c12"),
            "newest damaged: {newest}"
        );
    }
}

/// AC-J13 (J4): a durable commit is not visible before it is durable.
#[test]
fn ac_j13_a_durable_commit_is_invisible_until_it_is_durable() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let pages = reserve(&engine, 1, "before");
    let written = Arc::new(Barrier::new(2));
    let checked = Arc::new(Barrier::new(2));
    let (w, c) = (Arc::clone(&written), Arc::clone(&checked));
    engine.set_journal_sync_hook(Some(Arc::new(move || {
        w.wait();
        c.wait();
    })));
    std::thread::scope(|s| {
        let committer = s.spawn(|| commit_all(&engine, &pages, "after", true));
        written.wait();
        assert_eq!(texts(&engine, &pages)[0].as_deref(), Some("before"));
        checked.wait();
        committer.join().unwrap();
    });
    engine.set_journal_sync_hook(None);
    assert_eq!(texts(&engine, &pages)[0].as_deref(), Some("after"));
}

/// AC-J14 and AC11 (J12, G7): a commit that fails for lack of space, or is interrupted by a panic while it writes
/// the journal, never appears after a crash and does not keep the next commits from being recovered.
#[test]
fn ac_j14_a_failed_or_interrupted_commit_never_appears_and_blocks_nothing() {
    for failure in ["disk full", "panic"] {
        for model in POWER_CUTS.into_iter().chain([Model::Written]) {
            let (source, engine) = Run::start();
            let pages = reserve(&engine, 1, "first");
            let mut tx = engine.write(None).unwrap();
            put(&mut tx, pages[0], "failed");
            if failure == "disk full" {
                testing::inject(
                    Op::Write,
                    testing::count(Op::Write),
                    Action::Fail(std::io::ErrorKind::StorageFull),
                );
                assert_eq!(tx.commit().unwrap_err().kind(), ErrorKind::DiskFull);
            } else {
                testing::inject(Op::Sync, testing::count(Op::Sync), Action::Panic);
                assert!(catch_unwind(AssertUnwindSafe(|| tx.commit())).is_err());
            }
            assert_eq!(texts(&engine, &pages)[0].as_deref(), Some("first"));
            commit_all(&engine, &pages, "second", true);
            let run = Run::finish(source, vec![]);
            let folder = run.crash(run.ops.len(), model);
            let engine = open(folder.path());
            assert_eq!(
                texts(&engine, &pages)[0].as_deref(),
                Some("second"),
                "{failure} {model:?}"
            );
        }
    }
}

/// AC-J9 (J13): a transfer that fails outside any operation makes the store refuse writes; a commit without
/// changes still ends; the close reports the failure, and the next opening recovers every commit.
#[test]
fn ac_j9_a_failed_transfer_refuses_writes_and_recovery_brings_everything_back() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let pages = reserve(&engine, 1, "a");
    let released = Arc::new(Barrier::new(2));
    let taken = Arc::new(Barrier::new(2));
    let (r, t) = (Arc::clone(&released), Arc::clone(&taken));
    let once = Arc::new(std::sync::atomic::AtomicBool::new(true));
    engine.set_transfer_hook(Some(Arc::new(move || {
        if once.swap(false, std::sync::atomic::Ordering::SeqCst) {
            r.wait(); // the committer released the turn
            t.wait(); // the next writer has it
            // Fail the writes of this transfer (faults are per thread).
            for nth in 0..64 {
                testing::inject(
                    Op::Write,
                    testing::count(Op::Write) + nth,
                    Action::Fail(std::io::ErrorKind::Other),
                );
            }
        }
    })));
    std::thread::scope(|s| {
        let committer = s.spawn(|| commit_all(&engine, &pages, "b", true));
        released.wait();
        let empty = engine.write(None).unwrap();
        taken.wait();
        committer.join().unwrap();
        assert!(empty.commit().is_ok(), "a commit without changes ends");
    });
    engine.set_transfer_hook(None);
    assert_eq!(engine.write(None).err().unwrap().kind(), ErrorKind::Io);
    assert_eq!(texts(&engine, &pages)[0].as_deref(), Some("b"));
    testing::reset();
    for nth in 0..64 {
        testing::inject(Op::Write, nth, Action::Fail(std::io::ErrorKind::Other));
    }
    assert_eq!(engine.close().unwrap_err().kind(), ErrorKind::Io);
    testing::reset();
    let engine = open(tmp.path());
    assert_eq!(texts(&engine, &pages)[0].as_deref(), Some("b"));
}

/// AC-J16 (J2): a folder without a store that holds journal files or `muninn.temp` is not a store.
#[test]
fn ac_j16_journal_files_without_a_store_are_not_a_store() {
    for layout in ["journal", "journal and new", "temp"] {
        let tmp = tempfile::tempdir().unwrap();
        let journal = tmp.path().join(format!("{JOURNAL_PREFIX}{:019}", 1));
        match layout {
            "journal" => fs::write(&journal, b"x").unwrap(),
            "journal and new" => {
                fs::write(&journal, b"x").unwrap();
                fs::write(tmp.path().join("muninn.data.new"), b"y").unwrap();
            }
            _ => fs::create_dir(tmp.path().join(TEMP_DIR)).unwrap(),
        }
        let before = snapshot(tmp.path());
        assert_eq!(
            open_err(tmp.path()).kind(),
            ErrorKind::NotAStore,
            "{layout}"
        );
        assert_eq!(snapshot(tmp.path()), before, "{layout}");
    }
}

/// AC-J15 (second part): incompressible commits are journaled and recovered like any other.
#[test]
fn ac_j15_incompressible_commits_are_recovered() {
    let (source, engine) = Run::start();
    let mut tx = engine.write(None).unwrap();
    let mut x = 0x2545_F491_4F6C_DD1Du64;
    let mut expected = Vec::new();
    for _ in 0..20 {
        let page = tx.allocate().unwrap();
        let payload = tx.page_mut(page).unwrap();
        for b in payload.iter_mut() {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            *b = x as u8;
        }
        expected.push((page, payload.to_vec()));
    }
    tx.commit().unwrap();
    let run = Run::finish(source, vec![]);
    let folder = run.crash(run.ops.len(), Model::Flushed);
    let engine = open(folder.path());
    let read = engine.read().unwrap();
    for (page, bytes) in expected {
        assert_eq!(read.page(page).unwrap(), &bytes[..]);
    }
}

/// AC10 with the journal: altering any byte of a crashed store, its journal included, gives the damage class,
/// "not a store" (the mark), no difference, or, only inside the last journal record, the loss of that last commit
/// (J9); never data that was not written.
fn ac10_journal_sweep(step: usize) {
    let (source, engine) = Run::start();
    let pages = reserve(&engine, 2, "c0");
    let hold = engine.read().unwrap();
    for i in 1..=3 {
        commit_all(&engine, &pages, &format!("c{i}"), true);
    }
    let run = Run::finish(source, vec![]);
    drop(hold);
    let crashed = run.killed();
    // The sweep checks damage detection, not durability: its flushes to disk are skipped. Set after the run,
    // because recording a run resets this thread's test state.
    testing::skip_syncs(true);
    let files: Vec<PathBuf> = fs::read_dir(crashed.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    let journal = journal_files(crashed.path()).pop().unwrap();
    let last = *record_offsets(&journal).last().unwrap();
    // One folder for every attempt, refilled from memory: creating and deleting a folder per altered byte was most
    // of the sweep's time.
    let originals: Vec<(std::ffi::OsString, Vec<u8>)> = files
        .iter()
        .map(|f| (f.file_name().unwrap().to_owned(), fs::read(f).unwrap()))
        .collect();
    let work = tempfile::tempdir().unwrap();
    for (altered, file) in files.iter().enumerate() {
        let pristine = &originals[altered].1;
        for offset in (0..pristine.len()).step_by(step) {
            for entry in fs::read_dir(work.path()).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    fs::remove_dir_all(&path).unwrap();
                } else {
                    fs::remove_file(&path).unwrap();
                }
            }
            for (i, (name, bytes)) in originals.iter().enumerate() {
                if i == altered {
                    let mut bytes = bytes.clone();
                    bytes[offset] ^= 0xFF;
                    fs::write(work.path().join(name), &bytes).unwrap();
                } else {
                    fs::write(work.path().join(name), bytes).unwrap();
                }
            }
            let in_last = *file == journal && offset >= last;
            let at = format!("{} byte {offset}", file.display());
            let engine = match Engine::open(work.path(), &OpenOptions::default()) {
                Ok(engine) => engine,
                Err(error) => {
                    assert!(
                        matches!(error.kind(), ErrorKind::Damaged | ErrorKind::NotAStore),
                        "{at}: {error}"
                    );
                    continue;
                }
            };
            let read = engine.read().unwrap();
            for &page in &pages {
                match read.page(page) {
                    Ok(payload) => {
                        let seen = text(payload);
                        assert!(
                            seen == "c3" || (in_last && seen == "c2"),
                            "{at}: page {page} has {seen}"
                        );
                    }
                    Err(error) => assert_eq!(error.kind(), ErrorKind::Damaged, "{at}: {error}"),
                }
            }
        }
    }
}

#[test]
fn ac10_sample_of_altered_bytes_with_the_journal() {
    ac10_journal_sweep(997);
}

#[test]
#[ignore = "acceptance: alters every byte of a crashed store and its journal"]
fn ac10_every_altered_byte_with_the_journal() {
    ac10_journal_sweep(1);
}

/// AC12 (G12, J12): a failed flush of the journal fails the commit with the I/O class; later writes fail, reads
/// work, and reopening recovers every commit before it, with the failed one entire or absent.
#[test]
fn ac12_a_failed_journal_flush_refuses_writes_and_reopening_recovers() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let pages = reserve(&engine, 2, "kept");
    testing::reset();
    testing::inject(Op::Sync, 0, Action::Fail(std::io::ErrorKind::Other));
    let mut tx = engine.write(None).unwrap();
    for &page in &pages {
        put(&mut tx, page, "failed");
    }
    assert_eq!(tx.commit().unwrap_err().kind(), ErrorKind::Io);
    testing::reset();
    assert_eq!(engine.write(None).err().unwrap().kind(), ErrorKind::Io);
    assert_eq!(texts(&engine, &pages), vec![Some("kept".into()); 2]);
    drop(engine);
    let engine = open(tmp.path());
    let seen = texts(&engine, &pages);
    assert!(
        seen == vec![Some("kept".into()); 2] || seen == vec![Some("failed".into()); 2],
        "{seen:?}"
    );
}

const CHILD_ENV: &str = "MUNINN_TEST_CHILD_CRASH";

/// AC1 and AC3 with a real process: the child commits durably, opens another write with changes, and dies.
#[test]
fn ac1_ac3_a_killed_process_keeps_what_it_committed_and_nothing_else() {
    let tmp = tempfile::tempdir().unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "child_helper_commit_and_die",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_ENV, tmp.path())
        .status()
        .unwrap();
    assert!(!status.success(), "the child dies");
    let engine = open(tmp.path());
    let read = engine.read().unwrap();
    assert_eq!(text(read.page(3).unwrap()), "committed");
    assert_eq!(text(read.page(4).unwrap()), "committed");
}

/// Body of the child of [`ac1_ac3_a_killed_process_keeps_what_it_committed_and_nothing_else`].
#[test]
fn child_helper_commit_and_die() {
    let Some(folder) = std::env::var_os(CHILD_ENV) else {
        return;
    };
    let engine = open(Path::new(&folder));
    let mut tx = engine.write(None).unwrap();
    let pages = [tx.allocate().unwrap(), tx.allocate().unwrap()];
    assert_eq!(pages, [3, 4]);
    for page in pages {
        put(&mut tx, page, "committed");
    }
    tx.commit().unwrap();
    let mut tx = engine.write(None).unwrap();
    put(&mut tx, 3, "never committed");
    std::process::abort();
}

/// B1 of the verification: with a read held, a header rewrite and a deferred commit that supersedes durable ones,
/// a power cut must not lose the durable commits that returned (J4, J10).
#[test]
fn b1_a_deferred_commit_never_makes_returned_durable_ones_disappear() {
    for model in POWER_CUTS {
        let (source, engine) = Run::start();
        engine.set_journal_file_limit(4096);
        let pages = reserve(&engine, 1, "c0");
        let hold = engine.read().unwrap();
        commit_all(&engine, &pages, "c1", true);
        commit_all(&engine, &pages, "c2", true);
        commit_all(&engine, &pages, "c3", false);
        let run = Run::finish(source, vec![]);
        drop(hold);
        let folder = run.crash(run.ops.len(), model);
        let engine = open(folder.path());
        let seen = texts(&engine, &pages)[0].clone();
        assert!(
            matches!(seen.as_deref(), Some("c2" | "c3")),
            "{model:?}: {seen:?}"
        );
    }
}

/// B2 of the verification: a deferred commit that a journal rollover made durable is recorded as durable by the
/// next record, so damage in it is reported, not taken for an unfinished commit (J9).
#[test]
fn b2_damage_in_a_commit_made_durable_by_a_rollover_is_reported() {
    let (source, engine) = Run::start();
    engine.set_journal_file_limit(4096);
    let pages = reserve(&engine, 4, "-");
    let hold = engine.read().unwrap();
    commit_all(&engine, &pages[..1], "d0", false);
    commit_all(&engine, &pages[1..2], "last", true);
    let run = Run::finish(source, vec![]);
    drop(hold);
    let folder = run.killed();
    let files = journal_files(folder.path());
    let deferred_file = files
        .iter()
        .find(|f| {
            let bytes = fs::read(f).unwrap();
            record_offsets(f)
                .iter()
                .any(|&o| u64::from_le_bytes(bytes[o + 8..o + 16].try_into().unwrap()) == 2)
        })
        .unwrap();
    let bytes = fs::read(deferred_file).unwrap();
    let offset = record_offsets(deferred_file)
        .into_iter()
        .find(|&o| u64::from_le_bytes(bytes[o + 8..o + 16].try_into().unwrap()) == 2)
        .unwrap();
    flip(deferred_file, offset + 64 + 10);
    assert_eq!(open_err(folder.path()).kind(), ErrorKind::Damaged);
}

/// R1 of the review (J12, G12): a failed flush of the journal is the I/O class whatever its cause, also a lack of
/// space, and the store refuses writes, so no later commit can follow the record it left behind.
#[test]
fn r1_a_journal_flush_that_fails_for_lack_of_space_refuses_writes() {
    let (source, engine) = Run::start();
    let pages = reserve(&engine, 1, "kept");
    testing::inject(
        Op::Sync,
        testing::count(Op::Sync),
        Action::Fail(std::io::ErrorKind::StorageFull),
    );
    let mut tx = engine.write(None).unwrap();
    put(&mut tx, pages[0], "failed");
    assert_eq!(tx.commit().unwrap_err().kind(), ErrorKind::Io);
    assert_eq!(engine.write(None).err().unwrap().kind(), ErrorKind::Io);
    assert_eq!(engine.make_durable().unwrap_err().kind(), ErrorKind::Io);
    let run = Run::finish(source, vec![]);
    let folder = run.killed();
    let engine = open(folder.path());
    let seen = texts(&engine, &pages)[0].clone();
    assert!(
        matches!(seen.as_deref(), Some("kept" | "failed")),
        "{seen:?}"
    );
}

/// Review advisory: when the wipe of a failed append also fails, the record may be whole, so the store refuses
/// writes and no later commit can reuse its number.
#[test]
fn a_failed_wipe_after_a_failed_append_refuses_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let pages = reserve(&engine, 1, "kept");
    testing::reset();
    let writes = testing::count(Op::Write);
    testing::inject(
        Op::Write,
        writes,
        Action::Fail(std::io::ErrorKind::StorageFull),
    );
    testing::inject(
        Op::Write,
        writes + 1,
        Action::Fail(std::io::ErrorKind::Other),
    );
    let mut tx = engine.write(None).unwrap();
    put(&mut tx, pages[0], "failed");
    assert_eq!(tx.commit().unwrap_err().kind(), ErrorKind::DiskFull);
    testing::reset();
    assert_eq!(engine.write(None).err().unwrap().kind(), ErrorKind::Io);
}
