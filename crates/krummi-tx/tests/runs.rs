//! Value runs in transactions (large values execution: D4, D5; large values PRD: V2, V3, V5, V8): a run is
//! reserved as one contiguous buffer, read back as one sequence without a copy in a write and in a read, kept for
//! the reads that still see it after a later write frees and reuses its pages, and committed, journaled and
//! transferred whole (AC-V3, AC-V6 and AC-V8 at the transaction level).

mod common;

use std::io;
use std::path::Path;

use common::{POWER_CUTS, Run, both, disk, put, temporary, text, wait_until};
use krummi_pager::DATA_FILE;
use krummi_pager::format::PAGE_HEADER_SIZE;
use krummi_pager::testing::{self, Action, Model, Op};
use krummi_tx::{
    Engine, Error, ErrorKind, Memory, OpenOptions, PAYLOAD_SIZE, ReadTransaction, WriteTransaction,
    run_pages,
};

const PAGE: usize = PAYLOAD_SIZE + PAGE_HEADER_SIZE;

/// Bytes of a value of `len` bytes, different for each `seed`.
fn value(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| ((i * 31 + i / PAGE) % 251) as u8 ^ seed)
        .collect()
}

/// Where a value lives: its run's first page and number of pages, and its length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Place {
    first: u64,
    count: u64,
    len: usize,
}

/// Reserves a run for `bytes` and fills it, checking that the reservation is zeroed and sized for it.
fn store(tx: &mut WriteTransaction<'_>, bytes: &[u8]) -> Place {
    let count = run_pages(bytes.len() as u64);
    let (first, body) = tx.allocate_value_run(count).unwrap();
    assert_eq!(body.len(), count as usize * PAGE - PAGE_HEADER_SIZE);
    assert!(body.iter().all(|&b| b == 0), "a new run is zeroed");
    body[..bytes.len()].copy_from_slice(bytes);
    Place {
        first,
        count,
        len: bytes.len(),
    }
}

fn in_write<'t>(tx: &'t WriteTransaction<'_>, place: Place) -> &'t [u8] {
    &tx.run(place.first, place.count).unwrap()[..place.len]
}

fn in_read<'t>(read: &'t ReadTransaction<'_>, place: Place) -> &'t [u8] {
    assert!(read.may_hold_run(place.first, place.count));
    &read.run(place.first, place.count).unwrap()[..place.len]
}

/// Keeps a value's place in the meta page's slots 0 to 2 (first page, pages, length).
fn keep(tx: &mut WriteTransaction<'_>, place: Place) {
    tx.set_slot(0, place.first).unwrap();
    tx.set_slot(1, place.count).unwrap();
    tx.set_slot(2, place.len as u64).unwrap();
}

/// The place kept in the meta page, if any.
fn kept(read: &ReadTransaction<'_>) -> Option<Place> {
    let first = read.slot(0).unwrap();
    (first != 0).then(|| Place {
        first,
        count: read.slot(1).unwrap(),
        len: read.slot(2).unwrap() as usize,
    })
}

fn kind<T>(result: Result<T, Error>) -> ErrorKind {
    result.err().expect("an error").kind()
}

/// The lengths the tests store: inside one page, exactly one page, one byte more, three pages, and more than a chunk
/// of the temporary area (8 MiB).
const LENS: [usize; 5] = [
    100,
    PAGE - PAGE_HEADER_SIZE,
    PAGE - PAGE_HEADER_SIZE + 1,
    3 * PAGE,
    9 << 20,
];

