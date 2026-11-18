//! The sort that creates an index (AC-IX14 of the secondary indexes PRD, and AC-IX7 through its files): entries that
//! do not fit in the sort's memory, reduced with the test hook, go to run files in the subfolder `krummi.sort` of the
//! store's folder, which exists only while an index is created; the close, a drop and the next open remove what is
//! left; the failures of its files and of its memory fail the creation with their class (T11, T12) and change nothing;
//! a temporary store sorts in memory only (G9).

#[path = "../../krummi-tx/tests/common/mod.rs"]
mod common;
mod support;

use std::borrow::Cow;
use std::cell::Cell;
use std::fs;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use common::Run;
use krummi::table::testing::{StoredIndexEntry, with_sort_memory, with_sort_pause};
use krummi::table::value::{ColumnType, Value};
use krummi::{
    Column, Error, ErrorKind, IndexDefinition, IndexKey, IndexMode, Schema, Store, StoreInfo,
    StoreOptions, TableMode, TableSchema, WriteTable, WriteTransaction,
};
use krummi_pager::SORT_DIR;
use krummi_pager::testing::{self, Action, Op};
use support::open;

/// Rows of the table of [`commit_rows`].
const ROWS: i64 = 1500;
/// Memory of the reduced sort: about a dozen entries per run file, so that more than 64 runs are merged in groups
/// first.
const SMALL: usize = 1024;

/// `id i64 not null, n i32, s string`, primary key `id`.
fn schema() -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I32),
            Column::new("s", ColumnType::String),
        ],
        &["id"],
    )
}

/// Row `id`: `n` is `id % 97`, null for every 50th row, and `s` a distinct text for every row, out of the order of
/// `id`.
fn row(id: i64) -> Vec<(&'static str, Value<'static>)> {
    let n = match id % 50 {
        0 => Value::Null,
        _ => Value::I32((id % 97) as i32),
    };
    let s = format!("s{:05}", (id * 7919) % ROWS);
    vec![
        ("id", Value::I64(id)),
        ("n", n),
        ("s", Value::String(Cow::Owned(s))),
    ]
}

/// Commits the table `t` of [`schema`] with the rows 0 to [`ROWS`] of [`row`].
fn commit_rows(store: &Store) {
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&schema()))
        .unwrap();
    for id in 0..ROWS {
        write.insert_row(&table, &row(id)).unwrap();
    }
    write.commit().unwrap();
}

fn key(column: &str) -> IndexKey {
    IndexKey::new(column)
}

/// `ix_n`: not unique, on `n` (with nulls), with `s` included.
fn by_n() -> IndexDefinition {
    IndexDefinition::new("ix_n", vec![key("n")]).with_included(&["s"])
}

/// `ux_s`: unique, on `s` descending, nulls last.
fn by_s() -> IndexDefinition {
    IndexDefinition::new("ux_s", vec![key("s").descending().nulls_last()]).unique()
}

/// `ux_n`: unique on `n`, which the rows repeat.
fn unique_n() -> IndexDefinition {
    IndexDefinition::new("ux_n", vec![key("n")]).unique()
}

fn create(
    write: &mut WriteTransaction<'_>,
    table: &WriteTable<'_>,
    definition: &IndexDefinition,
) -> krummi::Result<()> {
    write.create_index(table, definition, IndexMode::CreateNew)
}

/// The entries of the index `name` of `t` that a new read sees, `None` without the index.
fn entries(store: &Store, name: &str) -> Option<Vec<StoredIndexEntry>> {
    let read = store.read().unwrap();
    read.stored_index_entries(&read.open_table("t").unwrap(), name)
        .unwrap()
}

/// What a test sees of the store: its information (P12) and the table's description.
fn state(store: &Store) -> (StoreInfo, TableSchema) {
    (
        store.info(),
        store.read().unwrap().describe_table("t").unwrap(),
    )
}

/// What a failed write leaves of the store besides the size it grew to: the pages in use and the table's
/// description.
fn kept(store: &Store) -> (u64, TableSchema) {
    let (info, schema) = state(store);
    (info.page_count - info.free_pages, schema)
}

fn sort_dir(folder: &Path) -> PathBuf {
    folder.join(SORT_DIR)
}

/// The table, primary key, index and index key of a duplicate key.
#[track_caller]
fn duplicate(error: Error) -> (String, String, Option<String>, Option<String>) {
    match error {
        Error::DuplicateKey {
            table,
            key,
            index,
            index_key,
            ..
        } => (
            table,
            key,
            index.map(String::from),
            index_key.map(String::from),
        ),
        other => panic!("expected a duplicate key, got {other}"),
    }
}

