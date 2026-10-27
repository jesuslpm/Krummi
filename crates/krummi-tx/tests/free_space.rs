//! Free space (free-space PRD, F1–F14): reservations reuse freed pages before the store grows, ranges, the
//! frontier and its regions, misuse, and the free pages of the store information.

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use common::{Run, both, disk, put, text};
use krummi_pager::DATA_FILE;
use krummi_pager::format::{PAGE_HEADER_SIZE, PAGE_SIZE};
use krummi_pager::testing::{self, Action, Model, Op};
use krummi_tx::{Engine, ErrorKind, FIRST_PAGE, MAP_PAGE, META_PAGE, REGION_PAGES};

/// Region size for the criteria that need several regions (criteria preamble of the PRD).
const SMALL: u64 = 64;

/// Reserves `count` single pages, each holding `value`, in one commit.
fn reserve(engine: &Engine, count: u64, value: &str) -> Vec<u64> {
    let mut tx = engine.write(None).unwrap();
    let pages: Vec<u64> = (0..count).map(|_| tx.allocate().unwrap()).collect();
    for &page in &pages {
        put(&mut tx, page, value);
    }
    tx.commit().unwrap();
    pages
}

/// Frees `pages` in one commit.
fn free(engine: &Engine, pages: &[u64]) {
    let mut tx = engine.write(None).unwrap();
    for &page in pages {
        tx.free(page, 1).unwrap();
    }
    tx.commit().unwrap();
}

/// Reserves `count` single pages in one commit and returns them.
fn take(engine: &Engine, count: u64) -> Vec<u64> {
    reserve(engine, count, "taken")
}

fn free_pages(engine: &Engine) -> u64 {
    engine.info().free_pages
}

fn small_disk(folder: &Path) -> Engine {
    let engine = disk(folder);
    engine.set_region_pages(SMALL);
    engine
}

fn kind<T>(result: krummi_tx::Result<T>) -> ErrorKind {
    match result {
        Ok(_) => panic!("expected an error"),
        Err(e) => e.kind(),
    }
}

/// AC-F1 (F3, F4, F10): reservations take the freed pages, without growing the store or moving the frontier, and
/// the free pages of the information drop; the next one moves the frontier.
#[test]
fn ac_f1_reservations_reuse_freed_pages_before_growing() {
    both(|engine| {
        let pages = reserve(engine, 20, "used");
        let freed: Vec<u64> = pages.iter().copied().step_by(2).collect();
        free(engine, &freed);
        let (size, frontier, before) = (
            engine.info().size_bytes,
            engine.frontier(),
            free_pages(engine),
        );
        let mut tx = engine.write(None).unwrap();
        let taken: BTreeSet<u64> = (0..10).map(|_| tx.allocate().unwrap()).collect();
        tx.commit().unwrap();
        assert_eq!(taken, freed.iter().copied().collect());
        assert_eq!(engine.info().size_bytes, size);
        assert_eq!(engine.frontier(), frontier);
        assert_eq!(free_pages(engine), before - 10);
        assert_eq!(take(engine, 1), [frontier]);
    });
}

/// AC-F2 (F4, F6): a page freed by a write is not for that write's reservations, but for the next write's.
#[test]
fn ac_f2_a_freed_page_waits_for_the_next_write() {
    both(|engine| {
        let page = reserve(engine, 3, "used")[1];
        let mut tx = engine.write(None).unwrap();
        tx.free(page, 1).unwrap();
        assert_ne!(tx.allocate().unwrap(), page);
        tx.commit().unwrap();
        assert_eq!(take(engine, 1), [page]);
    });
}

fn commit(tx: krummi_tx::WriteTransaction<'_>, deferred: bool) {
    if deferred {
        tx.commit_deferred().unwrap();
    } else {
        tx.commit().unwrap();
    }
}

/// Frees and reuses one page three times, both commits deferred or both durable, with a read opened before each
/// free and all of them held to the end; each read sees its generation and a new read the newest. Returns the page.
fn three_generations(engine: &Engine, deferred: bool) -> u64 {
    let page = reserve(engine, 2, "gen0")[0];
    let mut reads = Vec::new();
    for generation in 1..=3 {
        reads.push(engine.read().unwrap());
        let mut tx = engine.write(None).unwrap();
        tx.free(page, 1).unwrap();
        commit(tx, deferred);
        let mut tx = engine.write(None).unwrap();
        assert_eq!(tx.allocate().unwrap(), page, "the only free page");
        put(&mut tx, page, &format!("gen{generation}"));
        commit(tx, deferred);
        if deferred {
            assert_eq!(text(reads[0].page(page).unwrap()), "gen0");
            engine.make_durable().unwrap();
        }
        assert_eq!(common::read_text(engine, page), format!("gen{generation}"));
    }
    for (generation, read) in reads.iter().enumerate() {
        assert_eq!(text(read.page(page).unwrap()), format!("gen{generation}"));
    }
    page
}

/// AC-F3 (F6, T14, E3): reads opened before a page is freed and reused keep seeing its earlier content, through
/// several generations of the same page number; new reads, and the store after reopening, see the newest. AC-F18:
/// the same in a temporary store, without the deferred and reopening parts.
#[test]
fn ac_f3_old_reads_keep_their_version_of_a_reused_page() {
    for deferred in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let engine = disk(tmp.path());
        let page = three_generations(&engine, deferred);
        drop(engine);
        assert_eq!(common::read_text(&disk(tmp.path()), page), "gen3");
    }
    three_generations(&common::temporary(), false);
}

/// AC-F4 (F6, T10, T11): an aborted write, one dropped without committing, and one whose commit fails for lack of
/// space apply none of their frees and reservations. AC-F18: the same in a temporary store, without the disk-full
/// part.
#[test]
fn ac_f4_an_aborted_or_failed_write_frees_and_reserves_nothing() {
    let endings = [("abort", true), ("drop", true), ("disk full", true)];
    let temporary = [("abort", false), ("drop", false)];
    for (ending, on_disk) in endings.into_iter().chain(temporary) {
        let tmp = tempfile::tempdir().unwrap();
        let engine = if on_disk {
            disk(tmp.path())
        } else {
            common::temporary()
        };
        engine.set_journal_file_limit(4096);
        let pages = reserve(&engine, 6, "used");
        free(&engine, &pages[..2]);
        let (frontier, before) = (engine.frontier(), free_pages(&engine));
        let mut tx = engine.write(None).unwrap();
        tx.free(pages[4], 1).unwrap();
        let taken = [tx.allocate().unwrap(), tx.allocate().unwrap()];
        assert_eq!(BTreeSet::from(taken), BTreeSet::from([pages[0], pages[1]]));
        match ending {
            "abort" => tx.abort(),
            "drop" => drop(tx),
            _ => {
                testing::reset();
                testing::inject(
                    Op::Reserve,
                    testing::count(Op::Reserve),
                    Action::Fail(std::io::ErrorKind::StorageFull),
                );
                assert_eq!(kind(tx.commit()), ErrorKind::DiskFull);
                testing::reset();
            }
        }
        assert_eq!(
            (engine.frontier(), free_pages(&engine)),
            (frontier, before),
            "{ending}"
        );
        assert_eq!(common::read_text(&engine, pages[4]), "used", "{ending}");
        let again = BTreeSet::from_iter(take(&engine, 2));
        assert_eq!(again, BTreeSet::from([pages[0], pages[1]]), "{ending}");
    }
}

