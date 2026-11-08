//! Recovery of the journal that keeps the bytes each commit changes in a page (the journal page diffs PRD, AC-Y2 to
//! AC-Y4), through `Engine::open` after a simulated crash: what it recovers, byte for byte, after a crash, at every
//! sampled cut and with damaged pages. The workloads come from fixed seeds.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use common::{POWER_CUTS, Run, disk};
use krummi_pager::DATA_FILE;
use krummi_pager::format::{HEADER_PAGES, PAGE_SIZE};
use krummi_pager::testing::{self, Model};
use krummi_tx::{Engine, ErrorKind, OpenOptions};

/// SplitMix-like xorshift: a small, deterministic generator.
struct Rng(u64);

impl Rng {
    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % bound as u64) as usize
    }
}

/// What a workload leaves: the payload of each page of its own and the content of each value run, by first page.
#[derive(Clone, Default, PartialEq, Debug)]
struct State {
    pages: BTreeMap<u64, Vec<u8>>,
    runs: BTreeMap<u64, (u64, Vec<u8>)>,
}

/// Writes a few runs of non-zero bytes into `bytes`, now and then a wide one.
fn scribble(rng: &mut Rng, bytes: &mut [u8], max_runs: usize) {
    for _ in 0..1 + rng.below(max_runs) {
        let widest = if rng.below(8) == 0 { 3000 } else { 120 };
        let len = 1 + rng.below(widest);
        let at = rng.below(bytes.len().saturating_sub(len).max(1));
        let end = (at + len).min(bytes.len());
        for b in &mut bytes[at..end] {
            *b = 1 + rng.below(255) as u8;
        }
    }
}

/// Makes `commits` commits of a seeded workload that creates pages, some left all zeros, changes a few bytes of
/// others, frees and reuses pages, and writes and frees value runs. Returns the state after each commit (index 0 is
/// the empty store) and the recorded operations when each commit returned (index 0 is the start).
fn workload(engine: &Engine, commits: usize, seed: u64) -> (Vec<State>, Vec<usize>) {
    let mut rng = Rng(seed);
    let mut current = State::default();
    let (mut states, mut returned) = (vec![current.clone()], vec![testing::recorded_len()]);
    for commit in 1..=commits {
        let mut tx = engine.write(None).unwrap();
        let mut freed = Vec::new();
        for _ in 0..if commit == 1 { 6 } else { 1 + rng.below(4) } {
            let choice = if commit == 1 { 0 } else { rng.below(10) };
            match choice {
                0 | 1 => {
                    let page = tx.allocate().unwrap();
                    let payload = tx.page_mut(page).unwrap();
                    if commit == 1 || rng.below(5) != 0 {
                        scribble(&mut rng, payload, 3);
                    }
                    current.pages.insert(page, payload.to_vec());
                }
                2 if !current.pages.is_empty() => {
                    let pages: Vec<u64> = current.pages.keys().copied().collect();
                    let page = pages[rng.below(pages.len())];
                    if !freed.contains(&page) {
                        tx.free(page, 1).unwrap();
                        current.pages.remove(&page);
                        freed.push(page);
                    }
                }
                3 => {
                    let count = 1 + rng.below(3) as u64;
                    let (first, body) = tx.allocate_value_run(count).unwrap();
                    scribble(&mut rng, body, 4);
                    current.runs.insert(first, (count, body.to_vec()));
                }
                4 if !current.runs.is_empty() => {
                    let runs: Vec<u64> = current.runs.keys().copied().collect();
                    let first = runs[rng.below(runs.len())];
                    let (count, _) = current.runs.remove(&first).unwrap();
                    tx.free(first, count).unwrap();
                }
                _ if !current.pages.is_empty() => {
                    let pages: Vec<u64> = current.pages.keys().copied().collect();
                    let page = pages[rng.below(pages.len())];
                    let payload = tx.page_mut(page).unwrap();
                    scribble(&mut rng, payload, 5);
                    current.pages.insert(page, payload.to_vec());
                }
                _ => {}
            }
        }
        tx.commit().unwrap();
        returned.push(testing::recorded_len());
        states.push(current.clone());
    }
    (states, returned)
}

/// Whether `engine` holds exactly the pages and runs of `state`.
fn holds(engine: &Engine, state: &State) -> bool {
    let read = engine.read().unwrap();
    state
        .pages
        .iter()
        .all(|(&page, payload)| read.page(page).is_ok_and(|seen| seen == &payload[..]))
        && state.runs.iter().all(|(&first, (count, body))| {
            read.run(first, *count).is_ok_and(|seen| seen == &body[..])
        })
}