/// Runs `f` with the reduced sort memory and a pause that counts how often a sort wrote its first file and checks
/// that `krummi.sort` exists then; returns `f`'s result and that count.
fn spilled<R>(folder: Option<&Path>, f: impl FnOnce() -> R) -> (R, usize) {
    let pauses = Rc::new(Cell::new(0));
    let counted = Rc::clone(&pauses);
    let dir = folder.map(sort_dir);
    let result = with_sort_memory(SMALL, || {
        with_sort_pause(
            move || {
                counted.set(counted.get() + 1);
                let dir = dir.as_ref().expect("files only in a store on disk");
                assert!(dir.is_dir(), "krummi.sort exists while the sort runs");
                assert!(
                    fs::read_dir(dir).unwrap().next().is_some(),
                    "with its first file"
                );
            },
            f,
        )
    });
    (result, pauses.get())
}

/// AC-IX14: with the memory reduced, a non-unique and a unique index have the same entries as with all the memory and
/// as in a temporary store, which writes no file; with all the memory no file is written; and `krummi.sort` exists
/// while the sort runs and not after.
#[test]
fn ac_ix14_a_sort_in_files_gives_the_same_index_as_in_memory() {
    let memory_dir = tempfile::tempdir().unwrap();
    let memory = open(memory_dir.path());
    commit_rows(&memory);
    let mut write = memory.write().unwrap();
    let table = write.open_table("t").unwrap();
    with_sort_pause(
        || panic!("no file with all the memory"),
        || {
            create(&mut write, &table, &by_n()).unwrap();
            create(&mut write, &table, &by_s()).unwrap();
        },
    );
    write.commit().unwrap();
    assert!(!sort_dir(memory_dir.path()).exists());

    let files_dir = tempfile::tempdir().unwrap();
    let files = open(files_dir.path());
    commit_rows(&files);
    let mut write = files.write().unwrap();
    let table = write.open_table("t").unwrap();
    let (_, pauses) = spilled(Some(files_dir.path()), || {
        create(&mut write, &table, &by_n()).unwrap();
        assert!(!sort_dir(files_dir.path()).exists(), "removed on success");
        create(&mut write, &table, &by_s()).unwrap();
    });
    assert_eq!(pauses, 2, "each creation wrote files");
    assert!(!sort_dir(files_dir.path()).exists(), "removed on success");
    write.commit().unwrap();

    let temporary = Store::open_temporary(&StoreOptions::default()).unwrap();
    commit_rows(&temporary);
    let mut write = temporary.write().unwrap();
    let table = write.open_table("t").unwrap();
    let (_, pauses) = spilled(None, || {
        create(&mut write, &table, &by_n()).unwrap();
        create(&mut write, &table, &by_s()).unwrap();
    });
    assert_eq!(pauses, 0, "a temporary store writes no file");
    write.commit().unwrap();

    for name in ["ix_n", "ux_s"] {
        let expected = entries(&memory, name).unwrap();
        assert_eq!(expected.len(), ROWS as usize, "{name}");
        assert_eq!(entries(&files, name).unwrap(), expected, "{name}");
        assert_eq!(entries(&temporary, name).unwrap(), expected, "{name}");
    }
    // After reopening, too.
    files.close().unwrap();
    let files = open(files_dir.path());
    assert_eq!(entries(&files, "ix_n"), entries(&memory, "ix_n"));
}

/// Creates the table `u` with one row.
fn other_change(write: &mut WriteTransaction<'_>) {
    let table = write
        .open_table_with("u", TableMode::CreateNew(&schema()))
        .unwrap();
    write.insert_row(&table, &row(1)).unwrap();
}