/// AC-F7 (F5): a range reuses a long enough run of free pages; free pages that are only loose make it start at
/// the frontier.
#[test]
fn ac_f7_a_range_reuses_a_run_or_starts_at_the_frontier() {
    both(|engine| {
        let first = reserve(engine, 40, "used")[10];
        let mut tx = engine.write(None).unwrap();
        tx.free(first, 20).unwrap();
        tx.commit().unwrap();
        let frontier = engine.frontier();
        let mut tx = engine.write(None).unwrap();
        let range = tx.allocate_run(16).unwrap();
        tx.commit().unwrap();
        assert!(
            (first..=first + 4).contains(&range),
            "{range} inside the run at {first}"
        );
        assert_eq!(engine.frontier(), frontier);
    });
    both(|engine| {
        let pages = reserve(engine, 200, "used");
        let loose: Vec<u64> = pages.iter().copied().step_by(2).collect();
        free(engine, &loose);
        let frontier = engine.frontier();
        let mut tx = engine.write(None).unwrap();
        assert_eq!(tx.allocate_run(16).unwrap(), frontier);
        tx.commit().unwrap();
        assert_eq!(engine.frontier(), frontier + 16);
    });
}

/// Pages from `frontier` to the end of a store of `page_count` pages that are not map positions.
fn pages_after(frontier: u64, page_count: u64, region: u64) -> u64 {
    (frontier..page_count)
        .filter(|p| !p.is_multiple_of(region))
        .count() as u64
}

/// AC-F8 (F3, F5, F10): a range that does not fit in the frontier's region starts after the next region's map
/// page; the skipped pages are free, counted in the information and reserved by the next single reservation.
#[test]
fn ac_f8_a_range_that_does_not_fit_skips_to_the_next_region() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = small_disk(tmp.path());
    let mut tx = engine.write(None).unwrap();
    tx.allocate_run(SMALL - 10 - FIRST_PAGE).unwrap();
    tx.commit().unwrap();
    assert_eq!(engine.frontier(), SMALL - 10);
    let mut tx = engine.write(None).unwrap();
    assert_eq!(tx.allocate_run(16).unwrap(), SMALL + 1);
    tx.commit().unwrap();
    let info = engine.info();
    assert_eq!(
        info.free_pages,
        10 + pages_after(engine.frontier(), info.page_count, SMALL)
    );
    let page = take(&engine, 1)[0];
    assert!((SMALL - 10..SMALL).contains(&page));
}

/// Runs every misuse of the PRD's validation table in a write that already changed, reserved and freed pages:
/// each fails with the misuse class, the commit fails with it, and none of the changes apply. Returns the two
/// pages that write changed and freed, and the free pages after the cases.
fn misuse_spoils_the_write(engine: &Engine) -> ([u64; 2], u64) {
    // Two free pages: the write's own reservation takes the first, the misuse frees the second.
    let used = reserve(engine, 4, "used");
    free(engine, &used[2..]);
    type Misuse = fn(&mut krummi_tx::WriteTransaction<'_>, u64, u64) -> krummi_tx::Result<()>;
    let cases: [(&str, Misuse); 14] = [
        ("a range of 0", |tx, _, _| tx.allocate_run(0).map(drop)),
        ("a range over the cap", |tx, _, _| {
            tx.allocate_run(SMALL).map(drop)
        }),
        ("freeing twice", |tx, page, _| {
            tx.free(page, 1).unwrap();
            tx.free(page, 1)
        }),
        ("freeing a free page", |tx, _, hole| tx.free(hole, 1)),
        ("freeing a header page", |tx, _, _| tx.free(0, 1)),
        ("freeing the meta page", |tx, _, _| tx.free(META_PAGE, 1)),
        ("freeing the first map page", |tx, _, _| {
            tx.free(MAP_PAGE, 1)
        }),
        ("freeing another map page", |tx, _, _| tx.free(SMALL, 1)),
        ("freeing past the frontier", |tx, _, _| {
            tx.free(10 * SMALL + 1, 1)
        }),
        ("freeing 0 pages", |tx, page, _| tx.free(page, 0)),
        ("reading a freed page", |tx, page, _| {
            tx.free(page, 1).unwrap();
            tx.page(page).map(drop)
        }),
        ("changing a freed page", |tx, page, _| {
            tx.free(page, 1).unwrap();
            tx.page_mut(page).map(drop)
        }),
        ("changing the first map page", |tx, _, _| {
            tx.page_mut(MAP_PAGE).map(drop)
        }),
        ("changing another map page", |tx, _, _| {
            tx.page_mut(SMALL).map(drop)
        }),
    ];
    let free_before = free_pages(engine);
    for (name, misuse) in cases {
        let mut tx = engine.write(None).unwrap();
        put(&mut tx, used[0], "changed");
        let reserved = tx.allocate().unwrap();
        put(&mut tx, reserved, "reserved");
        tx.free(used[1], 1).unwrap();
        assert_eq!(
            kind(misuse(&mut tx, used[0], used[3])),
            ErrorKind::Misuse,
            "{name}"
        );
        assert_eq!(kind(tx.commit()), ErrorKind::Misuse, "{name}");
        assert_eq!(common::read_text(engine, used[0]), "used", "{name}");
        assert_eq!(common::read_text(engine, used[1]), "used", "{name}");
        assert_eq!(free_pages(engine), free_before, "{name}");
    }
    ([used[0], used[1]], free_before)
}

/// AC-F9 (F5, F8, T11): each misuse fails with the misuse class and no effect, the write can no longer be
/// committed, and none of its earlier changes apply, also after reopening; the next write works. AC-F18: the same
/// in a temporary store.
#[test]
fn ac_f9_misuse_fails_and_spoils_the_write() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = small_disk(tmp.path());
    let (pages, free_before) = misuse_spoils_the_write(&engine);
    drop(engine);
    let engine = small_disk(tmp.path());
    for page in pages {
        assert_eq!(common::read_text(&engine, page), "used", "after reopening");
    }
    assert_eq!(free_pages(&engine), free_before, "after reopening");
    common::set(&engine, pages[0], "the next write works");
    let engine = common::temporary();
    engine.set_region_pages(SMALL);
    let (pages, _) = misuse_spoils_the_write(&engine);
    common::set(&engine, pages[0], "the next write works");
}