/// D4, D5, AC-V3: a run reads back whole and contiguous in the write that stored it (its own copy), after the
/// commit in a new read, and on disk after reopening; it takes the pages its length needs, at most one more than its
/// bytes fill (V5).
#[test]
fn a_run_reads_back_contiguous_in_its_write_after_commit_and_after_reopen() {
    let check = |engine: &Engine| -> Vec<Place> {
        let mut tx = engine.write(None).unwrap();
        let mut places = Vec::new();
        for (i, len) in LENS.into_iter().enumerate() {
            let bytes = value(len, i as u8);
            let place = store(&mut tx, &bytes);
            assert!(place.count <= (len as u64).div_ceil(PAGE as u64) + 1, "V5");
            assert_eq!(place.count, (len + PAGE_HEADER_SIZE).div_ceil(PAGE) as u64);
            assert!(tx.may_hold_run(place.first, place.count));
            assert_eq!(in_write(&tx, place), &bytes[..]);
            places.push(place);
        }
        tx.commit().unwrap();
        let read = engine.read().unwrap();
        for (i, &place) in places.iter().enumerate() {
            assert_eq!(in_read(&read, place), &value(place.len, i as u8)[..]);
        }
        places
    };
    check(&temporary());
    let tmp = tempfile::tempdir().unwrap();
    let places = check(&disk(tmp.path()));
    let engine = disk(tmp.path());
    let read = engine.read().unwrap();
    for (i, &place) in places.iter().enumerate() {
        let bytes = in_read(&read, place);
        assert_eq!(bytes, &value(place.len, i as u8)[..], "after reopening");
        assert_eq!(engine.memory_of(bytes), Some(Memory::Data));
    }
}

/// AC-V3 at the transaction level: the reference is the engine's memory, never a copy: in a write, its own buffer;
/// a committed run not transferred yet, the versions of the temporary area; a transferred one, `krummi.data` or the
/// temporary store's memory, also when the store grew to take it.
#[test]
fn a_run_s_reference_is_the_engine_s_memory_where_it_lives() {
    both(|engine| {
        engine.set_transfer_paused(true);
        let bytes = value(3 * PAGE, 7);
        let mut tx = engine.write(None).unwrap();
        let count = run_pages(bytes.len() as u64);
        let (first, body) = tx.allocate_value_run(count).unwrap();
        body[..bytes.len()].copy_from_slice(&bytes);
        let own = body.as_ptr();
        let place = Place {
            first,
            count,
            len: bytes.len(),
        };
        assert_eq!(in_write(&tx, place).as_ptr(), own, "the write's own copy");
        tx.commit().unwrap();

        let read = engine.read().unwrap();
        let reference = in_read(&read, place);
        assert_eq!(engine.memory_of(reference), Some(Memory::Versions));
        assert_eq!(
            reference.as_ptr(),
            in_read(&engine.read().unwrap(), place).as_ptr(),
            "the same version, not a copy"
        );
        engine.set_transfer_paused(false);
        assert!(engine.transfer_now());
        assert_eq!(reference, &bytes[..], "the reference outlives the transfer");
        let after = engine.read().unwrap();
        assert_eq!(engine.memory_of(in_read(&after, place)), Some(Memory::Data));
        assert_eq!(in_read(&after, place), &bytes[..]);
    });

    // In temporary mode a run that makes the store grow past the memory it had is contiguous too.
    let engine = temporary();
    let size = engine.info().size_bytes;
    let mut tx = engine.write(None).unwrap();
    let bytes = value(size as usize, 9);
    let place = store(&mut tx, &bytes);
    tx.commit().unwrap();
    assert!(engine.info().size_bytes > size, "the store grew");
    assert!(engine.transfer_now());
    let read = engine.read().unwrap();
    assert_eq!(engine.memory_of(in_read(&read, place)), Some(Memory::Data));
    assert_eq!(in_read(&read, place), &bytes[..]);
}