/// AC-Y2 (Y1, Y2): after a crash with nothing transferred, the reopened store holds the last committed version of
/// every page and run, and every page of `krummi.data` but the header's equals what the same sequence leaves with a
/// clean close.
#[test]
fn ac_y2_recovery_gives_the_pages_of_a_clean_close() {
    for seed in 1..=6u64 {
        let (source, engine) = Run::start();
        let hold = engine.read().unwrap();
        let (states, returned) = workload(&engine, 60, 0x6B72_0000 + seed);
        let run = Run::finish(source, returned);
        drop(hold);
        let crashed = run.killed();
        engine.close().unwrap();
        let recovered = disk(crashed.path());
        assert!(holds(&recovered, states.last().unwrap()), "seed {seed}");
        recovered.close().unwrap();
        let clean = fs::read(run.source.path().join(DATA_FILE)).unwrap();
        let after = fs::read(crashed.path().join(DATA_FILE)).unwrap();
        let common = clean.len().min(after.len());
        let differing: Vec<usize> = (HEADER_PAGES as usize..common / PAGE_SIZE)
            .filter(|&page| {
                clean[page * PAGE_SIZE..][..PAGE_SIZE] != after[page * PAGE_SIZE..][..PAGE_SIZE]
            })
            .collect();
        assert!(
            differing.is_empty(),
            "seed {seed}: pages {differing:?} differ"
        );
        assert!(
            clean[common..]
                .iter()
                .chain(&after[common..])
                .all(|&b| b == 0),
            "seed {seed}"
        );
    }
}

/// AC-Y3 (Y2) through the opening: power cuts and cut transfers at about 40 points of one run recover a prefix of
/// its commits, and the store then reopens clean.
#[test]
fn ac_y3_cuts_at_sampled_points_recover_a_prefix_of_the_commits() {
    cuts_recover_a_prefix(1..=1, 40);
}

/// AC-Y3 at about 120 points of three runs (`cargo test -p krummi-tx --test page_diffs -- --ignored`).
#[test]
#[ignore = "acceptance: about 1,400 reopenings"]
fn ac_y3_cuts_at_many_points_recover_a_prefix_of_the_commits() {
    cuts_recover_a_prefix(1..=3, 120);
}

/// Runs the workload of each seed, then reopens it after a crash at about `points` cuts with every model.
fn cuts_recover_a_prefix(seeds: std::ops::RangeInclusive<u64>, points: usize) {
    for seed in seeds {
        let (source, engine) = Run::start();
        let (states, returned) = workload(&engine, 40, 0x7B72_0000 + seed);
        let run = Run::finish(source, returned);
        engine.close().unwrap();
        let total = run.ops.len();
        for cut in (0..=total).step_by((total / points).max(1)) {
            for model in POWER_CUTS.into_iter().chain([Model::Written]) {
                let crashed = run.crash(cut, model);
                if !crashed.path().join(DATA_FILE).exists() {
                    continue;
                }
                let engine = Engine::open(crashed.path(), &OpenOptions::default())
                    .unwrap_or_else(|e| panic!("seed {seed}, cut {cut}, {model:?}: {e:?}"));
                // The commit that had returned by the cut, or the one in flight.
                let returned = run.returned_at(cut).saturating_sub(1);
                assert!(
                    (returned..=(returned + 1).min(states.len() - 1))
                        .any(|n| holds(&engine, &states[n])),
                    "seed {seed}, cut {cut} of {total}, {model:?}: not commit {returned} or the next"
                );
                engine.close().unwrap();
                disk(crashed.path()).close().unwrap();
            }
        }
    }
}