/// AC-IX14 and AC-IX7 through the files: a unique index over rows that repeat its key fails with the same duplicate
/// as with all the memory, removes `krummi.sort`, and leaves the store as without the attempt while the write commits
/// its other changes (IX11); in a temporary store, too.
#[test]
fn ac_ix14_a_duplicate_is_found_the_same_in_files_and_leaves_no_effect() {
    // With all the memory.
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    commit_rows(&store);
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let expected = duplicate(create(&mut write, &table, &unique_n()).unwrap_err());
    assert_eq!(expected.2.as_deref(), Some("ux_n"));
    drop(write);

    for on_disk in [true, false] {
        let dirs = on_disk.then(|| [(); 2].map(|()| tempfile::tempdir().unwrap()));
        let folder = dirs.as_ref().map(|d| d[0].path());
        let [store, twin] = [0, 1].map(|i| match &dirs {
            Some(dirs) => open(dirs[i].path()),
            None => Store::open_temporary(&StoreOptions::default()).unwrap(),
        });
        for target in [&twin, &store] {
            commit_rows(target);
            let mut write = target.write().unwrap();
            let table = write.open_table("t").unwrap();
            create(&mut write, &table, &by_s()).unwrap();
            write.commit().unwrap();
        }
        // The twin's write commits the same other change without the attempt.
        let mut write = twin.write().unwrap();
        other_change(&mut write);
        write.commit().unwrap();

        let described = state(&store).1;
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let (error, pauses) = spilled(folder, || create(&mut write, &table, &unique_n()));
        assert_eq!(duplicate(error.unwrap_err()), expected);
        assert_eq!(pauses, usize::from(on_disk));
        if let Some(folder) = folder {
            assert!(!sort_dir(folder).exists(), "removed on failure");
        }
        other_change(&mut write);
        write.commit().unwrap();
        assert_eq!(entries(&store, "ux_n"), None);
        assert_eq!(store.info(), twin.info(), "on disk: {on_disk}");
        assert_eq!(state(&store).1, described, "on disk: {on_disk}");
        assert_eq!(entries(&store, "ux_s"), entries(&twin, "ux_s"));
    }
}

/// AC-IX14: a panic that interrupts the creation while its files exist removes `krummi.sort`, and the index does not
/// exist.
#[test]
fn ac_ix14_a_panic_during_the_sort_removes_its_files() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    commit_rows(&store);
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let panicked = catch_unwind(AssertUnwindSafe(|| {
        with_sort_memory(SMALL, || {
            with_sort_pause(
                || panic!("interrupted"),
                || create(&mut write, &table, &by_n()),
            )
        })
    }));
    assert!(panicked.is_err());
    assert!(!sort_dir(dir.path()).exists());
    drop(write);
    assert_eq!(entries(&store, "ix_n"), None);
}

/// Copies the files of the folder `from` into the new folder `to`.
fn copy_files(from: &Path, to: &Path) {
    fs::create_dir(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        fs::copy(&path, to.join(path.file_name().unwrap())).unwrap();
    }
}

/// The folder a crash of the process leaves while an index is created and its files exist: what the store's folder
/// had (the recorded operations of the run), with the `krummi.sort` of that moment, a foreign file in it and a foreign
/// file `notes.txt` next to the store.
fn crashed_during_a_sort() -> tempfile::TempDir {
    let source = Run::begin();
    let store = open(source.path());
    commit_rows(&store);
    let saved = tempfile::tempdir().unwrap();
    let kept = saved.path().join(SORT_DIR);
    let live = sort_dir(source.path());
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let crashed = catch_unwind(AssertUnwindSafe(|| {
        with_sort_memory(SMALL, || {
            with_sort_pause(
                move || {
                    copy_files(&live, &kept);
                    panic!("the process dies here");
                },
                || create(&mut write, &table, &by_n()),
            )
        })
    }));
    assert!(crashed.is_err());
    drop(write);
    let run = Run::finish(source, Vec::new());
    drop(store);
    let killed = run.killed();
    copy_files(&saved.path().join(SORT_DIR), &sort_dir(killed.path()));
    fs::write(sort_dir(killed.path()).join("foreign"), b"not ours").unwrap();
    fs::write(killed.path().join("notes.txt"), b"the user's").unwrap();
    killed
}

/// AC-IX14 (J8): after a crash during the sort, the next open removes `krummi.sort` whole, also with a foreign file in
/// it and also when it is a file; the index does not exist and the foreign files of the folder stay (P7).
#[test]
fn ac_ix14_the_open_after_a_crash_removes_the_files_of_the_sort() {
    let folder = crashed_during_a_sort();
    assert!(sort_dir(folder.path()).join("run.0000000001").is_file());
    let store = open(folder.path());
    assert!(!sort_dir(folder.path()).exists());
    assert_eq!(
        fs::read(folder.path().join("notes.txt")).unwrap(),
        b"the user's"
    );
    assert_eq!(entries(&store, "ix_n"), None);
    let read = store.read().unwrap();
    assert_eq!(
        read.row_count(&read.open_table("t").unwrap()).unwrap(),
        ROWS as u64
    );
    drop(read);
    store.close().unwrap();

    // `krummi.sort` as a file.
    let store = open(folder.path());
    store.close().unwrap();
    fs::write(sort_dir(folder.path()), b"a file").unwrap();
    let store = open(folder.path());
    assert!(!sort_dir(folder.path()).exists());
    store.close().unwrap();
}