/// AC-V6 at the transaction level (G3, T14, T16): a read that holds a reference to a run keeps reading the same
/// bytes after later writes free the run and reuse its pages, all of them (as a run and as a page) or only its
/// first one, whether the run was still in the temporary area or already transferred, and while and after those
/// writes are transferred; new reads see the new content, also after reopening.
#[test]
fn a_read_keeps_a_run_that_later_writes_free_and_reuse() {
    for (transferred, whole) in [(false, true), (true, true), (false, false), (true, false)] {
        let tmp = tempfile::tempdir().unwrap();
        let new_len = if whole { PAGE } else { 100 };
        for engine in [disk(tmp.path()), temporary()] {
            // Only the transfers this test asks for run, so the old run stays where it is until then (paused
            // before the first commit: the background transfer could otherwise move it before the pause).
            engine.set_transfer_paused(true);
            let old_bytes = value(2 * PAGE + 100, 1);
            let mut tx = engine.write(None).unwrap();
            let old = store(&mut tx, &old_bytes);
            tx.commit().unwrap();
            assert_eq!(old.count, 3);
            if transferred {
                assert!(engine.transfer_now());
                wait_until(|| engine.scratch_bytes() == 0);
            }
            let read = engine.read().unwrap();
            let reference = in_read(&read, old);

            let mut tx = engine.write(None).unwrap();
            tx.free(old.first, old.count).unwrap();
            tx.commit().unwrap();
            let new_bytes = value(new_len, 2);
            let mut tx = engine.write(None).unwrap();
            let new = store(&mut tx, &new_bytes);
            assert_eq!(new.first, old.first, "the pages are reused");
            let page = whole.then(|| {
                let page = tx.allocate().unwrap();
                put(&mut tx, page, "reused");
                assert_eq!((new.count, page), (2, old.first + 2));
                page
            });
            keep(&mut tx, new);
            tx.commit().unwrap();

            let check_old = |read: &ReadTransaction<'_>| {
                assert_eq!(reference, &old_bytes[..]);
                assert_eq!(in_read(read, old), &old_bytes[..]);
            };
            check_old(&read);
            if !transferred {
                assert_eq!(in_read(&read, old).as_ptr(), reference.as_ptr());
            }
            let check_new = |engine: &Engine| {
                let read = engine.read().unwrap();
                assert_eq!(kept(&read), Some(new));
                assert_eq!(in_read(&read, new), &new_bytes[..]);
                if let Some(page) = page {
                    assert_eq!(text(read.page(page).unwrap()), "reused");
                }
            };
            check_new(&engine);
            // With only its first page reused, the old run is transferred now, as a whole, while the read holds it.
            engine.set_transfer_paused(false);
            assert!(engine.transfer_now());
            check_old(&read);
            check_new(&engine);
            drop(read);
            assert!(engine.transfer_now());
            wait_until(|| engine.scratch_bytes() == 0);
            check_new(&engine);
        }
        let engine = disk(tmp.path());
        let read = engine.read().unwrap();
        let new = kept(&read).unwrap();
        assert_eq!(
            in_read(&read, new),
            &value(new_len, 2)[..],
            "after reopening"
        );
        if whole {
            assert_eq!(text(read.page(new.first + 2).unwrap()), "reused");
        }
    }
}

/// D5: a transfer writes the versions it moves in the order they were committed, so a run whose pages a later run
/// reused never overwrites the newer content, wherever the later run starts.
#[test]
fn a_transfer_never_writes_an_older_run_over_a_newer_one() {
    let tmp = tempfile::tempdir().unwrap();
    for engine in [disk(tmp.path()), temporary()] {
        engine.set_transfer_paused(true);
        let mut tx = engine.write(None).unwrap();
        let before = store(&mut tx, &value(10, 3));
        let older = store(&mut tx, &value(2 * PAGE + 10, 4));
        assert_eq!(older.first, before.first + 1);
        tx.commit().unwrap();
        let mut tx = engine.write(None).unwrap();
        tx.free(before.first, before.count).unwrap();
        tx.free(older.first, older.count).unwrap();
        tx.commit().unwrap();
        let newer_bytes = value(PAGE + 10, 5);
        let mut tx = engine.write(None).unwrap();
        let newer = store(&mut tx, &newer_bytes);
        assert_eq!(
            (newer.first, newer.count),
            (before.first, 2),
            "it starts before the older run and covers its first page"
        );
        keep(&mut tx, newer);
        tx.commit().unwrap();
        engine.set_transfer_paused(false);
        assert!(engine.transfer_now());
        wait_until(|| engine.scratch_bytes() == 0);
        let read = engine.read().unwrap();
        assert_eq!(engine.memory_of(in_read(&read, newer)), Some(Memory::Data));
        assert_eq!(in_read(&read, newer), &newer_bytes[..]);
    }
    let engine = disk(tmp.path());
    let read = engine.read().unwrap();
    let newer = kept(&read).unwrap();
    assert_eq!(in_read(&read, newer), &value(PAGE + 10, 5)[..]);
}