/// AC-F9, last part (F5): with the product's region size, a range as long as the cap is reserved in a region
/// that is not the first one.
#[test]
fn ac_f9_a_range_as_long_as_the_cap_is_reserved() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let mut tx = engine.write(None).unwrap();
    let first = tx.allocate_run(REGION_PAGES - 1).unwrap();
    assert_eq!(
        first,
        REGION_PAGES + 1,
        "after the second region's map page"
    );
    tx.abort();
}

/// A write that changes a page, reserves a range of 8 and a single page, and frees 3 pages of the range, the
/// single page and the changed one. Returns (the changed page, the range, the single page).
fn free_parts(engine: &Engine) -> (u64, u64, u64) {
    let changed = reserve(engine, 1, "before")[0];
    let mut tx = engine.write(None).unwrap();
    put(&mut tx, changed, "after");
    let range = tx.allocate_run(8).unwrap();
    let single = tx.allocate().unwrap();
    tx.free(range + 2, 3).unwrap();
    tx.free(changed, 1).unwrap();
    tx.free(single, 1).unwrap();
    tx.commit().unwrap();
    (changed, range, single)
}

/// AC-F10 (F4, F8): freeing part of a reserved range, a page reserved in the same write and a page changed in it.
/// AC-F18: the same in a temporary store, without reopening.
#[test]
fn ac_f10_freeing_parts_of_ranges_and_changed_pages() {
    let tmp = tempfile::tempdir().unwrap();
    let (changed, range, single) = free_parts(&disk(tmp.path()));
    check_parts(&disk(tmp.path()), changed, range, single);
    let engine = common::temporary();
    let (changed, range, single) = free_parts(&engine);
    check_parts(&engine, changed, range, single);
}

fn check_parts(engine: &Engine, changed: u64, range: u64, single: u64) {
    let read = engine.read().unwrap();
    for page in [range, range + 1, range + 5, range + 6, range + 7] {
        assert!(
            read.page(page).unwrap().iter().all(|&b| b == 0),
            "{page} in use and zeroed"
        );
    }
    assert_eq!(
        text(read.page(changed).unwrap()),
        "before",
        "its change was dropped"
    );
    drop(read);
    let expected = BTreeSet::from([range + 2, range + 3, range + 4, changed, single]);
    assert_eq!(BTreeSet::from_iter(take(engine, 5)), expected);
}

/// AC-F13 (F4, F9): opening reads no map page, and a reservation still finds the free pages of a region far from
/// the frontier.
#[test]
fn ac_f13_opening_reads_no_map_and_far_free_pages_are_found() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = small_disk(tmp.path());
    let near = reserve(&engine, 5, "used");
    let mut tx = engine.write(None).unwrap();
    tx.allocate_run(SMALL - engine.frontier()).unwrap();
    tx.allocate_run(SMALL - 1).unwrap();
    tx.allocate_run(SMALL - 1).unwrap();
    tx.commit().unwrap();
    free(&engine, &near[1..3]);
    let frontier = engine.frontier();
    engine.close().unwrap();
    let engine = small_disk(tmp.path());
    assert_eq!(engine.regions_read(), 0);
    let page = take(&engine, 1)[0];
    assert!(near[1..3].contains(&page));
    assert_eq!(engine.frontier(), frontier);
}

/// Flips a byte of the payload of `page` in the closed store's data file.
fn damage(folder: &Path, page: u64) {
    let data = folder.join(DATA_FILE);
    let mut bytes = fs::read(&data).unwrap();
    bytes[page as usize * PAGE_SIZE + PAGE_HEADER_SIZE + 100] ^= 0xFF;
    fs::write(&data, bytes).unwrap();
}

/// AC-F14 (F9, F10): a damaged map page fails, with its number, only the reservations that read it; the store
/// opens, its information and every other page still work.
#[test]
fn ac_f14_a_damaged_map_page_fails_only_what_reads_it() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = small_disk(tmp.path());
    let mut tx = engine.write(None).unwrap();
    tx.allocate_run(SMALL - FIRST_PAGE).unwrap();
    let second = tx.allocate_run(SMALL - 1).unwrap();
    let third = tx.allocate_run(SMALL - 1).unwrap();
    tx.commit().unwrap();
    let mut tx = engine.write(None).unwrap();
    tx.free(second + 5, 3).unwrap();
    tx.commit().unwrap();
    let before = free_pages(&engine);
    engine.close().unwrap();
    damage(tmp.path(), SMALL);
    let engine = small_disk(tmp.path());
    assert_eq!(free_pages(&engine), before);
    let read = engine.read().unwrap();
    for page in (FIRST_PAGE..engine.frontier()).filter(|&p| !p.is_multiple_of(SMALL)) {
        if !(second + 5..second + 8).contains(&page) {
            read.page(page).unwrap();
        }
    }
    drop(read);
    common::set(&engine, third, "still works");
    for _ in 0..2 {
        let mut tx = engine.write(None).unwrap();
        match tx.allocate() {
            Err(krummi_tx::Error::Damaged { page, .. }) => assert_eq!(page, Some(SMALL)),
            other => panic!("expected damage, got {other:?}"),
        }
        assert_eq!(kind(tx.commit()), ErrorKind::Damaged);
    }
}

/// AC-F15 (F10, E3): a new store's free pages are all but its own pages; frees count once they are committed and
/// durable, and the value survives reopening.
#[test]
fn ac_f15_free_pages_follow_what_is_durable() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let info = engine.info();
    assert_eq!(info.free_pages, info.page_count - FIRST_PAGE);
    let pages = reserve(&engine, 3, "used");
    let before = free_pages(&engine);
    let mut tx = engine.write(None).unwrap();
    tx.free(pages[0], 3).unwrap();
    assert_eq!(free_pages(&engine), before, "an open write");
    engine.set_journal_paused(true);
    tx.commit_deferred().unwrap();
    assert_eq!(
        free_pages(&engine),
        before,
        "a deferred commit not durable yet"
    );
    engine.set_journal_paused(false);
    engine.make_durable().unwrap();
    assert_eq!(free_pages(&engine), before + 3);
    drop(engine);
    assert_eq!(free_pages(&disk(tmp.path())), before + 3);
}

/// AC-F18 (F12): the same rules in a temporary store (AC-F1, AC-F2 and AC-F7 already run on both kinds).
#[test]
fn ac_f18_a_temporary_store_follows_the_same_rules() {
    let engine = common::temporary();
    engine.set_region_pages(SMALL);
    let info = engine.info();
    assert_eq!(info.free_pages, info.page_count - FIRST_PAGE);
    let pages = reserve(&engine, 3, "used");
    let before = free_pages(&engine);
    let mut tx = engine.write(None).unwrap();
    tx.free(pages[0], 3).unwrap();
    assert_eq!(free_pages(&engine), before, "an open write");
    tx.commit().unwrap();
    assert_eq!(free_pages(&engine), before + 3, "visible once it returns");
    let mut tx = engine.write(None).unwrap();
    tx.free(pages[0], 1).unwrap_err();
    tx.abort();
    let mut tx = engine.write(None).unwrap();
    tx.allocate_run(SMALL - 10 - engine.frontier()).unwrap();
    assert_eq!(tx.allocate_run(16).unwrap(), SMALL + 1);
    tx.commit().unwrap();
}

