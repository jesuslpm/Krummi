//! Opening, creating, locking and closing stores (Pager PRD: P1–P9, P12, P14; AC-P1–AC-P8; parent AC8, AC9).

mod common;

use std::fs;
use std::sync::{Arc, Barrier};

use common::{child_open_and_hold, damage_byte, file_bytes, folder_snapshot, open, open_err};
use krummi_pager::format::{FORMAT_VERSION, MARK, PAGE_SIZE};
use krummi_pager::{DATA_FILE, ErrorKind, NEW_DATA_FILE, OpenOptions, Pager};

const HEADER_A: usize = 64;
const HEADER_B: usize = PAGE_SIZE;

#[test]
fn ac_p1_a_missing_folder_is_created_with_a_new_store() {
    let tmp = tempfile::tempdir().unwrap();
    let folder = tmp.path().join("a").join("b").join("store");
    let pager = open(&folder);
    let info = pager.info();
    assert_eq!(info.format_version, FORMAT_VERSION);
    assert_eq!(info.format_version, 5);
    assert_eq!(info.page_size, 8192);
    assert!(
        info.size_bytes <= 1_048_576 && info.size_bytes.is_multiple_of(8192),
        "{info:?}"
    );
    assert_eq!(info.page_count, info.size_bytes / 8192);
    assert_eq!(info.max_size, None);
    assert!(!info.temporary);
    pager.close().unwrap();
    assert_eq!(
        folder_snapshot(&folder).keys().cloned().collect::<Vec<_>>(),
        vec![DATA_FILE.to_owned()]
    );
    assert_eq!(
        fs::metadata(folder.join(DATA_FILE)).unwrap().len(),
        info.size_bytes
    );
    assert_eq!(&file_bytes(&folder.join(DATA_FILE))[..16], &MARK);
}

#[test]
fn ac_p2_an_empty_folder_and_a_folder_with_only_the_leftover_get_a_new_store() {
    let tmp = tempfile::tempdir().unwrap();
    let empty = tmp.path().join("empty");
    fs::create_dir(&empty).unwrap();
    let leftover = tmp.path().join("leftover");
    fs::create_dir(&leftover).unwrap();
    fs::write(leftover.join(NEW_DATA_FILE), b"half-written store").unwrap();
    for folder in [&empty, &leftover] {
        open(folder).close().unwrap();
        assert_eq!(
            folder_snapshot(folder).keys().cloned().collect::<Vec<_>>(),
            vec![DATA_FILE.to_owned()]
        );
        open(folder).close().unwrap();
    }
}

#[test]
fn ac_p3_a_folder_without_a_store_but_with_other_entries_is_not_a_store() {
    let tmp = tempfile::tempdir().unwrap();
    let foreign = tmp.path().join("foreign");
    fs::create_dir(&foreign).unwrap();
    fs::write(foreign.join(".gitkeep"), b"").unwrap();
    let leftover_and_foreign = tmp.path().join("leftover_and_foreign");
    fs::create_dir(&leftover_and_foreign).unwrap();
    fs::write(leftover_and_foreign.join(NEW_DATA_FILE), b"x").unwrap();
    fs::write(leftover_and_foreign.join("notes.txt"), b"y").unwrap();
    let leftover_dir = tmp.path().join("leftover_dir");
    fs::create_dir_all(leftover_dir.join(NEW_DATA_FILE)).unwrap();
    for folder in [&foreign, &leftover_and_foreign, &leftover_dir] {
        let before = folder_snapshot(folder);
        assert_eq!(
            open_err(folder).kind(),
            ErrorKind::NotAStore,
            "{}",
            folder.display()
        );
        assert_eq!(folder_snapshot(folder), before);
    }
}

#[test]
fn ac_p4_a_closed_store_reopens_with_the_same_information() {
    let tmp = tempfile::tempdir().unwrap();
    let pager = open(tmp.path());
    let info = pager.info();
    pager.close().unwrap();
    let pager = open(tmp.path());
    assert_eq!(pager.info(), info);
}