/// D4, F6, F8: a run freed in the write that reserved it is dropped and its pages are free for the next writes; a
/// run is freed whole, and its pages are not pages: changing, reading or partly freeing them is a misuse.
#[test]
fn a_run_freed_in_its_own_write_is_dropped_and_its_pages_are_not_pages() {
    both(|engine| {
        let mut tx = engine.write(None).unwrap();
        let place = store(&mut tx, &value(2 * PAGE, 6));
        assert_eq!(kind(tx.page_mut(place.first + 1)), ErrorKind::Misuse);
        assert_eq!(kind(tx.page(place.first)), ErrorKind::Misuse);
        assert_eq!(kind(tx.free(place.first + 1, 1)), ErrorKind::Misuse);
        assert_eq!(
            kind(tx.commit()),
            ErrorKind::Misuse,
            "a misuse fails the write (T11)"
        );

        let mut tx = engine.write(None).unwrap();
        let place = store(&mut tx, &value(2 * PAGE, 6));
        tx.free(place.first, place.count).unwrap();
        assert!(!tx.may_hold_run(place.first, place.count));
        assert_eq!(kind(tx.run(place.first, place.count)), ErrorKind::Misuse);
        tx.abort();

        let mut tx = engine.write(None).unwrap();
        let place = store(&mut tx, &value(2 * PAGE, 6));
        tx.free(place.first, place.count).unwrap();
        tx.commit().unwrap();
        assert!(engine.info().free_pages >= place.count);
        let mut tx = engine.write(None).unwrap();
        let again = store(&mut tx, &value(2 * PAGE, 8));
        assert_eq!(again.first, place.first, "its pages are free again");
        tx.commit().unwrap();
        let read = engine.read().unwrap();
        assert_eq!(in_read(&read, again), &value(2 * PAGE, 8)[..]);
    });
}

/// G6, V7: a read, like a write, takes a run named in a leaf as one that may hold data only when the whole run lies
/// within the pages it can see: a run that starts in the store and ends beyond it, or whose end overflows, cannot.
#[test]
fn a_run_that_only_partly_fits_in_the_store_cannot_hold_data() {
    both(|engine| {
        let place = with_run(engine);
        let read = engine.read().unwrap();
        let pages = engine.info().page_count;
        assert!(read.may_hold_run(place.first, place.count));
        assert!(read.may_hold_run(pages - 1, 1), "the last page");
        assert!(
            !read.may_hold_run(pages - 1, 2),
            "one page beyond the store"
        );
        assert!(
            !read.may_hold_run(place.first, pages),
            "it ends beyond the store"
        );
        assert!(
            !read.may_hold_run(place.first, u64::MAX),
            "its end overflows"
        );
        assert!(!read.may_hold_run(place.first, 0), "no pages");
        let tx = engine.write(None).unwrap();
        assert!(!tx.may_hold_run(place.first, pages), "as the write sees it");
    });
}

/// T14, P10: a run counts against the maximum size exactly as the same number of pages does, while the write holds
/// it and once it is committed.
#[test]
fn a_run_counts_against_the_maximum_size_like_its_pages() {
    let max = 1 << 20;
    let options = OpenOptions {
        max_size: Some(max),
    };
    let largest = |value_run: bool| {
        let mut count = 1;
        loop {
            let engine = Engine::open_temporary(&options).unwrap();
            let mut tx = engine.write(None).unwrap();
            let reserved = if value_run {
                tx.allocate_value_run(count).map(|_| ())
            } else {
                tx.allocate_run(count).map(|_| ())
            };
            match reserved {
                Ok(()) => count += 1,
                Err(error) => {
                    assert_eq!(error.kind(), ErrorKind::MaxSize);
                    return count - 1;
                }
            }
        }
    };
    let pages = largest(false);
    assert!(pages > 16);
    assert_eq!(largest(true), pages);

    let committed = |value_run: bool| {
        let engine = Engine::open_temporary(&options).unwrap();
        engine.set_transfer_paused(true);
        let mut tx = engine.write(None).unwrap();
        if value_run {
            tx.allocate_value_run(5).unwrap();
        } else {
            tx.allocate_run(5).unwrap();
        }
        tx.commit().unwrap();
        engine.scratch_bytes()
    };
    assert_eq!(committed(true), committed(false));
}