/// Whether `page`, below the frontier and not one of the store's own, is free in the map as the next write sees
/// it: freeing it is then a misuse.
fn is_free(engine: &Engine, page: u64) -> bool {
    let mut tx = engine.write(None).unwrap();
    let free = tx.free(page, 1).is_err();
    tx.abort();
    free
}

/// AC-F5 (F6, E8): when the group of a commit that frees a page fails, the write that reused the page fails too,
/// and reopening never brings the reuse without the free.
#[test]
fn ac_f5_a_failed_group_never_keeps_a_reuse_without_its_free() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let page = reserve(&engine, 1, "orig")[0];
    let marker = reserve(&engine, 1, "none")[0];
    engine.set_journal_paused(true);
    std::thread::scope(|s| {
        let engine = &engine;
        let first = common::queue_on_thread(s, engine, 1, move || {
            let mut tx = engine.write(None).unwrap();
            tx.free(page, 1).unwrap();
            put(&mut tx, marker, "first");
            tx.commit()
        });
        let second = common::queue_on_thread(s, engine, 2, move || {
            let mut tx = common::next_writer(engine);
            assert_eq!(tx.allocate().unwrap(), page);
            put(&mut tx, page, "reused");
            tx.commit()
        });
        testing::reset();
        testing::inject(Op::Write, 0, Action::Fail(std::io::ErrorKind::Other));
        engine.set_journal_paused(false);
        assert_eq!(kind(first.join().unwrap()), ErrorKind::Io);
        assert_eq!(kind(second.join().unwrap()), ErrorKind::Io);
    });
    testing::reset();
    drop(engine);
    let engine = disk(tmp.path());
    let first = common::read_text(&engine, marker) == "first";
    let second = common::read_text(&engine, page) == "reused";
    assert!(first || !second, "never the reuse without the free");
}

/// Checks a store after a crash in [`ac_f6_crashes_keep_frees_and_reuses_consistent`]: the marker says which
/// commit was the last one kept; the page is free after a free and holds its generation after a reuse. A cut
/// before the marker was committed has nothing to check.
fn check_generations(folder: &Path, page: u64, marker: u64, label: &str) {
    let engine = disk(folder);
    let Ok(marked) = engine.read().unwrap().page(marker).map(text) else {
        return;
    };
    let step: u64 = marked.parse().unwrap();
    let freed = step % 2 == 1;
    assert_eq!(is_free(&engine, page), freed, "{label}: step {step}");
    if !freed {
        let expected = format!("g{}", step / 2);
        assert_eq!(common::read_text(&engine, page), expected, "{label}");
    }
    let info = engine.info();
    assert!(
        info.page_count >= engine.frontier(),
        "{label}: the store reaches the frontier"
    );
    let after = pages_after(engine.frontier(), info.page_count, REGION_PAGES);
    assert_eq!(info.free_pages, u64::from(freed) + after, "{label}");
}

/// AC-F6 (F2, F3, F6, F10, G5): power cuts at every write and flush of commits in both modes that free a page and
/// reuse it three times, and crashes at every point of the recovery, keep a prefix in which the page and the free
/// pages agree with the last commit kept.
#[test]
fn ac_f6_crashes_keep_frees_and_reuses_consistent() {
    let (source, engine) = Run::start();
    let page = reserve(&engine, 1, "g0")[0];
    let marker = reserve(&engine, 1, "0")[0];
    let mut returned = Vec::new();
    for step in 1..=6u64 {
        let mut tx = engine.write(None).unwrap();
        if step % 2 == 1 {
            tx.free(page, 1).unwrap();
        } else {
            assert_eq!(tx.allocate().unwrap(), page);
            put(&mut tx, page, &format!("g{}", step / 2));
        }
        put(&mut tx, marker, &step.to_string());
        if step % 3 == 0 {
            tx.commit_deferred().unwrap();
        } else {
            tx.commit().unwrap();
        }
        returned.push(testing::recorded_len());
    }
    let run = Run::finish(source, returned);
    drop(engine);
    for cut in 0..=run.ops.len() {
        for model in common::POWER_CUTS {
            let folder = run.crash(cut, model);
            if folder.path().join(DATA_FILE).exists() {
                let label = format!("cut {cut} {model:?}");
                check_generations(folder.path(), page, marker, &label);
            }
        }
    }
    let crashed = run.killed();
    testing::reset();
    testing::start_recording();
    let recovering = common::copy_folder(crashed.path());
    drop(disk(recovering.path()));
    let recovery = testing::take_recording();
    testing::reset();
    for cut in 0..=recovery.len() {
        let target = common::copy_folder(crashed.path());
        testing::materialize(
            &recovery,
            cut,
            recovering.path(),
            target.path(),
            Model::Written,
        );
        check_generations(target.path(), page, marker, &format!("recovery cut {cut}"));
    }
}

/// AC-F6, the frontier (F3): a commit that grows the store and frees the last page it reserved still leaves, after
/// any power cut that keeps it, a store that reaches its frontier.
#[test]
fn ac_f6_the_store_reaches_the_frontier_after_power_cuts() {
    let (source, engine) = Run::start();
    let marker = reserve(&engine, 1, "before")[0];
    let mut tx = engine.write(None).unwrap();
    let first = tx.allocate_run(20).unwrap();
    tx.free(first + 19, 1).unwrap();
    put(&mut tx, marker, "after");
    tx.commit().unwrap();
    let run = Run::finish(source, vec![]);
    drop(engine);
    for cut in 0..=run.ops.len() {
        for model in common::POWER_CUTS {
            let folder = run.crash(cut, model);
            if !folder.path().join(DATA_FILE).exists() {
                continue;
            }
            let engine = disk(folder.path());
            let committed = engine.read().unwrap().page(marker).map(text).ok();
            if committed.as_deref() == Some("after") {
                let label = format!("cut {cut} {model:?}");
                assert!(engine.info().page_count >= engine.frontier(), "{label}");
                assert!(is_free(&engine, first + 19), "{label}");
            }
        }
    }
}

/// Reserves pages until the store holds at least `pages` pages and the frontier is at its end.
fn fill(engine: &Engine, pages: u64) {
    loop {
        let (count, frontier) = (engine.info().page_count, engine.frontier());
        if count >= pages && frontier == count {
            return;
        }
        let mut tx = engine.write(None).unwrap();
        tx.allocate_run((count - frontier).max(1)).unwrap();
        tx.commit().unwrap();
    }
}