#[test]
fn ac_p5_dropping_a_store_without_closing_it_releases_the_lock() {
    let tmp = tempfile::tempdir().unwrap();
    drop(open(tmp.path()));
    open(tmp.path()).close().unwrap();
}

#[test]
fn ac_p5_a_killed_process_leaves_the_store_unlocked() {
    let tmp = tempfile::tempdir().unwrap();
    open(tmp.path()).close().unwrap();
    let mut child = child_open_and_hold(tmp.path());
    assert_eq!(
        open_err(tmp.path()).kind(),
        ErrorKind::StoreInUse,
        "the child holds the store"
    );
    child.kill().unwrap();
    child.wait().unwrap();
    open(tmp.path()).close().unwrap();
}

#[test]
fn ac8_a_store_open_in_this_process_or_another_one_cannot_be_opened_again() {
    let tmp = tempfile::tempdir().unwrap();
    let pager = open(tmp.path());
    assert_eq!(open_err(tmp.path()).kind(), ErrorKind::StoreInUse);
    pager.close().unwrap();
    let mut child = child_open_and_hold(tmp.path());
    assert_eq!(open_err(tmp.path()).kind(), ErrorKind::StoreInUse);
    child.kill().unwrap();
    child.wait().unwrap();
}

#[test]
fn ac_p6_concurrent_openers_of_a_missing_folder_get_one_store() {
    for round in 0..20 {
        let tmp = tempfile::tempdir().unwrap();
        let folder = tmp.path().join(format!("store{round}"));
        let barrier = Arc::new(Barrier::new(4));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let (folder, barrier) = (folder.clone(), barrier.clone());
                std::thread::spawn(move || {
                    barrier.wait();
                    let result = Pager::open(&folder, &OpenOptions::default());
                    let outcome = result.as_ref().map(|_| ()).map_err(|e| e.kind());
                    // Keep every opened store until all threads have tried.
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    outcome
                })
            })
            .collect();
        let outcomes: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(
            outcomes.iter().filter(|o| o.is_ok()).count(),
            1,
            "{outcomes:?}"
        );
        assert!(
            outcomes
                .iter()
                .all(|o| *o == Ok(()) || *o == Err(ErrorKind::StoreInUse)),
            "{outcomes:?}"
        );
        assert_eq!(
            folder_snapshot(&folder).keys().cloned().collect::<Vec<_>>(),
            vec![DATA_FILE.to_owned()]
        );
    }
}

#[test]
fn ac_p6_another_path_to_the_same_store_is_the_same_store() {
    let tmp = tempfile::tempdir().unwrap();
    let folder = tmp.path().join("store");
    let pager = open(&folder);
    let other = tmp.path().join("store").join("..").join("store");
    assert_eq!(open_err(&other).kind(), ErrorKind::StoreInUse);
    if cfg!(windows) {
        let upper = tmp.path().join("STORE");
        assert_eq!(open_err(&upper).kind(), ErrorKind::StoreInUse);
    }
    pager.close().unwrap();
}

#[test]
fn ac_p7_foreign_files_in_a_store_folder_are_left_alone() {
    let tmp = tempfile::tempdir().unwrap();
    open(tmp.path()).close().unwrap();
    fs::write(tmp.path().join("notes.txt"), b"keep me").unwrap();
    fs::create_dir(tmp.path().join("backups")).unwrap();
    fs::write(tmp.path().join("backups").join("old.bin"), b"\x00\x01").unwrap();
    fs::write(tmp.path().join(NEW_DATA_FILE), b"not ours to delete").unwrap();
    let mut before = folder_snapshot(tmp.path());
    before.remove(DATA_FILE);
    let pager = open(tmp.path());
    pager.close().unwrap();
    let mut after = folder_snapshot(tmp.path());
    after.remove(DATA_FILE);
    assert_eq!(after, before);
}