/// The largest chunk the temporary area adds for single pages (`MAX_CHUNK_PAGES` of `temp.rs`).
const NORMAL_CHUNK: u64 = 1024 * PAGE as u64;

/// The maximum size of the tests that pile runs up in the temporary area.
const RUNS_MAX: u64 = 64 << 20;

/// Bytes of the files of `krummi.temp` in `folder`.
fn temp_files_bytes(folder: &Path) -> u64 {
    std::fs::read_dir(folder.join("krummi.temp")).map_or(0, |entries| {
        entries
            .map(|entry| entry.unwrap().metadata().unwrap().len())
            .sum()
    })
}

/// Checks that the maximum-size accounting covers what the temporary area holds, but for one chunk of single pages,
/// and that the store and its versions stay within [`RUNS_MAX`]; on disk, the area is the files of `krummi.temp`.
fn within_maximum(engine: &Engine, step: &str) {
    let (area, held, data) = (
        engine.area_bytes(),
        engine.scratch_bytes(),
        engine.info().size_bytes,
    );
    if let Some(folder) = engine.folder() {
        assert_eq!(temp_files_bytes(folder), area, "{step}");
    }
    assert!(
        area <= held + NORMAL_CHUNK && data + area <= RUNS_MAX,
        "{step}: the temporary area holds {area} bytes, {held} counted, and the store {data}, with a maximum of \
         {RUNS_MAX}"
    );
}

/// P10, T14 and the large values PRD (*Dejado a la implementación*: the memory reserved so that a run is contiguous
/// counts for the maximum size); R1 of the code review: replacing a value with one a page larger, again and again,
/// each over the largest chunk the temporary area grows by, never leaves the area holding more than the versions
/// still waiting plus one such chunk, so the store and its versions stay within the maximum size, on disk and in
/// temporary mode.
#[test]
fn growing_runs_never_pile_up_in_the_temporary_area() {
    let options = OpenOptions {
        max_size: Some(RUNS_MAX),
    };
    let tmp = tempfile::tempdir().unwrap();
    let engines = [
        Engine::open(tmp.path(), &options).unwrap(),
        Engine::open_temporary(&options).unwrap(),
    ];
    for engine in engines {
        engine.set_transfer_paused(true);
        let mut old: Option<Place> = None;
        for i in 0..16 {
            let bytes = value((1025 + i) * PAGE - PAGE_HEADER_SIZE, i as u8);
            let mut tx = engine.write(None).unwrap();
            if let Some(old) = old {
                tx.free(old.first, old.count).unwrap();
            }
            let place = store(&mut tx, &bytes);
            assert_eq!(place.count, 1025 + i as u64);
            keep(&mut tx, place);
            tx.commit().unwrap();
            old = Some(place);
            assert!(engine.transfer_now());
            wait_until(|| engine.scratch_bytes() == 0);
            within_maximum(&engine, &format!("write {i}"));
            let read = engine.read().unwrap();
            assert_eq!(in_read(&read, place), &bytes[..], "write {i}");
        }
        engine.close().unwrap();
    }
}

/// The run that the meta page keeps in the shrinking runs tests, with its bytes, and the first page every run reuses.
#[derive(Default)]
struct Shrinking {
    current: Option<(Place, Vec<u8>)>,
    first_page: Option<u64>,
}