/// AC-F11 (F5, P10, G7): with no room for `krummi.data` to grow, the free pages are still reserved, while a
/// reservation that needs to grow fails with the maximum-size or disk-full class and applies nothing.
#[test]
fn ac_f11_free_pages_are_reused_when_the_store_cannot_grow() {
    const FREED: u64 = 4;
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    fill(&engine, 128);
    let size = engine.info().size_bytes;
    let pages: Vec<u64> = (FIRST_PAGE..FIRST_PAGE + 2 * FREED).step_by(2).collect();
    free(&engine, &pages);
    engine.close().unwrap();
    // Room for the versions of a write that reserves the free pages, with the meta page and a map page.
    let options = krummi_tx::OpenOptions {
        max_size: Some(size + (FREED + 2) * PAGE_SIZE as u64),
    };
    let copy = common::copy_folder(tmp.path());
    let engine = Engine::open(tmp.path(), &options).unwrap();
    let mut tx = engine.write(None).unwrap();
    let taken: BTreeSet<u64> = (0..FREED).map(|_| tx.allocate().unwrap()).collect();
    tx.commit().unwrap();
    assert_eq!(taken, BTreeSet::from_iter(pages.iter().copied()));
    drop(engine);
    let engine = Engine::open(copy.path(), &options).unwrap();
    let mut tx = engine.write(None).unwrap();
    for _ in 0..FREED {
        tx.allocate().unwrap();
    }
    assert_eq!(kind(tx.allocate()), ErrorKind::MaxSize);
    assert_eq!(kind(tx.commit()), ErrorKind::MaxSize);
    assert!(
        pages.iter().all(|&p| is_free(&engine, p)),
        "nothing applied"
    );
    assert_eq!(engine.info().size_bytes, size);
    common::set(&engine, FIRST_PAGE + 1, "still accepts writes");
    drop(engine);

    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    fill(&engine, 16);
    free(&engine, &[FIRST_PAGE, FIRST_PAGE + 2]);
    testing::reset();
    testing::inject(
        Op::Reserve,
        testing::count(Op::Reserve),
        Action::Fail(std::io::ErrorKind::StorageFull),
    );
    let mut tx = engine.write(None).unwrap();
    assert_eq!(kind(tx.allocate_run(3)), ErrorKind::DiskFull);
    assert_eq!(kind(tx.commit()), ErrorKind::DiskFull);
    testing::reset();
    let mut tx = engine.write(None).unwrap();
    tx.allocate().unwrap();
    tx.allocate().unwrap();
    tx.commit().unwrap();
}

/// AC-F12 (F7): a read held open while commits free and reserve the same number of pages does not make
/// `krummi.data` grow after the second commit.
/// AC-F18: the same in a temporary store.
#[test]
fn ac_f12_a_long_read_does_not_make_the_store_grow() {
    both(long_read);
}

fn long_read(engine: &Engine) {
    let mut held = reserve(engine, 8, "v");
    let read = engine.read().unwrap();
    let mut size = 0;
    for i in 0..1000 {
        let mut tx = engine.write(None).unwrap();
        for &page in &held {
            tx.free(page, 1).unwrap();
        }
        held = (0..8).map(|_| tx.allocate().unwrap()).collect();
        tx.commit_deferred().unwrap();
        let now = engine.info().size_bytes;
        if i == 1 {
            size = now;
        } else if i > 1 {
            assert_eq!(now, size, "commit {i}");
        }
    }
    drop(read);
}

/// AC-F19 (F3, F10): a store truncated while closed before its frontier counts no pages after the frontier, and a
/// map page lost to the truncation is damage.
#[test]
fn ac_f19_a_store_truncated_before_its_frontier() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = small_disk(tmp.path());
    let mut tx = engine.write(None).unwrap();
    tx.allocate_run(SMALL - FIRST_PAGE).unwrap();
    let second = tx.allocate_run(SMALL - 1).unwrap();
    tx.allocate_run(SMALL - 1).unwrap();
    tx.commit().unwrap();
    free(&engine, &[second + 5, second + 9, second + 20]);
    engine.close().unwrap();
    let data = tmp.path().join(DATA_FILE);
    let bytes = fs::read(&data).unwrap();
    fs::write(&data, &bytes[..SMALL as usize * PAGE_SIZE]).unwrap();
    let engine = small_disk(tmp.path());
    assert_eq!(free_pages(&engine), 3);
    let mut tx = engine.write(None).unwrap();
    match tx.allocate() {
        Err(krummi_tx::Error::Damaged { page, .. }) => assert_eq!(page, Some(SMALL)),
        other => panic!("expected damage, got {other:?}"),
    }
}

/// F3 (verification probe): when the map page survives the truncation, a free page beyond the end of the file is
/// reserved and the file grows to hold it.
#[test]
fn a_free_page_beyond_a_truncated_end_is_reserved() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = small_disk(tmp.path());
    let mut tx = engine.write(None).unwrap();
    tx.allocate_run(SMALL - FIRST_PAGE).unwrap();
    let second = tx.allocate_run(SMALL - 1).unwrap();
    tx.allocate_run(SMALL - 1).unwrap();
    tx.commit().unwrap();
    free(&engine, &[second + 40, second + 41, second + 42]);
    engine.close().unwrap();
    let data = tmp.path().join(DATA_FILE);
    let bytes = fs::read(&data).unwrap();
    fs::write(&data, &bytes[..(SMALL as usize + 10) * PAGE_SIZE]).unwrap();
    let engine = small_disk(tmp.path());
    assert_eq!(free_pages(&engine), 3, "no pages after the frontier");
    let page = take(&engine, 1)[0];
    assert_eq!(page, second + 40);
    assert!(engine.info().page_count > page);
}

/// F10 (verification probe): the free pages of the information do not depend on which regions were read.
#[test]
fn free_pages_do_not_depend_on_the_regions_read() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = small_disk(tmp.path());
    let mut tx = engine.write(None).unwrap();
    tx.allocate_run(SMALL - FIRST_PAGE).unwrap();
    let a = tx.allocate_run(SMALL - 1).unwrap();
    let b = tx.allocate_run(SMALL - 1).unwrap();
    tx.commit().unwrap();
    free(&engine, &[a + 3, a + 4, a + 5, a + 6]);
    free(&engine, &[b + 10, b + 11]);
    let before = free_pages(&engine);
    engine.close().unwrap();
    let engine = small_disk(tmp.path());
    assert_eq!(engine.regions_read(), 0);
    assert_eq!(free_pages(&engine), before);
    let mut tx = engine.write(None).unwrap();
    tx.allocate_run(5).unwrap();
    assert!(engine.regions_read() >= 2, "the search read the regions");
    assert_eq!(free_pages(&engine), before, "an open write");
    tx.abort();
    assert_eq!(free_pages(&engine), before);
}

