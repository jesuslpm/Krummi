//! Pages, growth, maximum size, temporary mode and zero-copy reads (Pager PRD: P10–P13, P15; AC-P10–AC-P14,
//! AC-P17; parent AC10, AC14, AC20), and the value runs of the large values PRD in the Pager (its part of AC-V3 and
//! AC-V11: contiguous reads without a copy, also across a growth in temporary mode, and damage).

mod common;

use std::fs;
use std::process::Command;

use common::{file_bytes, open};
use krummi_pager::format::{
    HEADER_PAGES, MARK, PAGE_HEADER_SIZE, PAGE_PAYLOAD_SIZE, PAGE_SIZE, seal_run,
};
use krummi_pager::{DATA_FILE, Error, ErrorKind, INITIAL_SIZE, OpenOptions, Pager};

const PAGE: u64 = PAGE_SIZE as u64;

fn payload(page: u64) -> Vec<u8> {
    (0..PAGE_PAYLOAD_SIZE)
        .map(|i| (i as u64 * 31 + page * 7) as u8)
        .collect()
}

/// Grows the store if needed and writes the test payload of `page`.
fn write(pager: &mut Pager, page: u64) -> Result<(), Error> {
    pager.ensure_pages(page + 1)?;
    pager.write_page(page, &payload(page))
}

fn write_byte(file: &fs::File, offset: usize, value: u8) {
    #[cfg(unix)]
    std::os::unix::fs::FileExt::write_all_at(file, &[value], offset as u64).unwrap();
    #[cfg(windows)]
    assert_eq!(
        std::os::windows::fs::FileExt::seek_write(file, &[value], offset as u64).unwrap(),
        1
    );
}

fn with_max(max_size: u64) -> OpenOptions {
    OpenOptions {
        max_size: Some(max_size),
    }
}

#[test]
fn written_pages_are_read_back_after_reopening() {
    let tmp = tempfile::tempdir().unwrap();
    let mut pager = open(tmp.path());
    for page in HEADER_PAGES..40 {
        write(&mut pager, page).unwrap();
    }
    pager.close().unwrap();
    let pager = open(tmp.path());
    for page in HEADER_PAGES..40 {
        assert_eq!(pager.page(page).unwrap(), &payload(page)[..], "page {page}");
    }
}

#[test]
fn a_short_payload_reads_back_zero_padded() {
    let pager_store = tempfile::tempdir().unwrap();
    let mut pager = open(pager_store.path());
    pager.write_page(2, b"abc").unwrap();
    let page = pager.page(2).unwrap();
    assert_eq!(&page[..3], b"abc");
    assert!(page[3..].iter().all(|b| *b == 0));
    assert_eq!(page.len(), PAGE_PAYLOAD_SIZE);
}

#[test]
fn ac_p10_growth_stops_exactly_at_the_maximum_size() {
    let tmp = tempfile::tempdir().unwrap();
    let max = 1_572_864; // 1.5 MiB = 192 pages
    let mut pager = Pager::open(tmp.path(), &with_max(max)).unwrap();
    let mut page = HEADER_PAGES;
    let error = loop {
        match write(&mut pager, page) {
            Ok(()) => page += 1,
            Err(e) => break e,
        }
    };
    assert_eq!(error.kind(), ErrorKind::MaxSize, "{error}");
    assert_eq!(page, max / PAGE, "every page up to the maximum is usable");
    assert_eq!(pager.info().size_bytes, max);
    assert_eq!(fs::metadata(tmp.path().join(DATA_FILE)).unwrap().len(), max);
    for written in HEADER_PAGES..page {
        assert_eq!(pager.page(written).unwrap(), &payload(written)[..]);
    }
    pager.close().unwrap();
    let mut pager = open(tmp.path());
    write(&mut pager, page).unwrap();
    assert!(pager.info().size_bytes > max);
}

