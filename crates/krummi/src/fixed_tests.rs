//! Fixed-size trees where the public tests cannot see without the engine's test hooks: where the values of `get_fixed`
//! and of the scans point (AC-X14), and a store that refuses writes (the T12 parts of AC-X17 and AC-X20).

use std::io;
use std::thread;

use krummi_pager::testing::{self, Action, Op};
use krummi_tx::Memory;

use super::*;

/// AC-X14 (X6, X7, AC20, T16): the values that `get_fixed` and the scans of a fixed-size tree of length 8 return are
/// read where the engine keeps them, never a copy: once transferred, `krummi.data` or the memory of a temporary store;
/// and the same memory comes back on each read. That a reference cannot outlive its transaction is in `tests/ui/trees`.
#[test]
fn ac_x14_reads_and_scans_of_a_fixed_size_tree_return_the_engine_s_memory() {
    let tmp = tempfile::tempdir().unwrap();
    for store in [
        Store::open(tmp.path(), &StoreOptions::default()).unwrap(),
        Store::open_temporary(&StoreOptions::default()).unwrap(),
    ] {
        let mut write = store.write().unwrap();
        let tree = write
            .open_fixed_tree_with("f", 8, OpenMode::CreateNew)
            .unwrap();
        for key in 0..1500u64 {
            write.set_fixed(&tree, key * 5, &key.to_le_bytes()).unwrap();
        }
        write.commit().unwrap();
        assert!(store.engine.transfer_now());

        let read = store.read().unwrap();
        let tree = read.open_fixed_tree("f", 8).unwrap();
        for key in [0, 5, 3000, 7495] {
            let value = read.get_fixed(&tree, key).unwrap().unwrap();
            assert_eq!(value, (key / 5).to_le_bytes());
            assert_eq!(store.engine.memory_of(value), Some(Memory::Data), "{key}");
            let again = read.get_fixed(&tree, key).unwrap().unwrap();
            assert_eq!(value.as_ptr(), again.as_ptr(), "no copy");
        }
        let values: Vec<(u64, &[u8])> = read
            .range_fixed(&tree, ..)
            .unwrap()
            .collect::<Result<_>>()
            .unwrap();
        assert_eq!(values.len(), 1500);
        for (key, value) in &values {
            assert_eq!(store.engine.memory_of(value), Some(Memory::Data), "{key}");
            let read_back = read.get_fixed(&tree, *key).unwrap().unwrap();
            assert_eq!(value.as_ptr(), read_back.as_ptr(), "no copy");
        }
        let (_, last) = read
            .range_fixed(&tree, ..)
            .unwrap()
            .next_back()
            .unwrap()
            .unwrap();
        assert_eq!(last.as_ptr(), values.last().unwrap().1.as_ptr(), "no copy");
        drop(read);
        store.close().unwrap();
    }
}

/// AC-X17 and AC-X20, their parts of a store that refuses writes (T12): once the store refuses writes, opening a
/// fixed-size tree with a length over 2,048, in every mode, or with a name of 1,025 bytes, and storing a value of
/// another length, in the open write, still fail with the invalid size, not with the I/O class; `tree_kind` gives every
/// kind in that write and in a read; and committing the write without changes ends without error.
#[test]
fn ac_x17_ac_x20_validation_and_tipo_in_a_store_that_refuses_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    // The transfer that fails is the one the test runs on this thread, whose faults it injects.
    store.engine.set_transfer_paused(true);
    let mut write = store.write().unwrap();
    let tree = write
        .open_fixed_tree_with("f", 8, OpenMode::CreateNew)
        .unwrap();
    write.set_fixed(&tree, 1, &[1; 8]).unwrap();
    write.open_tree_with("n", OpenMode::CreateNew).unwrap();
    write.commit().unwrap();

    let mut write = store.write().unwrap();
    let tree = write.open_fixed_tree("f", 8).unwrap();
    // As in the T12 test of the normal trees: a margin of failed writes over the first write of the transfer.
    for nth in 0..8 {
        testing::inject(Op::Write, nth, Action::Fail(io::ErrorKind::Other));
    }
    assert!(!store.engine.transfer_now(), "the transfer failed");
    testing::reset();
    let refused = thread::scope(|s| {
        s.spawn(|| {
            store
                .write_with_timeout(Duration::ZERO)
                .err()
                .map(|e| e.kind())
        })
        .join()
        .unwrap()
    });
    assert_eq!(refused, Some(ErrorKind::Io), "the store refuses writes");

    let size = |error: Error| match error {
        Error::InvalidSize {
            what,
            received,
            min,
            max,
            ..
        } => (what, received, min, max),
        other => panic!("expected invalid size, got {other}"),
    };
    let over = (SizeSubject::Value, 2049, 0, 2048);
    for name in ["f", "n", "absent"] {
        assert_eq!(size(write.open_fixed_tree(name, 2049).err().unwrap()), over);
        for mode in [
            OpenMode::CreateNew,
            OpenMode::OpenOrCreate,
            OpenMode::CreateOrEmpty,
            OpenMode::Empty,
        ] {
            let error = write.open_fixed_tree_with(name, 2049, mode).err().unwrap();
            assert_eq!(size(error), over, "{name} {mode:?}");
        }
    }
    let long = "x".repeat(MAX_NAME + 1);
    let error = write.open_fixed_tree(&long, 2049).err().unwrap();
    assert_eq!(size(error), (SizeSubject::TreeName, 1025, 1, 1024));
    let error = write.set_fixed(&tree, 2, &[2; 9]).unwrap_err();
    assert_eq!(size(error), (SizeSubject::Value, 9, 8, 8));
    let kinds = [
        ("absent", None),
        ("f", Some(TreeKind::FixedSize(8))),
        ("n", Some(TreeKind::Normal)),
    ];
    for (name, kind) in kinds {
        assert_eq!(write.tree_kind(name).unwrap(), kind, "{name}");
    }
    let read = store.read().unwrap();
    for (name, kind) in kinds {
        assert_eq!(read.tree_kind(name).unwrap(), kind, "{name}");
    }
    drop(read);
    write.commit().unwrap();

    store.close().unwrap();
    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    let read = store.read().unwrap();
    let tree = read.open_fixed_tree("f", 8).unwrap();
    assert_eq!(read.get_fixed(&tree, 1).unwrap(), Some(&[1; 8][..]));
    assert_eq!(read.get_fixed(&tree, 2).unwrap(), None);
}