/// Precedence of the validation table (verification probe): a misuse recognized without the map wins over a
/// damaged map page; freeing a page of the damaged region reports the damage with the map page's number.
#[test]
fn misuse_known_without_the_map_wins_over_damage() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = small_disk(tmp.path());
    let mut tx = engine.write(None).unwrap();
    tx.allocate_run(SMALL - FIRST_PAGE).unwrap();
    let second = tx.allocate_run(SMALL - 1).unwrap();
    tx.commit().unwrap();
    free(&engine, &[second + 5]);
    engine.close().unwrap();
    damage(tmp.path(), SMALL);
    let engine = small_disk(tmp.path());
    let mut tx = engine.write(None).unwrap();
    assert_eq!(kind(tx.free(SMALL, 1)), ErrorKind::Misuse, "an own page");
    drop(tx);
    let mut tx = engine.write(None).unwrap();
    assert_eq!(
        kind(tx.free(second + 6, 10_000)),
        ErrorKind::Misuse,
        "past the frontier"
    );
    drop(tx);
    let mut tx = engine.write(None).unwrap();
    match tx.free(second + 5, 1) {
        Err(krummi_tx::Error::Damaged { page, .. }) => assert_eq!(page, Some(SMALL)),
        other => panic!("expected damage, got {other:?}"),
    }
    assert_eq!(kind(tx.commit()), ErrorKind::Damaged);
}

/// F3 (verification probe): a write that reserves a range and frees all of it still leaves a store that reaches
/// the frontier, and the range is reused whole.
#[test]
fn a_range_freed_by_the_write_that_reserved_it() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let mut tx = engine.write(None).unwrap();
    let first = tx.allocate_run(300).unwrap();
    tx.free(first, 300).unwrap();
    tx.commit().unwrap();
    let frontier = engine.frontier();
    engine.close().unwrap();
    let engine = disk(tmp.path());
    assert!(engine.info().page_count >= frontier);
    let mut tx = engine.write(None).unwrap();
    assert_eq!(tx.allocate_run(300).unwrap(), first);
    tx.commit().unwrap();
    assert_eq!(engine.frontier(), frontier);
}

/// F6 and G3 (verification probe): reads on three threads see consistent snapshots while the writer frees and
/// reuses pages in every commit, durable and deferred, and the store stays small.
#[test]
fn concurrent_reads_see_consistent_snapshots_while_pages_are_reused() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    // The root page holds a stamp and 8 page numbers; each of those pages holds the same stamp.
    let root = reserve(&engine, 1, "")[0];
    let stop = std::sync::atomic::AtomicBool::new(false);
    let field = |payload: &[u8], i: usize| {
        u64::from_le_bytes(payload[i * 8..i * 8 + 8].try_into().unwrap())
    };
    std::thread::scope(|s| {
        for _ in 0..3 {
            s.spawn(|| {
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    let read = engine.read().unwrap();
                    let payload = read.page(root).unwrap();
                    let stamp = field(payload, 0);
                    for i in (stamp > 0).then_some(1..9).into_iter().flatten() {
                        let page = field(payload, i);
                        assert_eq!(field(read.page(page).unwrap(), 0), stamp, "page {page}");
                    }
                }
            });
        }
        let mut set: Vec<u64> = Vec::new();
        for stamp in 1..=1500u64 {
            let mut tx = engine.write(None).unwrap();
            for &page in &set {
                tx.free(page, 1).unwrap();
            }
            set = (0..8).map(|_| tx.allocate().unwrap()).collect();
            for &page in &set {
                tx.page_mut(page).unwrap()[..8].copy_from_slice(&stamp.to_le_bytes());
            }
            let payload = tx.page_mut(root).unwrap();
            payload[..8].copy_from_slice(&stamp.to_le_bytes());
            for (i, page) in set.iter().enumerate() {
                payload[8 + i * 8..16 + i * 8].copy_from_slice(&page.to_le_bytes());
            }
            commit(tx, stamp % 3 == 0);
        }
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
    });
    assert!(engine.info().page_count < 64, "the pages are reused");
}

/// AC-F11, first part, and AC-F18 (verification probe): in a temporary store whose maximum size leaves room for
/// the versions of a write but not for growth, the free pages are reserved and one more fails.
#[test]
fn ac_f18_the_maximum_size_in_a_temporary_store() {
    let options = krummi_tx::OpenOptions {
        max_size: Some((128 + 6) * PAGE_SIZE as u64),
    };
    let engine = Engine::open_temporary(&options).unwrap();
    while engine.info().page_count < 128 || engine.frontier() < engine.info().page_count {
        let room = engine.info().page_count - engine.frontier();
        let mut tx = engine.write(None).unwrap();
        tx.allocate_run(room.clamp(1, 4)).unwrap();
        tx.commit().unwrap();
    }
    let pages: Vec<u64> = (FIRST_PAGE..FIRST_PAGE + 8).step_by(2).collect();
    free(&engine, &pages);
    let mut tx = engine.write(None).unwrap();
    let taken: BTreeSet<u64> = (0..4).map(|_| tx.allocate().unwrap()).collect();
    assert_eq!(taken, BTreeSet::from_iter(pages.iter().copied()));
    tx.commit().unwrap();
    free(&engine, &pages);
    let mut tx = engine.write(None).unwrap();
    for _ in 0..4 {
        tx.allocate().unwrap();
    }
    assert_eq!(kind(tx.allocate()), ErrorKind::MaxSize);
    assert_eq!(kind(tx.commit()), ErrorKind::MaxSize);
    common::set(&engine, FIRST_PAGE + 1, "still accepts writes");
}