#[test]
fn ac_p11_a_store_larger_than_the_maximum_opens_but_cannot_grow() {
    let tmp = tempfile::tempdir().unwrap();
    let mut pager = open(tmp.path());
    write(&mut pager, 200).unwrap();
    let size = pager.info().size_bytes;
    assert!(size > 1_048_576);
    pager.close().unwrap();
    let pager = Pager::open(tmp.path(), &with_max(1_048_576)).unwrap();
    assert_eq!(pager.page(200).unwrap(), &payload(200)[..]);
    let error = pager.ensure_pages(pager.page_count() + 1).err().unwrap();
    assert_eq!(error.kind(), ErrorKind::MaxSize);
    assert_eq!(pager.info().size_bytes, size);
}

#[test]
fn ac_p12_growth_steps_are_page_multiples_that_never_shrink_and_increase() {
    let tmp = tempfile::tempdir().unwrap();
    let pager = open(tmp.path());
    let mut sizes = vec![pager.info().size_bytes];
    assert_eq!(sizes[0], INITIAL_SIZE);
    while sizes.len() < 6 {
        pager.ensure_pages(pager.page_count() + 1).unwrap();
        sizes.push(pager.info().size_bytes);
    }
    let steps: Vec<u64> = sizes.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(sizes.iter().all(|s| s.is_multiple_of(PAGE)), "{sizes:?}");
    assert!(steps.windows(2).all(|w| w[1] >= w[0]), "{steps:?}");
    assert!(steps[2] > steps[0], "{steps:?}");
    let last = *sizes.last().unwrap();
    assert_eq!(
        fs::metadata(tmp.path().join(DATA_FILE)).unwrap().len(),
        last
    );
    pager.close().unwrap();
    assert!(open(tmp.path()).info().size_bytes >= last);
}

#[test]
fn ac_p13_an_incomplete_final_page_is_ignored() {
    let tmp = tempfile::tempdir().unwrap();
    let info = open(tmp.path()).info();
    let data = tmp.path().join(DATA_FILE);
    let mut bytes = file_bytes(&data);
    bytes.extend_from_slice(&[0xAB; 100]);
    fs::write(&data, bytes).unwrap();
    let mut pager = open(tmp.path());
    assert_eq!(pager.info(), info);
    let next = pager.page_count();
    write(&mut pager, next).unwrap();
}

#[test]
fn ac_p13_a_page_lost_to_truncation_reads_as_damage() {
    let tmp = tempfile::tempdir().unwrap();
    let mut pager = open(tmp.path());
    write(&mut pager, 3).unwrap();
    pager.close().unwrap();
    let data = tmp.path().join(DATA_FILE);
    let bytes = file_bytes(&data);
    fs::write(&data, &bytes[..HEADER_PAGES as usize * PAGE_SIZE]).unwrap();
    let pager = open(tmp.path());
    match pager.page(3).err().unwrap() {
        Error::Damaged { page: Some(3), .. } => {}
        other => panic!("expected damage of page 3, got {other:?}"),
    }
}

#[test]
fn ac_p14_a_temporary_store_works_and_respects_its_maximum_size() {
    let mut pager = Pager::open_temporary(&with_max(1_048_576)).unwrap();
    let info = pager.info();
    assert!(info.temporary);
    assert_eq!(
        (info.format_version, info.page_size, info.size_bytes),
        (8, 8192, INITIAL_SIZE)
    );
    let mut page = HEADER_PAGES;
    let error = loop {
        match write(&mut pager, page) {
            Ok(()) => page += 1,
            Err(e) => break e,
        }
    };
    assert_eq!(error.kind(), ErrorKind::MaxSize);
    assert_eq!(page, 1_048_576 / PAGE);
    for written in HEADER_PAGES..page {
        assert_eq!(pager.page(written).unwrap(), &payload(written)[..]);
    }
    assert!(pager.info().temporary);
}

#[test]
fn ac14_a_new_temporary_store_is_empty() {
    let mut first = Pager::open_temporary(&OpenOptions::default()).unwrap();
    write(&mut first, 2).unwrap();
    first.close().unwrap();
    let second = Pager::open_temporary(&OpenOptions::default()).unwrap();
    assert_eq!(second.info().size_bytes, INITIAL_SIZE);
    assert_eq!(
        second.page(2).err().unwrap().kind(),
        ErrorKind::Damaged,
        "nothing was kept"
    );
}