/// Commits whose copied page changed no byte journal no entry for it, down to records with none, and every cut of
/// such a run recovers one of the committed texts (Y1).
#[test]
fn records_without_entries_recover_like_the_others() {
    let (source, engine) = Run::start();
    let mut tx = engine.write(None).unwrap();
    let page = tx.allocate().unwrap();
    common::put(&mut tx, page, "a");
    tx.commit().unwrap();
    let hold = engine.read().unwrap();
    let mut returned = vec![testing::recorded_len()];
    let texts = ["a", "a", "b", "b", "b", "c", "a"];
    for text in texts {
        let mut tx = engine.write(None).unwrap();
        common::put(&mut tx, page, text);
        tx.commit().unwrap();
        returned.push(testing::recorded_len());
    }
    let run = Run::finish(source, returned);
    drop(hold);
    let crashed = run.killed();
    let empty = journal_records(crashed.path())
        .iter()
        .filter(|&&(entries, _)| entries == 0)
        .count();
    assert!(empty > 0, "some record has no entry");
    let reopened = disk(crashed.path());
    assert_eq!(common::read_text(&reopened, page), "a");
    reopened.close().unwrap();
    // The text after commit `n` of the run: the first commit wrote "a", the later ones `texts`.
    let text_after = |n: usize| match n {
        0 => None,
        1 => Some("a".to_string()),
        n => Some(texts[n - 2].to_string()),
    };
    for cut in 0..=run.ops.len() {
        for model in POWER_CUTS.into_iter().chain([Model::Written]) {
            let crashed = run.crash(cut, model);
            if !crashed.path().join(DATA_FILE).exists() {
                continue;
            }
            let engine = Engine::open(crashed.path(), &OpenOptions::default())
                .unwrap_or_else(|e| panic!("cut {cut}, {model:?}: {e:?}"));
            let seen = engine
                .read()
                .ok()
                .and_then(|read| read.page(page).ok().map(common::text));
            let returned = run.returned_at(cut);
            let allowed: Vec<Option<String>> = [returned, returned + 1]
                .into_iter()
                .filter(|&n| n <= texts.len() + 1)
                .map(text_after)
                .collect();
            assert!(
                allowed.contains(&seen) || seen.is_none() && returned == 0,
                "cut {cut}, {model:?}: {seen:?}, allowed {allowed:?}"
            );
            engine.close().unwrap();
        }
    }
}

/// The entry count and stored length of every record of the journal files in `folder`.
fn journal_records(folder: &Path) -> Vec<(u32, usize)> {
    let mut records = Vec::new();
    for entry in fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if !path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("krummi.journal.")
        {
            continue;
        }
        let bytes = fs::read(&path).unwrap();
        let mut at = 0;
        while at + 64 <= bytes.len() && bytes[at..at + 8] == *b"KrummiJ2" {
            let entries = u32::from_le_bytes(bytes[at + 24..at + 28].try_into().unwrap());
            let stored = u64::from_le_bytes(bytes[at + 32..at + 40].try_into().unwrap()) as usize;
            records.push((entries, stored));
            at += (64 + stored).next_multiple_of(4096);
        }
    }
    records
}

/// AC-Y4 (Y3) through the opening: bytes that no pending entry holds, altered in one, two or three pages of
/// `krummi.data`, make the opening fail with the damage class and the lowest of those pages; the header does not
/// change, and a second opening fails the same way.
#[test]
fn ac_y4_a_page_damaged_outside_its_entries_fails_the_opening_with_the_lowest() {
    for damaged in [vec![0usize, 1, 2], vec![1, 2], vec![2]] {
        let (source, engine) = Run::start();
        let mut tx = engine.write(None).unwrap();
        let pages: Vec<u64> = (0..3).map(|_| tx.allocate().unwrap()).collect();
        for &page in &pages {
            common::put(&mut tx, page, "c0");
        }
        tx.commit().unwrap();
        engine.checkpoint().unwrap();
        let hold = engine.read().unwrap();
        for _ in 0..2 {
            let mut tx = engine.write(None).unwrap();
            for &page in &pages {
                common::put(&mut tx, page, "c1");
            }
            tx.commit().unwrap();
        }
        let run = Run::finish(source, vec![]);
        drop(hold);
        let crashed = run.killed();
        let data = crashed.path().join(DATA_FILE);
        let mut bytes = fs::read(&data).unwrap();
        // `put` changes the first bytes of the payload; byte 5,000 of the page is in no entry.
        for &i in &damaged {
            bytes[pages[i] as usize * PAGE_SIZE + 5_000 + i] ^= 0x01;
        }
        fs::write(&data, &bytes).unwrap();
        let header = || fs::read(&data).unwrap()[..HEADER_PAGES as usize * PAGE_SIZE].to_vec();
        let before = header();
        let first = Engine::open(crashed.path(), &OpenOptions::default())
            .err()
            .unwrap_or_else(|| panic!("{damaged:?}: the opening succeeded"));
        assert_eq!(first.kind(), ErrorKind::Damaged, "{damaged:?}");
        match &first {
            krummi_tx::Error::Damaged { page, .. } => {
                assert_eq!(*page, Some(pages[damaged[0]]), "{damaged:?}")
            }
            other => panic!("{damaged:?}: {other:?}"),
        }
        assert_eq!(header(), before, "{damaged:?}: the header changed");
        let second = Engine::open(crashed.path(), &OpenOptions::default())
            .err()
            .unwrap_or_else(|| panic!("{damaged:?}: the second opening succeeded"));
        assert_eq!(format!("{second:?}"), format!("{first:?}"), "{damaged:?}");
    }
}