/// J7 with AC-F11 and AC-F18 (the intermittent `ac_f18` above): the growth of a store with a maximum size does not
/// depend on whether versions no read needs were still waiting for the background transfer. With the transfer
/// paused, every commit leaves its versions waiting; growing must transfer them first, so the store reaches its
/// full room (128 pages) and a write of the free pages that fills the rest still fits.
#[test]
fn growth_within_the_maximum_size_ignores_versions_no_read_needs() {
    let options = krummi_tx::OpenOptions {
        max_size: Some((128 + 6) * PAGE_SIZE as u64),
    };
    let engine = Engine::open_temporary(&options).unwrap();
    engine.set_transfer_paused(true);
    let passes = engine.transfer_passes();
    while engine.info().page_count < 128 || engine.frontier() < engine.info().page_count {
        let room = engine.info().page_count - engine.frontier();
        let mut tx = engine.write(None).unwrap();
        tx.allocate_run(room.clamp(1, 4)).unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(
        engine.info().page_count,
        128,
        "the store took its full room"
    );
    // No background pass ran: the writer's own transfer before each growth made the room.
    assert_eq!(engine.transfer_passes(), passes);
    let pages: Vec<u64> = (FIRST_PAGE..FIRST_PAGE + 8).step_by(2).collect();
    free(&engine, &pages);
    let mut tx = engine.write(None).unwrap();
    for _ in 0..4 {
        tx.allocate().unwrap();
    }
    tx.commit().unwrap();
    engine.set_transfer_paused(false);
}

/// J5 and AC-J17 with a maximum size: when the pending versions do not limit the growth, a write that grows the
/// store does not wait for a running transfer.
#[test]
fn growth_with_ample_room_does_not_wait_for_a_running_transfer() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Barrier, mpsc};
    use std::time::Duration;
    let options = krummi_tx::OpenOptions {
        max_size: Some(64 << 20),
    };
    let engine = Engine::open_temporary(&options).unwrap();
    engine.set_transfer_paused(true);
    let mut tx = engine.write(None).unwrap();
    tx.allocate_run(engine.info().page_count - FIRST_PAGE)
        .unwrap();
    tx.commit().unwrap();
    assert!(engine.scratch_bytes() > 0, "versions wait for the transfer");
    let pages = engine.info().page_count;
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let (e, r) = (Arc::clone(&entered), Arc::clone(&release));
    let armed = Arc::new(AtomicBool::new(true));
    engine.set_transfer_hook(Some(Arc::new(move || {
        if armed.swap(false, Ordering::SeqCst) {
            e.wait();
            r.wait();
        }
    })));
    std::thread::scope(|s| {
        let held = s.spawn(|| engine.transfer_now());
        entered.wait();
        let (done, finished) = mpsc::channel();
        let engine = &engine;
        let writer = s.spawn(move || {
            let mut tx = engine.write(None).unwrap();
            tx.allocate().unwrap();
            tx.commit().unwrap();
            done.send(()).unwrap();
        });
        // Without the fix the write waits until the transfer is released, after this wait; with it, it ends in
        // milliseconds, so the long wait only guards slow machines.
        let completed = finished.recv_timeout(Duration::from_secs(30)).is_ok();
        release.wait();
        writer.join().unwrap();
        held.join().unwrap();
        assert!(
            completed,
            "the growing write finished while the transfer was held"
        );
    });
    assert!(engine.info().page_count > pages, "the store grew");
    engine.set_transfer_hook(None);
    engine.set_transfer_paused(false);
}

/// J7 and T14 with a read open while the store grows: the versions it needs still count, so a reservation that
/// does not fit fails with the maximum-size class and the store and those versions stay within the maximum; once
/// the read ends, the rest of the room is reserved.
#[test]
fn growth_within_the_maximum_size_counts_versions_a_read_needs() {
    let options = krummi_tx::OpenOptions {
        max_size: Some((128 + 6) * PAGE_SIZE as u64),
    };
    let max = options.max_size.unwrap();
    let engine = Engine::open_temporary(&options).unwrap();
    engine.set_transfer_paused(true);
    let fill = |engine: &Engine| -> Result<(), krummi_tx::Error> {
        while engine.info().page_count < 128 || engine.frontier() < engine.info().page_count {
            let room = engine.info().page_count - engine.frontier();
            let mut tx = engine.write(None).unwrap();
            tx.allocate_run(room.clamp(1, 4))?;
            tx.commit().unwrap();
        }
        Ok(())
    };
    // Grow to 64 pages, then keep the versions of three pages for a read.
    while engine.frontier() < 64 {
        let mut tx = engine.write(None).unwrap();
        tx.allocate_run(4).unwrap();
        tx.commit().unwrap();
    }
    let read = engine.read().unwrap();
    let mut tx = engine.write(None).unwrap();
    for page in FIRST_PAGE..FIRST_PAGE + 3 {
        common::put(&mut tx, page, "changed");
    }
    tx.commit().unwrap();
    let error = fill(&engine).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::MaxSize, "{error}");
    assert!(
        engine.info().size_bytes + engine.scratch_bytes() <= max,
        "the store and the versions it keeps stay within the maximum"
    );
    assert!(
        engine.frontier() < engine.info().page_count,
        "the read's versions took the room of the last pages"
    );
    drop(read);
    fill(&engine).unwrap();
    assert_eq!(
        engine.info().page_count,
        128,
        "the store took its full room"
    );
    engine.set_transfer_paused(false);
}

/// AC10 of the root PRD with free space: every altered byte of a store with freed pages and a map page gives the
/// damage class, "not a store" or no visible difference: reads never return different data and a reservation
/// never returns a page in use.
fn ac10_free_space_sweep(step: usize) {
    testing::skip_syncs(true);
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let engine = disk(&source);
    let pages: Vec<u64> = (0..6)
        .map(|i| reserve(&engine, 1, &format!("page {i}"))[0])
        .collect();
    free(&engine, &[pages[1], pages[3]]);
    engine.close().unwrap();
    let in_use = [0, 2, 4, 5].map(|i| (pages[i], format!("page {i}")));
    let pristine = fs::read(source.join(DATA_FILE)).unwrap();
    let copy = tmp.path().join("copy");
    fs::create_dir(&copy).unwrap();
    for offset in (0..pristine.len()).step_by(step) {
        let mut bytes = pristine.clone();
        bytes[offset] ^= 0xFF;
        fs::write(copy.join(DATA_FILE), &bytes).unwrap();
        let engine = match Engine::open(&copy, &common::options()) {
            Ok(engine) => engine,
            Err(error) => {
                let expected = [ErrorKind::Damaged, ErrorKind::NotAStore];
                assert!(expected.contains(&error.kind()), "byte {offset}: {error}");
                continue;
            }
        };
        let read = engine.read().unwrap();
        for (page, written) in &in_use {
            match read.page(*page) {
                Ok(payload) => assert_eq!(&text(payload), written, "byte {offset}"),
                Err(error) => assert_eq!(error.kind(), ErrorKind::Damaged, "byte {offset}"),
            }
        }
        drop(read);
        let mut tx = engine.write(None).unwrap();
        match tx.allocate() {
            Ok(page) => assert!(
                in_use.iter().all(|(p, _)| *p != page),
                "byte {offset}: {page}"
            ),
            Err(error) => assert_eq!(error.kind(), ErrorKind::Damaged, "byte {offset}"),
        }
    }
}

#[test]
fn ac10_sample_of_altered_bytes_with_free_space() {
    ac10_free_space_sweep(397);
}

#[test]
#[ignore = "acceptance: alters every byte of the store"]
fn ac10_every_altered_byte_with_free_space() {
    ac10_free_space_sweep(1);
}

/// A small reproducible random generator (xorshift64*), so a failing seed can be replayed.
struct Random(u64);

impl Random {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) % n
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

/// The pages in use and their contents, as the model says.
type Pages = std::collections::BTreeMap<u64, String>;

const MODEL_REGION: u64 = 128;
const MODEL_REGIONS: u64 = 4;

fn own(page: u64) -> bool {
    page < FIRST_PAGE || page.is_multiple_of(MODEL_REGION)
}