#[test]
fn ac14_a_temporary_store_creates_no_files() {
    let tmp = tempfile::tempdir().unwrap();
    let private_temp = tmp.path().join("temp");
    let work = tmp.path().join("work");
    fs::create_dir(&private_temp).unwrap();
    fs::create_dir(&work).unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "child_helper_temporary_store",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("KRUMMI_TEST_TEMPORARY", "1")
        .env("TMP", &private_temp)
        .env("TEMP", &private_temp)
        .env("TMPDIR", &private_temp)
        .current_dir(&work)
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(fs::read_dir(&private_temp).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&work).unwrap().count(), 0);
}

/// Helper run in a child process by `ac14_a_temporary_store_creates_no_files`.
#[test]
fn child_helper_temporary_store() {
    if std::env::var_os("KRUMMI_TEST_TEMPORARY").is_none() {
        return;
    }
    let mut pager = Pager::open_temporary(&OpenOptions::default()).unwrap();
    for page in HEADER_PAGES..300 {
        write(&mut pager, page).unwrap();
    }
    assert_eq!(pager.page(299).unwrap(), &payload(299)[..]);
    pager.close().unwrap();
}

fn held_reference_survives_growth(pager: &Pager) {
    let reference = pager.page(2).unwrap();
    let address = reference.as_ptr();
    let before = reference.to_vec();
    for _ in 0..5 {
        pager.ensure_pages(pager.page_count() + 1).unwrap();
    }
    assert!(pager.info().size_bytes >= 32 * INITIAL_SIZE);
    assert_eq!(reference.as_ptr(), address);
    assert_eq!(reference, &before[..]);
    assert_eq!(pager.page(2).unwrap(), &before[..]);
}

#[test]
fn ac_p17_references_survive_growth() {
    let tmp = tempfile::tempdir().unwrap();
    let mut pager = open(tmp.path());
    write(&mut pager, 2).unwrap();
    held_reference_survives_growth(&pager);
    let mut pager = Pager::open_temporary(&OpenOptions::default()).unwrap();
    write(&mut pager, 2).unwrap();
    held_reference_survives_growth(&pager);
}

#[test]
fn ac20_page_reads_point_into_the_mapping_without_copying() {
    let tmp = tempfile::tempdir().unwrap();
    let mut pager = open(tmp.path());
    write(&mut pager, 2).unwrap();
    write(&mut pager, 3).unwrap();
    let a = pager.page(2).unwrap().as_ptr() as usize;
    let again = pager.page(2).unwrap().as_ptr() as usize;
    let b = pager.page(3).unwrap().as_ptr() as usize;
    assert_eq!(a, again, "two reads of a page return the same memory");
    assert_eq!(
        b - a,
        PAGE_SIZE,
        "consecutive pages are consecutive in the mapping"
    );
}

/// A run of `pages` pages sealed for `first`: a value of all its bytes but the last 100, which stay zero.
fn sealed_run(first: u64, pages: usize) -> Vec<u8> {
    let mut run = vec![0; pages * PAGE_SIZE];
    let end = run.len() - 100;
    for (i, b) in run[PAGE_HEADER_SIZE..end].iter_mut().enumerate() {
        *b = (i as u64 * 13 + first) as u8;
    }
    seal_run(first, &mut run);
    run
}

/// Page 2 sealed, then a run of 5 pages at pages 6 to 10, reserved by the growth from the initial 8 pages: the run
/// starts in the memory the store had and ends in the memory the growth added.
fn store_with_a_run_across_a_growth(pager: &mut Pager) -> Vec<u8> {
    write(pager, 2).unwrap();
    assert_eq!(pager.page_count(), INITIAL_SIZE / PAGE);
    pager.ensure_pages(11).unwrap();
    let run = sealed_run(6, 5);
    pager.write_run(6, &run).unwrap();
    run
}