impl Shrinking {
    /// Replaces the current run with one of `count` pages that reuses all its pages but its last one: frees it in a
    /// commit of its own, so that its pages can be reused, then stores the new one in another and has a transfer run.
    /// Checks [`within_maximum`] and that a new read sees the new run, or nothing when its write failed with the
    /// maximum-size class, which then applied nothing. Returns whether it failed.
    fn replace(&mut self, engine: &Engine, count: usize) -> bool {
        let none = Place {
            first: 0,
            count: 0,
            len: 0,
        };
        if let Some((old, _)) = self.current.take() {
            let mut tx = engine.write(None).unwrap();
            tx.free(old.first, old.count).unwrap();
            keep(&mut tx, none);
            tx.commit().unwrap();
        }
        let bytes = value(count * PAGE - PAGE_HEADER_SIZE, count as u8);
        let mut tx = engine.write(None).unwrap();
        let stored = match tx.allocate_value_run(count as u64) {
            Ok((first, body)) => {
                body[..bytes.len()].copy_from_slice(&bytes);
                let place = Place {
                    first,
                    count: count as u64,
                    len: bytes.len(),
                };
                keep(&mut tx, place);
                tx.commit().map(|_| place)
            }
            // The write ends without applying anything.
            Err(error) => {
                drop(tx);
                Err(error)
            }
        };
        let refused = match stored {
            Ok(place) => {
                assert_eq!(*self.first_page.get_or_insert(place.first), place.first);
                self.current = Some((place, bytes));
                false
            }
            Err(error) => {
                assert_eq!(error.kind(), ErrorKind::MaxSize, "{count} pages");
                true
            }
        };
        engine.transfer_now();

        within_maximum(engine, &format!("{count} pages"));
        let read = engine.read().unwrap();
        match &self.current {
            Some((place, bytes)) => {
                assert_eq!(kept(&read), Some(*place));
                assert_eq!(in_read(&read, *place), &bytes[..], "{count} pages");
            }
            None => assert_eq!(kept(&read), None, "{count} pages: nothing applied"),
        }
        refused
    }
}

/// The engines of the shrinking runs tests, on disk in `folder` and in temporary mode, with transfers paused.
fn shrinking_engines(folder: &Path) -> [Engine; 2] {
    let options = OpenOptions {
        max_size: Some(RUNS_MAX),
    };
    let engines = [
        Engine::open(folder, &options).unwrap(),
        Engine::open_temporary(&options).unwrap(),
    ];
    for engine in &engines {
        engine.set_transfer_paused(true);
    }
    engines
}

/// P10, T14 and the large values PRD, as in [`growing_runs_never_pile_up_in_the_temporary_area`]; R1 of the code
/// follow-up review: with a read opened before the first run and kept open, a run that reuses all of a freed run's
/// pages but its last one leaves that last page's version holding the whole earlier run. The maximum-size accounting
/// counts every run the temporary area keeps, whole, so the writes stay within the maximum or fail with its class
/// without applying anything, on disk and in temporary mode; once the read ends, a transfer empties the area.
#[test]
fn shrinking_runs_under_an_old_read_count_whole_for_the_maximum_size() {
    let tmp = tempfile::tempdir().unwrap();
    for engine in shrinking_engines(tmp.path()) {
        let old_read = engine.read().unwrap();
        let mut runs = Shrinking::default();
        let refused = (1025..=1040)
            .rev()
            .any(|count| runs.replace(&engine, count));
        assert!(refused, "the old read keeps more than the maximum size");
        // A write still works, and once the old read ends a transfer gives everything back.
        let mut tx = engine.write(None).unwrap();
        tx.set_slot(3, 1).unwrap();
        tx.commit().unwrap();
        drop(old_read);
        assert!(engine.transfer_now());
        wait_until(|| engine.scratch_bytes() == 0);
        let area = engine.area_bytes();
        assert!(area <= NORMAL_CHUNK, "the area keeps {area} bytes");
        engine.close().unwrap();
    }
}