#[test]
fn ac_p8_one_damaged_header_copy_opens_and_two_are_damage() {
    let tmp = tempfile::tempdir().unwrap();
    let pager = open(tmp.path());
    let info = pager.info();
    pager.close().unwrap();
    let data = tmp.path().join(DATA_FILE);
    let pristine = file_bytes(&data);
    for copy in [HEADER_A, HEADER_B] {
        fs::write(&data, &pristine).unwrap();
        damage_byte(&data, copy + 3);
        let pager = open(tmp.path());
        assert_eq!(pager.info(), info);
        pager.close().unwrap();
    }
    fs::write(&data, &pristine).unwrap();
    damage_byte(&data, HEADER_A + 3);
    damage_byte(&data, HEADER_B + 3);
    let before = file_bytes(&data);
    let err = open_err(tmp.path());
    assert_eq!(err.kind(), ErrorKind::Damaged);
    assert!(
        matches!(err, krummi_pager::Error::Damaged { page: None, .. }),
        "{err:?}"
    );
    assert_eq!(file_bytes(&data), before);
}

/// AC-F17 (F11 of the free-space PRD; earlier AC-E14): format `4`, the one before the free-space map, is a
/// previous format; a new store has format `5`.
#[test]
fn ac_f17_format_4_is_a_previous_format_and_new_stores_have_format_5() {
    let tmp = tempfile::tempdir().unwrap();
    let pager = open(tmp.path());
    assert_eq!(pager.info().format_version, 5);
    pager.close().unwrap();
    let data = tmp.path().join(DATA_FILE);
    common::rewrite_format_version(&data, 4);
    let before = file_bytes(&data);
    match open_err(tmp.path()) {
        krummi_pager::Error::DifferentFormat {
            found, expected, ..
        } => assert_eq!((found, expected), (4, 5)),
        other => panic!("expected a different format, got {other:?}"),
    }
    assert_eq!(file_bytes(&data), before);
}

#[test]
fn ac9_other_format_versions_are_refused_without_touching_the_store() {
    let tmp = tempfile::tempdir().unwrap();
    open(tmp.path()).close().unwrap();
    let data = tmp.path().join(DATA_FILE);
    for version in [0, 2, 4, FORMAT_VERSION + 1] {
        common::rewrite_format_version(&data, version);
        let before = file_bytes(&data);
        match open_err(tmp.path()) {
            krummi_pager::Error::DifferentFormat {
                found, expected, ..
            } => {
                assert_eq!((found, expected), (version, FORMAT_VERSION));
            }
            other => panic!("expected a different format, got {other:?}"),
        }
        assert_eq!(file_bytes(&data), before);
    }
}

#[test]
fn a_data_file_without_the_mark_is_not_a_store() {
    let tmp = tempfile::tempdir().unwrap();
    let cases: [(&str, &[u8]); 3] = [
        ("empty", b""),
        ("short", &MARK[..8]),
        ("text", b"hello, this is not a store at all"),
    ];
    for (name, content) in cases {
        let folder = tmp.path().join(name);
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join(DATA_FILE), content).unwrap();
        let before = folder_snapshot(&folder);
        assert_eq!(open_err(&folder).kind(), ErrorKind::NotAStore, "{name}");
        assert_eq!(folder_snapshot(&folder), before);
    }
    let folder = tmp.path().join("dir");
    fs::create_dir_all(folder.join(DATA_FILE)).unwrap();
    assert_eq!(open_err(&folder).kind(), ErrorKind::NotAStore);
}

#[test]
fn ac_p13_the_mark_without_complete_header_pages_is_damage() {
    let tmp = tempfile::tempdir().unwrap();
    open(tmp.path()).close().unwrap();
    let data = tmp.path().join(DATA_FILE);
    let bytes = file_bytes(&data);
    fs::write(&data, &bytes[..PAGE_SIZE + 100]).unwrap();
    assert_eq!(open_err(tmp.path()).kind(), ErrorKind::Damaged);
}

#[test]
fn a_path_that_is_a_file_cannot_be_used() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("file");
    fs::write(&file, b"x").unwrap();
    assert_eq!(open_err(&file).kind(), ErrorKind::Access);
    assert_eq!(fs::read(&file).unwrap(), b"x");
}