/// The run at page 6 is one contiguous sequence in the store's memory, without a copy: where page 2 is plus four
/// pages, and the same memory on every read.
fn assert_run_in_place(pager: &Pager, run: &[u8]) {
    let read = pager.run(6, 5).unwrap();
    assert_eq!(read, run);
    let page_2 = pager.page(2).unwrap().as_ptr() as usize - PAGE_HEADER_SIZE;
    assert_eq!(read.as_ptr() as usize, page_2 + 4 * PAGE_SIZE);
    assert_eq!(pager.run(6, 5).unwrap().as_ptr(), read.as_ptr());
}

/// AC-V3, Pager part (D3): a run reads back as one contiguous sequence inside the store's memory, on disk and in
/// temporary mode, also when its pages were reserved by a growth and would have straddled two chunks of memory;
/// the reference survives later growths unchanged.
#[test]
fn ac_v3_a_run_reads_back_contiguously_and_without_a_copy() {
    let tmp = tempfile::tempdir().unwrap();
    let mut on_disk = open(tmp.path());
    let mut temporary = Pager::open_temporary(&OpenOptions::default()).unwrap();
    for pager in [&mut on_disk, &mut temporary] {
        let run = store_with_a_run_across_a_growth(pager);
        assert_run_in_place(pager, &run);
        let held = pager.run(6, 5).unwrap();
        for _ in 0..4 {
            pager.ensure_pages(pager.page_count() + 1).unwrap();
        }
        assert_eq!(held, &run[..]);
        assert_run_in_place(pager, &run);
    }
    on_disk.close().unwrap();
    let pager = open(tmp.path());
    assert_run_in_place(&pager, &sealed_run(6, 5));
}

/// AC-V7 and AC-V11, Pager part (G6, V7): a run read with another first page or another length, or beyond the end
/// of the store, is damage reported with its first page, as is a run whose first, interior or last page has an
/// altered byte; reading it again fails the same way.
#[test]
fn ac_v11_a_damaged_run_is_reported_with_its_first_page() {
    let damaged = |result: Result<&[u8], Error>| match result {
        Err(Error::Damaged { page, .. }) => page,
        other => panic!("expected damage, got {:?}", other.map(<[u8]>::len)),
    };
    let tmp = tempfile::tempdir().unwrap();
    let mut pager = open(tmp.path());
    let run = store_with_a_run_across_a_growth(&mut pager);
    assert_eq!(damaged(pager.run(7, 4)), Some(7));
    assert_eq!(damaged(pager.run(6, 4)), Some(6));
    assert_eq!(damaged(pager.run(6, 6)), Some(6));
    let beyond = pager.page_count() - 2;
    assert_eq!(damaged(pager.run(beyond, 3)), Some(beyond));
    pager.close().unwrap();
    let data = tmp.path().join(DATA_FILE);
    let pristine = file_bytes(&data);
    let file = fs::OpenOptions::new().write(true).open(&data).unwrap();
    let start = 6 * PAGE_SIZE;
    for offset in [
        start + 3,
        start + 2 * PAGE_SIZE + 77,
        start + 5 * PAGE_SIZE - 1,
    ] {
        write_byte(&file, offset, pristine[offset] ^ 0x01);
        let pager = open(tmp.path());
        assert_eq!(damaged(pager.run(6, 5)), Some(6), "offset {offset}");
        assert_eq!(damaged(pager.run(6, 5)), Some(6), "offset {offset}, again");
        assert_eq!(pager.page(2).unwrap(), &payload(2)[..]);
        drop(pager);
        write_byte(&file, offset, pristine[offset]);
    }
    assert_eq!(open(tmp.path()).run(6, 5).unwrap(), &run[..]);
}

