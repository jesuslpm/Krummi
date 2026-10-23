//! Durable commits per second with 1, 4 and 16 threads committing at once (E16 of the grouped journal PRD).
//!
//! Run it twice, with direct journal writes and with the fallback through the system's cache:
//!
//! ```text
//! cargo run --release -p krummi-tx --example commit_rate [folder] [seconds]
//! KRUMMI_JOURNAL_BUFFERED=1 cargo run --release -p krummi-tx --example commit_rate [folder] [seconds]
//! ```
//!
//! Each run opens a new store in a temporary folder inside `folder` (the system's temporary folder by default),
//! with journal files of the release size, and lets every thread commit one page of its own, durably, for
//! `seconds` (3 by default). It prints one Markdown table row per thread count. There is no threshold (D5).

use std::time::{Duration, Instant};

use krummi_pager::RELEASE_FILE_LIMIT;
use krummi_tx::{Engine, OpenOptions};

fn main() {
    let mut args = std::env::args().skip(1);
    let base = args.next().map_or_else(std::env::temp_dir, Into::into);
    let seconds: u64 = args.next().map_or(3, |s| s.parse().expect("seconds"));
    let mode = if std::env::var_os("KRUMMI_JOURNAL_BUFFERED").is_some() {
        "fallback"
    } else {
        "direct"
    };
    println!("| Journal writes | Threads | Commits | Seconds | Commits/s |");
    println!("|---|---|---|---|---|");
    for threads in [1usize, 4, 16] {
        let folder = tempfile::tempdir_in(&base).expect("a temporary folder");
        let engine = Engine::open(folder.path(), &OpenOptions::default()).expect("open");
        engine.set_journal_file_limit(RELEASE_FILE_LIMIT);
        let pages: Vec<u64> = {
            let mut tx = engine.write(None).expect("write");
            let pages = (0..threads)
                .map(|_| tx.allocate().expect("allocate"))
                .collect();
            tx.commit().expect("commit");
            pages
        };
        let limit = Duration::from_secs(seconds);
        let start = Instant::now();
        let commits: u64 = std::thread::scope(|s| {
            let handles: Vec<_> = pages
                .iter()
                .map(|&page| {
                    let engine = &engine;
                    s.spawn(move || {
                        let mut done = 0u64;
                        while start.elapsed() < limit {
                            let mut tx = engine.write(None).expect("write");
                            tx.page_mut(page).expect("page")[..8]
                                .copy_from_slice(&done.to_le_bytes());
                            tx.commit().expect("commit");
                            done += 1;
                        }
                        done
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().expect("thread")).sum()
        });
        let elapsed = start.elapsed().as_secs_f64();
        engine.close().expect("close");
        println!(
            "| {mode} | {threads} | {commits} | {elapsed:.2} | {:.0} |",
            commits as f64 / elapsed
        );
    }
}