/// P10, T14 and the large values PRD, as in [`shrinking_runs_under_an_old_read_count_whole_for_the_maximum_size`];
/// R1 of the third code follow-up review: once the old read ends, a transfer writes the runs it kept in place and
/// retires them, but a newer read that holds a reference to the current run keeps their slots allocated until it
/// ends. They stay counted for the maximum size until they are freed, so further replacements stay within the
/// maximum or fail with its class without applying anything, the newer read's reference keeps its bytes, and once
/// it ends a transfer empties the area, on disk and in temporary mode.
#[test]
fn shrinking_runs_retired_under_a_newer_read_count_until_freed() {
    let tmp = tempfile::tempdir().unwrap();
    for engine in shrinking_engines(tmp.path()) {
        let old_read = engine.read().unwrap();
        let mut runs = Shrinking::default();
        for count in (1035..=1040).rev() {
            assert!(!runs.replace(&engine, count), "{count} pages fit");
        }
        let (place, bytes) = runs.current.clone().unwrap();
        let new_read = engine.read().unwrap();
        let held = in_read(&new_read, place);
        drop(old_read);
        assert!(engine.transfer_now());
        within_maximum(&engine, "the old read ended");
        for count in (1030..=1034).rev() {
            runs.replace(&engine, count);
            assert_eq!(
                held,
                &bytes[..],
                "{count} pages: the newer read's reference"
            );
        }
        drop(new_read);
        assert!(engine.transfer_now());
        wait_until(|| engine.scratch_bytes() == 0);
        let area = engine.area_bytes();
        assert!(area <= NORMAL_CHUNK, "the area keeps {area} bytes");
        engine.close().unwrap();
    }
}

/// The commits of [`a_crash_at_any_point_keeps_runs_all_or_nothing`]: the value each one leaves, by generation.
fn generation_len(generation: u64) -> usize {
    [0, 2 * PAGE, 3 * PAGE + 7, 50, 0][generation as usize]
}

/// AC-V8, first part, at the transaction level (G1, G5): commits that store, overwrite (a new run, the old one
/// freed) and free runs, interrupted by a crash or a power cut at every point, leave each commit entire or absent.
#[test]
fn a_crash_at_any_point_keeps_runs_all_or_nothing() {
    let (source, engine) = Run::start();
    let mut returned = vec![testing::recorded_len()];
    let mut current: Option<Place> = None;
    for generation in 1..=4 {
        let mut tx = engine.write(None).unwrap();
        if let Some(old) = current.take() {
            tx.free(old.first, old.count).unwrap();
        }
        let len = generation_len(generation);
        if len > 0 {
            current = Some(store(&mut tx, &value(len, generation as u8)));
        }
        keep(
            &mut tx,
            current.unwrap_or(Place {
                first: 0,
                count: 0,
                len: 0,
            }),
        );
        tx.set_slot(3, generation).unwrap();
        tx.commit().unwrap();
        returned.push(testing::recorded_len());
    }
    let run = Run::finish(source, returned);
    drop(engine);
    for cut in 0..=run.ops.len() {
        for model in POWER_CUTS.into_iter().chain([Model::Written]) {
            let folder = run.crash(cut, model);
            if !folder.path().join(DATA_FILE).exists() {
                continue;
            }
            let engine = disk(folder.path());
            let read = engine.read().unwrap();
            let generation = read.slot(3).unwrap();
            let returned = run.returned_at(cut) as u64;
            assert!(
                generation + 1 == returned || generation == returned,
                "cut {cut} {model:?}: generation {generation}, {returned} returned"
            );
            let len = generation_len(generation);
            match kept(&read) {
                Some(place) => {
                    assert_eq!(place.len, len);
                    assert_eq!(
                        in_read(&read, place),
                        &value(len, generation as u8)[..],
                        "cut {cut} {model:?}"
                    );
                }
                None => assert_eq!(len, 0, "cut {cut} {model:?}"),
            }
        }
    }
}

/// A store with one committed run kept in the meta page.
fn with_run(engine: &Engine) -> Place {
    let mut tx = engine.write(None).unwrap();
    let place = store(&mut tx, &value(2 * PAGE, 11));
    keep(&mut tx, place);
    tx.commit().unwrap();
    place
}

/// Checks that the store still holds `place` with its bytes, and that a new write works.
fn unchanged(engine: &Engine, place: Place) {
    let read = engine.read().unwrap();
    assert_eq!(kept(&read), Some(place));
    assert_eq!(in_read(&read, place), &value(2 * PAGE, 11)[..]);
    drop(read);
    let mut tx = engine.write(None).unwrap();
    let page = tx.allocate().unwrap();
    put(&mut tx, page, "works");
    tx.abort();
}