/// AC-IX14 (J2, J8): an open that fails leaves `krummi.sort`; one that cannot remove it fails with the access class
/// and leaves it; and a folder with `krummi.sort` and no `krummi.data` is not a store and stays as it is.
#[test]
fn ac_ix14_a_failed_open_leaves_the_files_of_the_sort() {
    let dir = tempfile::tempdir().unwrap();
    open(dir.path()).close().unwrap();
    let sort = sort_dir(dir.path());
    fs::create_dir(&sort).unwrap();
    fs::write(sort.join("run.0000000001"), b"left").unwrap();

    // A `krummi.temp` that is a file is damage (J9): the open fails before removing anything.
    let temp = dir.path().join(krummi_pager::TEMP_DIR);
    fs::write(&temp, b"not a folder").unwrap();
    let error = Store::open(dir.path(), &StoreOptions::default())
        .err()
        .unwrap();
    assert_eq!(error.kind(), ErrorKind::Damaged);
    assert!(sort.join("run.0000000001").is_file());
    fs::remove_file(&temp).unwrap();

    // An open that cannot remove it.
    testing::reset();
    testing::inject(
        Op::SortRemove,
        0,
        Action::Fail(io::ErrorKind::PermissionDenied),
    );
    let error = Store::open(dir.path(), &StoreOptions::default())
        .err()
        .unwrap();
    testing::reset();
    assert_eq!(error.kind(), ErrorKind::Access);
    assert!(sort.join("run.0000000001").is_file());

    // The next one removes it.
    open(dir.path()).close().unwrap();
    assert!(!sort.exists());

    // Not a store.
    let other = tempfile::tempdir().unwrap();
    fs::create_dir(sort_dir(other.path())).unwrap();
    let error = Store::open(other.path(), &StoreOptions::default())
        .err()
        .unwrap();
    assert_eq!(error.kind(), ErrorKind::NotAStore);
    let names: Vec<_> = fs::read_dir(other.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, [SORT_DIR]);
}

/// AC-IX14: a creation that cannot remove `krummi.sort` keeps its result; closing the store removes it, and so does
/// dropping it without closing.
#[test]
fn ac_ix14_files_the_creation_cannot_remove_go_with_the_close_or_a_drop() {
    for close in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let store = open(dir.path());
        commit_rows(&store);
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        testing::reset();
        testing::inject(
            Op::SortRemove,
            0,
            Action::Fail(io::ErrorKind::PermissionDenied),
        );
        let (created, pauses) = spilled(Some(dir.path()), || create(&mut write, &table, &by_n()));
        testing::reset();
        created.unwrap();
        assert_eq!(pauses, 1);
        assert!(sort_dir(dir.path()).is_dir(), "left by the creation");
        write.commit().unwrap();
        assert_eq!(entries(&store, "ix_n").unwrap().len(), ROWS as usize);
        if close {
            store.close().unwrap();
        } else {
            drop(store);
        }
        assert!(!sort_dir(dir.path()).exists(), "close: {close}");
    }
}

/// A store on disk with the rows of [`commit_rows`], with its information and description.
fn committed() -> (tempfile::TempDir, Store, (StoreInfo, TableSchema)) {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    commit_rows(&store);
    let before = state(&store);
    (dir, store, before)
}