/// AC10 for a run (AC-V11, Pager part), sampled: the run's header and every 251st byte of its pages, one at a time:
/// each flip is a damage error on the run's first page, and the sealed page next to it still reads.
#[test]
fn ac10_sample_no_damaged_byte_of_a_run_returns_wrong_data() {
    let tmp = tempfile::tempdir().unwrap();
    let mut pager = open(tmp.path());
    let run = store_with_a_run_across_a_growth(&mut pager);
    pager.close().unwrap();
    let data = tmp.path().join(DATA_FILE);
    let pristine = file_bytes(&data);
    let file = fs::OpenOptions::new().write(true).open(&data).unwrap();
    let run_bytes = 6 * PAGE_SIZE..11 * PAGE_SIZE;
    for offset in run_bytes
        .clone()
        .filter(|o| *o < run_bytes.start + PAGE_HEADER_SIZE || o % 251 == 0)
    {
        write_byte(&file, offset, pristine[offset] ^ 0x01);
        let pager = open(tmp.path());
        match pager.run(6, 5) {
            Err(Error::Damaged { page: Some(6), .. }) => {}
            Err(e) => panic!("offset {offset}: unexpected error {e}"),
            Ok(read) => panic!(
                "offset {offset}: a damaged run was returned ({})",
                read == run
            ),
        }
        assert_eq!(pager.page(2).unwrap(), &payload(2)[..], "offset {offset}");
        drop(pager);
        write_byte(&file, offset, pristine[offset]);
    }
}

#[test]
fn ac20_a_page_reference_cannot_outlive_the_store() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}

/// AC10, full: flips every byte of `krummi.data` one at a time. It reopens the store 65,536 times, which takes
/// minutes where an antivirus scans each modified file, so it runs as an acceptance step:
/// `cargo test -p krummi-pager --test pages -- --ignored ac10`.
#[test]
#[ignore = "acceptance: flips every byte of a store (slow)"]
fn ac10_no_single_damaged_byte_returns_wrong_data() {
    damaged_byte_sweep(|_| true);
}

/// AC10, sample run with the suite: the mark, both header copies, and one byte in 1021 of the rest (a prime, so
/// the sample falls at different positions of the page header and payload in each page).
#[test]
fn ac10_sample_no_damaged_byte_returns_wrong_data() {
    damaged_byte_sweep(|offset| {
        offset < 96 || (PAGE_SIZE..PAGE_SIZE + 32).contains(&offset) || offset % 1021 == 0
    });
}

/// Writes a small store, then flips each selected byte of `krummi.data` one at a time and reads everything
/// back: every flip is a damage error on its own page, a "not a store" error in the mark, or no difference.
fn damaged_byte_sweep(selected: impl Fn(usize) -> bool) {
    let tmp = tempfile::tempdir().unwrap();
    let mut pager = open(tmp.path());
    let pages = pager.page_count();
    for page in HEADER_PAGES..pages {
        pager.write_page(page, &payload(page)).unwrap();
    }
    pager.close().unwrap();
    let data = tmp.path().join(DATA_FILE);
    let pristine = file_bytes(&data);
    let mut not_a_store = 0;
    let mut damage = 0;
    let file = fs::OpenOptions::new().write(true).open(&data).unwrap();
    let mut flipped = Vec::new();
    for offset in (0..pristine.len()).filter(|o| selected(*o)) {
        flipped.push(offset);
        // Flip the byte in place, and restore it at the end of the iteration.
        write_byte(&file, offset, pristine[offset] ^ 0x01);
        match Pager::open(tmp.path(), &OpenOptions::default()) {
            Err(e) if e.kind() == ErrorKind::NotAStore => {
                assert!(offset < MARK.len(), "offset {offset}");
                not_a_store += 1;
            }
            Err(e) => panic!("offset {offset}: unexpected error {e}"),
            Ok(pager) => {
                for page in HEADER_PAGES..pages {
                    match pager.page(page) {
                        Ok(read) => {
                            assert_eq!(read, &payload(page)[..], "offset {offset}, page {page}")
                        }
                        Err(Error::Damaged { page: Some(p), .. }) => {
                            assert_eq!(p, page);
                            assert_eq!(offset as u64 / PAGE, page, "offset {offset}");
                            damage += 1;
                        }
                        Err(e) => panic!("offset {offset}, page {page}: unexpected error {e}"),
                    }
                }
            }
        }
        write_byte(&file, offset, pristine[offset]);
    }
    assert_eq!(
        not_a_store,
        flipped.iter().filter(|o| **o < MARK.len()).count()
    );
    let in_data_pages = flipped
        .iter()
        .filter(|o| **o as u64 >= HEADER_PAGES * PAGE)
        .count();
    assert_eq!(
        damage, in_data_pages,
        "every byte of a data page is checked"
    );
}