/// The failure of class `class` (disk full, maximum size, memory exhausted, input and output) for a store in
/// `folder`.
fn t11_error(class: usize, folder: Option<&Path>) -> Error {
    let folder = folder.map(Path::to_path_buf).unwrap_or_default();
    match class {
        0 => Error::DiskFull { folder },
        1 => Error::MaxSize {
            max_size: 1 << 20,
            needed: 2 << 20,
        },
        2 => Error::OutOfMemory { requested: 1 << 20 },
        _ => Error::Io {
            folder,
            source: io::Error::other("injected"),
        },
    }
}

/// AC-V8, second part, at the transaction level (V8, T11): a failure of disk full, maximum size, memory exhausted
/// or input and output where a write reserves a run, reads one or frees one makes that operation fail with it; the
/// commit fails with that class and applies nothing, and a new write works.
#[test]
fn a_failure_at_a_run_s_reservation_read_or_free_fails_the_commit() {
    let tmp = tempfile::tempdir().unwrap();
    for class in 0..4 {
        for on_disk in [true, false] {
            let mut nth = 0;
            loop {
                let engine = if on_disk {
                    disk(&tmp.path().join(format!("{class}-{nth}")))
                } else {
                    temporary()
                };
                let place = with_run(&engine);
                let error = t11_error(class, engine.folder());
                let expected = error.kind();
                let mut tx = engine.write(None).unwrap();
                engine.set_page_fault(Some((nth, error)));
                let mut results = Vec::new();
                let bytes = value(3 * PAGE, 12);
                let count = run_pages(bytes.len() as u64);
                match tx.allocate_value_run(count) {
                    Ok((first, body)) => {
                        body[..bytes.len()].copy_from_slice(&bytes);
                        results.extend(tx.run(first, count).err().map(|e| e.kind()));
                    }
                    Err(e) => results.push(e.kind()),
                }
                results.extend(tx.run(place.first, place.count).err().map(|e| e.kind()));
                results.extend(tx.free(place.first, place.count).err().map(|e| e.kind()));
                if engine.page_fault_pending() {
                    engine.set_page_fault(None);
                    assert_eq!(results, vec![]);
                    tx.commit().unwrap();
                    break;
                }
                assert_eq!(results, vec![expected], "class {expected:?}, point {nth}");
                assert_eq!(kind(tx.commit()), expected, "point {nth}");
                unchanged(&engine, place);
                nth += 1;
            }
            assert_eq!(
                nth, 4,
                "reserving, reading the new run and the old one, freeing it"
            );
        }
    }
}

/// AC-V8, third part, at the transaction level (V8, G7, P13): running out of disk or memory while a commit stores its
/// runs in the temporary area fails the commit with that class at every point where it can happen, applies nothing,
/// and a new write works.
#[test]
fn a_full_disk_or_no_memory_while_committing_runs_fails_the_commit() {
    let tmp = tempfile::tempdir().unwrap();
    for (on_disk, op, io_kind, class) in [
        (
            true,
            Op::Temp,
            io::ErrorKind::StorageFull,
            ErrorKind::DiskFull,
        ),
        (
            false,
            Op::Alloc,
            io::ErrorKind::OutOfMemory,
            ErrorKind::OutOfMemory,
        ),
    ] {
        let engine = if on_disk {
            disk(tmp.path())
        } else {
            temporary()
        };
        let place = with_run(&engine);
        let mut failures = 0;
        loop {
            let mut tx = engine.write(None).unwrap();
            tx.free(place.first, place.count).unwrap();
            // Larger than the area's first chunk, so the commit makes it grow.
            let new = store(&mut tx, &value(40 * PAGE, 13));
            keep(&mut tx, new);
            testing::reset();
            testing::inject(op, testing::count(op) + failures, Action::Fail(io_kind));
            let committed = tx.commit();
            let fired = testing::count(op) > failures;
            testing::reset();
            match committed {
                Ok(()) => {
                    let read = engine.read().unwrap();
                    assert_eq!(in_read(&read, new), &value(40 * PAGE, 13)[..]);
                    break;
                }
                Err(error) => {
                    assert!(fired);
                    assert_eq!(error.kind(), class, "{error}");
                    unchanged(&engine, place);
                    failures += 1;
                }
            }
        }
        assert!(failures > 0, "{op:?} failed the commit at least once");
    }
}