/// AC-IX14 (IX2, IX11, T11, T12): a failure creating, writing or reading the files of the sort fails the creation with
/// its class (access, disk full of G7, out of memory of P13, I/O of G12), the write cannot commit and fails with it,
/// `krummi.sort` is removed and the store is as before; an I/O failure writing them makes the store refuse writes until
/// it is reopened.
#[test]
fn ac_ix14_failures_of_the_files_of_the_sort_fail_the_creation_with_their_class() {
    use io::ErrorKind as Io;
    let cases = [
        (Op::SortCreate, 0, Io::PermissionDenied, ErrorKind::Access),
        (Op::SortCreate, 1, Io::StorageFull, ErrorKind::DiskFull),
        (Op::SortCreate, 3, Io::Other, ErrorKind::Io),
        (Op::SortWrite, 0, Io::StorageFull, ErrorKind::DiskFull),
        (Op::SortWrite, 2, Io::OutOfMemory, ErrorKind::OutOfMemory),
        (Op::SortWrite, 1, Io::PermissionDenied, ErrorKind::Access),
        (Op::SortWrite, 3, Io::Other, ErrorKind::Io),
        (Op::SortRead, 0, Io::PermissionDenied, ErrorKind::Access),
        (Op::SortRead, 1, Io::Other, ErrorKind::Io),
        (Op::SortRead, 70, Io::Other, ErrorKind::Io),
    ];
    for (op, nth, kind, class) in cases {
        let case = format!("{op:?} {nth} {kind:?}");
        let (dir, store, before) = committed();
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        testing::reset();
        testing::inject(op, nth, Action::Fail(kind));
        let (error, _) = spilled(Some(dir.path()), || create(&mut write, &table, &by_n()));
        let reached = testing::count(op) > nth;
        testing::reset();
        assert!(reached, "{case}: the fault point was reached");
        assert_eq!(error.unwrap_err().kind(), class, "{case}");
        assert!(!sort_dir(dir.path()).exists(), "{case}");
        assert_eq!(write.commit().unwrap_err().kind(), class, "{case}: T11");
        assert_eq!(state(&store), before, "{case}");
        assert_eq!(entries(&store, "ix_n"), None, "{case}");
        let refused = op == Op::SortWrite && class == ErrorKind::Io;
        match store.write() {
            Ok(write) => {
                assert!(!refused, "{case}: T12 refuses writes");
                drop(write);
            }
            Err(error) => {
                assert!(refused, "{case}: {error}");
                assert_eq!(error.kind(), ErrorKind::Io, "{case}");
            }
        }
        if refused {
            drop(store);
            let store = open(dir.path());
            let mut write = store.write().unwrap();
            let table = write.open_table("t").unwrap();
            create(&mut write, &table, &by_n()).unwrap();
            write.commit().unwrap();
        }
    }
}

/// AC-IX14 (G9, AC14 of the root): a temporary store with the reduced memory touches no file or folder, and fails with
/// out of memory, with T11 and no change, when the system does not give the memory of the sort (P13).
#[test]
fn ac_ix14_a_temporary_store_sorts_in_memory_only() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    commit_rows(&store);
    testing::reset();
    let file_ops = [
        Op::CreateFile,
        Op::Write,
        Op::Reserve,
        Op::Sync,
        Op::Rename,
        Op::Remove,
        Op::SyncDir,
        Op::Map,
        Op::Lock,
        Op::Read,
        Op::Temp,
        Op::SortCreate,
        Op::SortWrite,
        Op::SortRead,
        Op::SortRemove,
    ];
    for op in file_ops {
        testing::inject(op, 0, Action::Fail(io::ErrorKind::Other));
    }
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let (created, pauses) = spilled(None, || create(&mut write, &table, &by_n()));
    created.unwrap();
    write.commit().unwrap();
    assert_eq!(pauses, 0);
    for op in file_ops {
        assert_eq!(testing::count(op), 0, "{op:?}");
    }
    testing::reset();
    assert_eq!(entries(&store, "ix_n").unwrap().len(), ROWS as usize);

    // Every reservation of the creation, failing in turn.
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    commit_rows(&store);
    let before = kept(&store);
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    testing::reset();
    with_sort_memory(SMALL, || create(&mut write, &table, &by_n())).unwrap();
    let reservations = testing::count(Op::Alloc);
    testing::reset();
    drop(write);
    assert!(reservations > ROWS as usize / 2, "{reservations}");
    let mut failed = 0;
    for nth in (0..reservations).step_by(reservations / 40) {
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        testing::reset();
        testing::inject(Op::Alloc, nth, Action::Fail(io::ErrorKind::OutOfMemory));
        let created = with_sort_memory(SMALL, || create(&mut write, &table, &by_n()));
        testing::reset();
        if let Err(error) = created {
            failed += 1;
            assert_eq!(error.kind(), ErrorKind::OutOfMemory, "{nth}");
            assert_eq!(
                write.commit().unwrap_err().kind(),
                ErrorKind::OutOfMemory,
                "{nth}: T11"
            );
            // A temporary store may keep the memory the failed write grew it by (P11).
            assert_eq!(kept(&store), before, "{nth}");
        }
    }
    assert!(failed > 30, "{failed}");
}