/// Checks the free pages of the information against the model: every page of the store that is neither in use
/// nor one of the store's own is free (pages from the frontier on were never used).
fn check_free_pages(engine: &Engine, model: &Pages, label: &str) {
    let info = engine.info();
    let expected = (FIRST_PAGE..info.page_count)
        .filter(|p| !own(*p) && !model.contains_key(p))
        .count() as u64;
    assert_eq!(info.free_pages, expected, "{label}");
}

/// What the random writes did: operations, and pages reserved and freed by the writes that committed.
#[derive(Default)]
struct Done {
    operations: usize,
    reserved: usize,
    freed: usize,
}

/// A random length of pages: one, or up to 64 for a third of them.
fn length(rng: &mut Random) -> u64 {
    if rng.chance(30) { 1 + rng.below(64) } else { 1 }
}

/// One random write: reservations, frees and changes, checked against the model, then committed or aborted.
/// Once the frontier is past three regions, it only reserves single pages the map can serve (pages before the
/// frontier that are neither in use, freed by this write nor the store's own), so the store stays within four.
fn random_write(
    engine: &Engine,
    rng: &mut Random,
    model: &mut Pages,
    stamp: &mut u64,
    done: &mut Done,
) {
    let mut tx = engine.write(None).unwrap();
    let mut pages = model.clone();
    let mut freed = BTreeSet::new();
    let (mut reserved, actions) = (0, 1 + rng.below(6) as usize);
    let frontier = engine.frontier();
    let full = frontier > (MODEL_REGIONS - 1) * MODEL_REGION;
    for _ in 0..actions {
        let choice = rng.below(100);
        let hole = |pages: &Pages, freed: &BTreeSet<u64>| {
            (FIRST_PAGE..frontier)
                .any(|p| !own(p) && !pages.contains_key(&p) && !freed.contains(&p))
        };
        if choice < 45 && (!full || hole(&pages, &freed)) {
            let count = if full { 1 } else { length(rng) };
            let first = tx.allocate_run(count).unwrap();
            // Pages freed by commits that are not durable yet are reusable too (F6): the map serves them.
            assert!(
                !full || first < frontier,
                "{first}: a full store reserves from the map"
            );
            let last = first + count - 1;
            assert_eq!(
                first / MODEL_REGION,
                last / MODEL_REGION,
                "a range crosses regions"
            );
            for page in first..first + count {
                assert!(!own(page), "{page} is one of the store's own");
                assert!(!freed.contains(&page), "{page} was freed by this write");
                *stamp += 1;
                let value = format!("p{page}s{stamp}");
                assert!(
                    pages.insert(page, value.clone()).is_none(),
                    "{page} was in use"
                );
                assert!(tx.page(page).unwrap().iter().all(|&b| b == 0));
                put(&mut tx, page, &value);
            }
            reserved += count as usize;
        } else if choice < 80 && !pages.is_empty() {
            let index = rng.below(pages.len() as u64) as usize;
            let first = *pages.keys().nth(index).unwrap();
            let longest = length(rng);
            let mut count = 1;
            while count < longest && pages.contains_key(&(first + count)) {
                count += 1;
            }
            tx.free(first, count).unwrap();
            for page in first..first + count {
                pages.remove(&page);
                freed.insert(page);
            }
        } else if !pages.is_empty() {
            let index = rng.below(pages.len() as u64) as usize;
            let page = *pages.keys().nth(index).unwrap();
            assert_eq!(text(tx.page(page).unwrap()), pages[&page]);
            *stamp += 1;
            let value = format!("p{page}s{stamp}");
            put(&mut tx, page, &value);
            pages.insert(page, value);
        }
    }
    done.operations += actions;
    if rng.below(10) < 2 {
        tx.abort();
        return;
    }
    commit(tx, rng.below(8) >= 2);
    *model = pages;
    done.reserved += reserved;
    done.freed += freed.len();
}

/// AC-F16 (F2, F4, F5, F6, F10): a reproducible random sequence of at least 100,000 reservations of single pages
/// and ranges, frees, changes, commits in both modes and aborts, with reads held across it and simulated crashes,
/// matches a model of the pages in use: no page in use is ever reserved, no range takes one of the store's own
/// pages, every read sees its snapshot, and the free pages of the information match the model.
#[test]
fn ac_f16_sample_of_random_operations() {
    model_run(20_000);
}

#[test]
#[ignore = "acceptance: the 100,000 operations of AC-F16"]
fn ac_f16_random_operations_match_a_model() {
    model_run(100_000);
}

/// The run of AC-F16 with `operations` operations, a simulated crash every 10,000.
fn model_run(operations: usize) {
    const PER_CRASH: usize = 10_000;
    let mut rng = Random(0x5EED_F4EE_5BAC_E001);
    let mut model = Pages::new();
    let mut stamp = 0;
    let mut done = Done::default();
    let mut folder = tempfile::tempdir().unwrap();
    while done.operations < operations {
        let start = common::copy_folder(folder.path());
        testing::reset();
        // The model checks free space, not durability: every crash comes after making everything durable.
        testing::skip_syncs(true);
        testing::start_recording();
        let engine = disk(folder.path());
        engine.set_region_pages(MODEL_REGION);
        for (page, value) in &model {
            assert_eq!(&common::read_text(&engine, *page), value, "after reopening");
        }
        check_free_pages(&engine, &model, "after reopening");
        let mut reads: Vec<(krummi_tx::ReadTransaction<'_>, Pages)> = Vec::new();
        let mut segment = 0;
        while segment < PER_CRASH && done.operations < operations {
            let before = done.operations;
            random_write(&engine, &mut rng, &mut model, &mut stamp, &mut done);
            segment += done.operations - before;
            if rng.chance(5) {
                engine.make_durable().unwrap();
                check_free_pages(&engine, &model, &format!("operation {}", done.operations));
                if reads.len() < 4 {
                    reads.push((engine.read().unwrap(), model.clone()));
                }
                if rng.chance(30) && !reads.is_empty() {
                    let (read, seen) = reads.remove(rng.below(reads.len() as u64) as usize);
                    for (page, value) in &seen {
                        assert_eq!(&text(read.page(*page).unwrap()), value, "a read's snapshot");
                    }
                }
            }
        }
        engine.make_durable().unwrap();
        drop(reads);
        let ops = testing::take_recording();
        testing::reset();
        let crashed = common::copy_folder(start.path());
        testing::materialize(
            &ops,
            ops.len(),
            folder.path(),
            crashed.path(),
            Model::Written,
        );
        drop(engine);
        folder = crashed;
    }
    testing::reset();
    // The sequence keeps reserving and freeing pages to the end, not only until the store fills.
    assert!(
        done.reserved > operations / 4 && done.freed > operations / 4,
        "reserved {} and freed {} pages in {} operations",
        done.reserved,
        done.freed,
        done.operations
    );
}