#[test]
fn ac_p11_a_maximum_size_below_one_mib_is_refused_before_touching_anything() {
    let tmp = tempfile::tempdir().unwrap();
    let folder = tmp.path().join("missing");
    let options = OpenOptions {
        max_size: Some(1_048_575),
    };
    let err = Pager::open(&folder, &options).err().unwrap();
    assert_eq!(err.kind(), ErrorKind::InvalidOptions);
    assert!(
        !folder.exists(),
        "invalid options are checked before the folder is created"
    );
    let err = Pager::open_temporary(&options).err().unwrap();
    assert_eq!(err.kind(), ErrorKind::InvalidOptions);
}

#[test]
fn ac_p11_the_maximum_size_is_rounded_down_to_the_page_size() {
    let tmp = tempfile::tempdir().unwrap();
    let pager = Pager::open(
        tmp.path(),
        &OpenOptions {
            max_size: Some(1_048_576 + 5000),
        },
    )
    .unwrap();
    assert_eq!(pager.info().max_size, Some(1_048_576));
}

/// Helper run in a child process by the tests above: opens the store named by the environment and holds it.
#[test]
fn child_helper_open_and_hold() {
    common::run_child_helper();
}

/// The data file is named exactly `krummi.data` whatever the length of the folder path (the Windows rename
/// once read the target name past its end when no padding followed it).
#[test]
fn the_data_file_name_does_not_depend_on_the_path_length() {
    let tmp = tempfile::tempdir().unwrap();
    for length in 1..=12 {
        let folder = tmp.path().join("s".repeat(length));
        open(&folder).close().unwrap();
        assert_eq!(
            folder_snapshot(&folder).keys().cloned().collect::<Vec<_>>(),
            vec![DATA_FILE.to_owned()],
            "folder name of {length} characters"
        );
    }
}

/// AC-J6: a store written by modules 1 and 2, with the header layout of format 1, is of an older format.
#[test]
fn ac_j6_a_store_of_format_1_is_refused_as_older_without_touching_it() {
    let tmp = tempfile::tempdir().unwrap();
    open(tmp.path()).close().unwrap();
    let data = tmp.path().join(DATA_FILE);
    let mut bytes = file_bytes(&data);
    let v1 = krummi_pager::format::encode_v1(1, 1);
    for offset in krummi_pager::format::HEADER_COPY_OFFSETS {
        let offset = offset as usize;
        bytes[offset..offset + krummi_pager::format::RAW_HEADER_SIZE].fill(0);
        bytes[offset..offset + v1.len()].copy_from_slice(&v1);
    }
    std::fs::write(&data, &bytes).unwrap();
    match open_err(tmp.path()) {
        krummi_pager::Error::DifferentFormat {
            found, expected, ..
        } => assert_eq!((found, expected), (1, FORMAT_VERSION)),
        other => panic!("expected a different format, got {other:?}"),
    }
    assert_eq!(file_bytes(&data), bytes);
}

/// J6: each header write replaces the copy that is not the current one, so a damaged current copy leaves the
/// previous header (P6).
#[test]
fn header_writes_alternate_between_the_two_copies() {
    use krummi_pager::format::{HEADER_COPY_OFFSETS, Header, header_copies};
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join(DATA_FILE);
    let pager = open(tmp.path());
    for applied in 1..=3 {
        pager
            .write_header(Header {
                applied_txid: applied,
                journal_first: applied + 1,
                ..pager.header()
            })
            .unwrap();
    }
    pager.close().unwrap();
    let copies = header_copies(&file_bytes(&data));
    let applied: Vec<u64> = copies.iter().map(|c| c.unwrap().applied_txid).collect();
    assert!(applied.contains(&3) && applied.contains(&2), "{applied:?}");
    let pager = open(tmp.path());
    assert_eq!(pager.header().applied_txid, 3);
    assert_eq!(pager.header().revision, 4);
    pager.close().unwrap();
    let newest = copies
        .iter()
        .position(|c| c.unwrap().applied_txid == 3)
        .unwrap();
    damage_byte(&data, HEADER_COPY_OFFSETS[newest] as usize + 9);
    let pager = open(tmp.path());
    assert_eq!(pager.header().applied_txid, 2);
    pager.close().unwrap();
}
