//! Free space (free-space PRD, F1–F14): reservations reuse freed pages before the store grows, ranges, the
//! frontier and its regions, misuse, and the free pages of the store information.

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use common::{both, disk, put, text};
use krummi_pager::DATA_FILE;
use krummi_pager::format::{PAGE_HEADER_SIZE, PAGE_SIZE};
use krummi_pager::testing::{self, Action, Op};
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

/// AC-F3 (F6, T14, E3): reads opened before a page is freed and reused keep seeing its earlier content, through
/// several generations of the same page number; new reads, and the store after reopening, see the newest.
#[test]
fn ac_f3_old_reads_keep_their_version_of_a_reused_page() {
    for deferred in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let engine = disk(tmp.path());
        let page = reserve(&engine, 2, "gen0")[0];
        let mut reads = Vec::new();
        for generation in 1..=3 {
            reads.push(engine.read().unwrap());
            free(&engine, &[page]);
            let mut tx = engine.write(None).unwrap();
            assert_eq!(tx.allocate().unwrap(), page, "the only free page");
            put(&mut tx, page, &format!("gen{generation}"));
            if deferred {
                tx.commit_deferred().unwrap();
                engine.make_durable().unwrap();
            } else {
                tx.commit().unwrap();
            }
            assert_eq!(common::read_text(&engine, page), format!("gen{generation}"));
        }
        for (generation, read) in reads.iter().enumerate() {
            assert_eq!(text(read.page(page).unwrap()), format!("gen{generation}"));
        }
        drop(reads);
        drop(engine);
        assert_eq!(common::read_text(&disk(tmp.path()), page), "gen3");
    }
}

/// AC-F4 (F6, T10, T11): an aborted write, one dropped without committing, and one whose commit fails for lack of
/// space apply none of their frees and reservations.
#[test]
fn ac_f4_an_aborted_or_failed_write_frees_and_reserves_nothing() {
    for ending in ["abort", "drop", "disk full"] {
        let tmp = tempfile::tempdir().unwrap();
        let engine = disk(tmp.path());
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
    (frontier..page_count).filter(|p| p % region != 0).count() as u64
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

/// AC-F9 (F5, F8, T11): each misuse fails with the misuse class and no effect, the write can no longer be
/// committed, and none of its earlier changes apply; a range as long as the cap is reserved without error.
#[test]
fn ac_f9_misuse_fails_and_spoils_the_write() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = small_disk(tmp.path());
    // Two free pages: the write's own reservation takes the first, the misuse frees the second.
    let used = reserve(&engine, 4, "used");
    free(&engine, &used[2..]);
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
    for (name, misuse) in cases {
        let free_before = free_pages(&engine);
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
        assert_eq!(common::read_text(&engine, used[0]), "used", "{name}");
        assert_eq!(common::read_text(&engine, used[1]), "used", "{name}");
        assert_eq!(free_pages(&engine), free_before, "{name}");
    }
    common::set(&engine, used[0], "the next write works");
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

/// AC-F10 (F4, F8): freeing part of a reserved range, a page reserved in the same write and a page changed in it.
#[test]
fn ac_f10_freeing_parts_of_ranges_and_changed_pages() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let changed = reserve(&engine, 1, "before")[0];
    let mut tx = engine.write(None).unwrap();
    put(&mut tx, changed, "after");
    let range = tx.allocate_run(8).unwrap();
    let single = tx.allocate().unwrap();
    tx.free(range + 2, 3).unwrap();
    tx.free(changed, 1).unwrap();
    tx.free(single, 1).unwrap();
    tx.commit().unwrap();
    drop(engine);
    let engine = disk(tmp.path());
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
    assert_eq!(BTreeSet::from_iter(take(&engine, 5)), expected);
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
    for page in (FIRST_PAGE..engine.frontier()).filter(|&p| p != SMALL && p % SMALL != 0) {
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
