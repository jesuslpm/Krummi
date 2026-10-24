//! Time to reserve a page and ranges of 16, 256 and 4,096 pages in a store of at least 1 million pages with half
//! of them freed in loose holes, and time to open that store (decision F13 of the free-space PRD, checked by
//! criterion AC-F20).
//!
//! ```text
//! cargo run --release -p krummi-tx --example free_space_timing [folder] [pages]
//! ```
//!
//! It builds the store in a temporary folder inside `folder` (the system's temporary folder by default), with
//! `pages` pages (1,048,576 by default, 8 GiB on disk), frees every other page, closes it, and measures on fresh
//! openings. "First" is the first reservation after opening, which reads the regions' maps it needs; "next" is the
//! mean of the following ones, each in its own write, aborted so the store does not change. The loose holes
//! serve single pages; no run of 16 pages is free, so ranges take the frontier after checking every region. It
//! prints Markdown tables. There is no threshold (D5).

use std::path::Path;
use std::time::{Duration, Instant};

use krummi_tx::{Engine, OpenOptions, REGION_PAGES};

/// Reservations measured after the first one.
const REPEATS: u32 = 100;

fn main() {
    let mut args = std::env::args().skip(1);
    let base = args.next().map_or_else(std::env::temp_dir, Into::into);
    let pages: u64 = args.next().map_or(1 << 20, |s| s.parse().expect("pages"));
    let folder = tempfile::tempdir_in(&base).expect("a temporary folder");
    let built = Instant::now();
    build(folder.path(), pages);
    eprintln!("built in {:.1} s", built.elapsed().as_secs_f64());

    let (engine, opening) = timed(|| open(folder.path()));
    let info = engine.info();
    drop(engine);
    println!("| Pages | Free pages | Open (ms) |");
    println!("|---|---|---|");
    println!(
        "| {} | {} | {:.2} |",
        info.page_count,
        info.free_pages,
        ms(opening)
    );
    println!();
    println!("| Reservation | First (µs) | Next, mean (µs) |");
    println!("|---|---|---|");
    for count in [1, 16, 256, 4096] {
        let engine = open(folder.path());
        let first = reserve(&engine, count);
        let next = (0..REPEATS)
            .map(|_| reserve(&engine, count))
            .sum::<Duration>()
            / REPEATS;
        let label = if count == 1 {
            "1 page".to_string()
        } else {
            format!("range of {count}")
        };
        println!("| {label} | {:.1} | {:.1} |", us(first), us(next));
    }
}

fn open(folder: &Path) -> Engine {
    Engine::open(folder, &OpenOptions::default()).expect("open")
}

/// Fills a new store with `pages` pages in ranges, then frees every other one.
fn build(folder: &Path, pages: u64) {
    let engine = open(folder);
    while engine.frontier() < pages {
        // Up to the end of the frontier's region, so no pages are skipped (and left free) at its end.
        let offset = engine.frontier() % REGION_PAGES;
        let room = if offset == 0 {
            REGION_PAGES - 1
        } else {
            REGION_PAGES - offset
        };
        let mut tx = engine.write(None).expect("write");
        tx.allocate_run(room.min(8192)).expect("reserve");
        tx.commit_deferred().expect("commit");
    }
    let frontier = engine.frontier();
    let mut page = krummi_tx::FIRST_PAGE;
    while page < frontier {
        let mut tx = engine.write(None).expect("write");
        let mut freed = 0;
        while page < frontier && freed < 16_384 {
            if !page.is_multiple_of(REGION_PAGES) {
                tx.free(page, 1).expect("free");
                freed += 1;
            }
            page += 2;
        }
        tx.commit_deferred().expect("commit");
    }
    engine.close().expect("close");
}

/// The time of one reservation of `count` pages, in a write that is then aborted.
fn reserve(engine: &Engine, count: u64) -> Duration {
    let mut tx = engine.write(None).expect("write");
    let (_, elapsed) = timed(|| tx.allocate_run(count).expect("reserve"));
    tx.abort();
    elapsed
}

fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let value = f();
    (value, start.elapsed())
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn us(d: Duration) -> f64 {
    d.as_secs_f64() * 1e6
}
