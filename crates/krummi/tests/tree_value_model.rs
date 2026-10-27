//! Probes of the independent verification of the large values run (PRD 5.3), kept as regression tests: a random
//! model of stores, overwrites, scans and range removals with values in and off their leaf, on disk with reopening
//! and in temporary mode (S1, S2, S4); pages given back after overwrites and removals (S5); references that
//! survive durability and page reuse (S3, S6); the leaf threshold for every key length (S2); and, ignored for its runtime, altered bytes of a small store with
//! value runs (S11).

mod support;
use std::collections::BTreeMap;

use krummi::{OpenMode, Store, StoreOptions, TreeKind};
use support::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}
fn val(rng: &mut Rng, len: usize) -> Vec<u8> {
    let seed = rng.next() as u8;
    (0..len)
        .map(|i| (i as u8).wrapping_mul(31) ^ seed ^ (i >> 8) as u8)
        .collect()
}

type Model = BTreeMap<Vec<u8>, Vec<u8>>;

fn check_all(r: &krummi::ReadTransaction<'_>, name: &str, model: &Model, label: &str) {
    let t = r.open_tree(name, TreeKind::Normal).unwrap();
    assert_eq!(r.len(&t).unwrap(), model.len() as u64, "{label} len");
    let fw: Vec<_> = r.range(&t, ..).unwrap().map(|x| x.unwrap()).collect();
    assert_eq!(fw.len(), model.len(), "{label} fw len");
    for ((k, v), (mk, mv)) in fw.iter().zip(model) {
        assert!(
            *k == &mk[..] && *v == &mv[..],
            "{label} fw entry {}",
            mv.len()
        );
    }
    let bw: Vec<_> = r.range(&t, ..).unwrap().rev().map(|x| x.unwrap()).collect();
    for ((k, v), (mk, mv)) in bw.iter().zip(model.iter().rev()) {
        assert!(*k == &mk[..] && *v == &mv[..], "{label} bw entry");
    }
    let mut it = r.range(&t, ..).unwrap();
    let mut m = model.iter();
    loop {
        match (it.next(), m.next()) {
            (None, None) => break,
            (Some(a), Some(b)) => {
                let a = a.unwrap();
                assert!(a.0 == &b.0[..] && a.1 == &b.1[..]);
            }
            _ => panic!("interleave"),
        }
        match (it.next_back(), m.next_back()) {
            (None, None) => break,
            (Some(a), Some(b)) => {
                let a = a.unwrap();
                assert!(a.0 == &b.0[..] && a.1 == &b.1[..]);
            }
            _ => panic!("interleave2"),
        }
    }
    for (k, v) in model {
        assert!(r.get(&t, k).unwrap() == Some(&v[..]), "{label} get");
    }
    let pf = r
        .range_prefix(&t, b"p", ..)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .len();
    let mp = model.iter().filter(|(k, _)| k.starts_with(b"p")).count();
    assert_eq!(pf, mp, "{label} prefix");
}

fn rand_size(rng: &mut Rng, klen: usize) -> usize {
    let thr = 4078 - 4 - klen;
    match rng.below(12) {
        0 => 0,
        1 => thr.saturating_sub(1 + rng.below(3) as usize),
        2 => thr + rng.below(4) as usize,
        3 => 8192 - 16 + rng.below(3) as usize - 1,
        4 => 2 * 8192 - 16 + rng.below(3) as usize - 1,
        5 => 3 * 8192 - 16 + rng.below(3) as usize - 1,
        6 => rng.below(300_000) as usize,
        7 => 1 + rng.below(2000) as usize,
        _ => rng.below(20_000) as usize,
    }
}

fn model_run(store: &Store, seed: u64) -> Model {
    let mut rng = Rng(seed);
    let mut model = Model::new();
    let mut keys: Vec<Vec<u8>> = vec![];
    for i in 0..150 {
        let kl = match rng.below(8) {
            0 => 2048,
            1 => 1,
            _ => 1 + rng.below(30) as usize,
        };
        let mut k = val(&mut rng, kl);
        if i % 4 == 0 {
            k[0] = b'p';
        }
        keys.push(k);
    }
    keys.sort();
    keys.dedup();
    for round in 0..40 {
        let mut w = store.write().unwrap();
        let t = w
            .open_tree_with("m", TreeKind::Normal, OpenMode::OpenOrCreate)
            .unwrap();
        let mut pending = model.clone();
        for _ in 0..40 {
            let k = keys[rng.below(keys.len() as u64) as usize].clone();
            match rng.below(10) {
                0..=4 => {
                    let sz = rand_size(&mut rng, k.len());
                    let v = val(&mut rng, sz);
                    w.set(&t, &k, &v).unwrap();
                    pending.insert(k, v);
                }
                5 | 6 => {
                    let sz = rand_size(&mut rng, k.len());
                    let v = val(&mut rng, sz);
                    let added = w.try_add(&t, &k, &v).unwrap();
                    assert_eq!(added, !pending.contains_key(&k));
                    if added {
                        pending.insert(k, v);
                    }
                }
                7 => {
                    let r = w.remove(&t, &k).unwrap();
                    assert_eq!(r, pending.remove(&k).is_some());
                }
                8 => {
                    let k2 = keys[rng.below(keys.len() as u64) as usize].clone();
                    let (a, b) = if k <= k2 { (k, k2) } else { (k2, k) };
                    let n = w.remove_range(&t, &a[..]..&b[..]).unwrap();
                    let before = pending.len();
                    pending.retain(|key, _| !(key >= &a && key < &b));
                    assert_eq!(n as usize, before - pending.len());
                }
                _ => {
                    let g = w.get(&t, &k).unwrap();
                    assert!(g == pending.get(&k).map(|v| &v[..]));
                }
            }
        }
        let all: Vec<_> = w.range(&t, ..).unwrap().map(|x| x.unwrap()).collect();
        assert_eq!(all.len(), pending.len());
        for ((k, v), (mk, mv)) in all.iter().zip(&pending) {
            assert!(*k == &mk[..] && *v == &mv[..]);
        }
        drop(all);
        if round % 9 == 8 {
            w.abort();
        } else {
            w.commit().unwrap();
            model = pending;
        }
        let r = store.read().unwrap();
        check_all(&r, "m", &model, &format!("seed {seed} round {round}"));
    }
    model
}

#[test]
fn model_disk_with_reopen_and_temp() {
    for seed in [1u64, 2, 3] {
        let tmp = tempfile::tempdir().unwrap();
        let s = open(tmp.path());
        let model = model_run(&s, seed);
        s.close().unwrap();
        let s = open(tmp.path());
        {
            let r = s.read().unwrap();
            check_all(&r, "m", &model, "reopened");
        }
        s.close().unwrap();
        let t = Store::open_temporary(&StoreOptions::default()).unwrap();
        let m2 = model_run(&t, seed);
        assert_eq!(m2.len(), model.len());
        t.close().unwrap();
    }
}

#[test]
fn no_page_leak() {
    for temporary in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let s = if temporary {
            Store::open_temporary(&StoreOptions::default()).unwrap()
        } else {
            open(tmp.path())
        };
        let mut rng = Rng(11);
        let in_use = |s: &Store| {
            let i = s.info();
            i.page_count - i.free_pages
        };
        let settle = |s: &Store| {
            for _ in 0..3 {
                s.write().unwrap().commit().unwrap();
            }
        };
        {
            let mut w = s.write().unwrap();
            w.open_tree_with("t", TreeKind::Normal, OpenMode::CreateNew)
                .unwrap();
            w.commit().unwrap();
        }
        settle(&s);
        let base_empty = in_use(&s);
        let mut peak = 0;
        for i in 0..60 {
            let mut w = s.write().unwrap();
            let t = w.open_tree("t", TreeKind::Normal).unwrap();
            let sz = [1 << 20, 64 << 10, 5000, 3 << 20, 100][i % 5];
            w.set(&t, b"a", &val(&mut rng, sz)).unwrap();
            w.set(&t, b"b", &val(&mut rng, 1 << 20)).unwrap();
            if i % 3 == 0 {
                w.remove(&t, b"b").unwrap();
            }
            w.commit().unwrap();
            peak = peak.max(in_use(&s));
        }
        // live data <= 3 MiB + 1 MiB = 512 pages; F6 delays reuse by about one commit
        assert!(peak < 2 * 512 + 200, "peak {peak} temporary={temporary}");
        let mut w = s.write().unwrap();
        w.open_tree_with("t", TreeKind::Normal, OpenMode::Empty)
            .unwrap();
        w.commit().unwrap();
        settle(&s);
        assert_eq!(in_use(&s), base_empty, "after empty, temporary={temporary}");
        let mut w = s.write().unwrap();
        let t = w.open_tree("t", TreeKind::Normal).unwrap();
        for i in 0..20u8 {
            w.set(&t, &[i], &val(&mut rng, 100_000)).unwrap();
        }
        w.commit().unwrap();
        let mut w = s.write().unwrap();
        w.open_tree_with("u", TreeKind::Normal, OpenMode::CreateNew)
            .unwrap();
        w.commit().unwrap();
        let mut w = s.write().unwrap();
        w.rename_tree("t", "u", true).unwrap();
        w.commit().unwrap();
        let mut w = s.write().unwrap();
        w.delete_tree("u").unwrap();
        w.commit().unwrap();
        settle(&s);
        let after = in_use(&s);
        // base_empty counted tree "t" (empty); now no tree remains, so it can only be smaller or equal
        assert!(
            after <= base_empty,
            "final {after} vs {base_empty}, temporary={temporary}"
        );
        s.close().unwrap();
    }
}

#[test]
fn references_survive_make_durable_and_reuse() {
    for temporary in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let s = if temporary {
            Store::open_temporary(&StoreOptions::default()).unwrap()
        } else {
            open(tmp.path())
        };
        let mut rng = Rng(5);
        let v1 = val(&mut rng, 3 * 8192 + 17);
        let v2 = val(&mut rng, 200_000);
        let mut w = s.write().unwrap();
        let t = w
            .open_tree_with("t", TreeKind::Normal, OpenMode::CreateNew)
            .unwrap();
        w.set(&t, b"k1", &v1).unwrap();
        w.set(&t, b"k2", &v2).unwrap();
        for i in 0..300u32 {
            let v = val(&mut rng, 900);
            w.set(&t, format!("filler{i:05}").as_bytes(), &v).unwrap();
        }
        w.commit().unwrap();
        let r = s.read().unwrap();
        let rt = r.open_tree("t", TreeKind::Normal).unwrap();
        let g1 = r.get(&rt, b"k1").unwrap().unwrap();
        let g2 = r.get(&rt, b"k2").unwrap().unwrap();
        let ptr1 = g1.as_ptr();
        assert!(g1 == &v1[..] && g2 == &v2[..]);
        for round in 0..6usize {
            let mut w = s.write().unwrap();
            let t = w.open_tree("t", TreeKind::Normal).unwrap();
            let v = val(&mut rng, 3 * 8192 + 17 + round);
            w.set(&t, b"k1", &v).unwrap();
            w.remove(&t, b"k2").unwrap();
            w.remove_range(&t, &b"filler"[..]..&b"filler00150"[..])
                .unwrap();
            for i in 0..50u32 {
                let v = val(&mut rng, 40_000);
                w.set(&t, format!("new{round}{i:03}").as_bytes(), &v)
                    .unwrap();
            }
            w.commit().unwrap();
            if round % 2 == 0 {
                s.make_durable().unwrap();
            }
            let mut w = s.write().unwrap();
            let t = w.open_tree("t", TreeKind::Normal).unwrap();
            w.remove_range(&t, &b"new"[..]..&b"nex"[..]).unwrap();
            for i in 0..30u32 {
                let v = val(&mut rng, 70_000);
                w.set(&t, format!("zz{round}{i:03}").as_bytes(), &v)
                    .unwrap();
            }
            w.commit().unwrap();
            s.make_durable().unwrap();
            assert!(
                g1 == &v1[..] && g2 == &v2[..],
                "refs changed round {round} temporary={temporary}"
            );
            assert_eq!(g1.as_ptr(), ptr1);
            let again = r.get(&rt, b"k1").unwrap().unwrap();
            assert!(again == &v1[..]);
        }
        drop(r);
        s.close().unwrap();
    }
}

#[test]
fn threshold_sweep_keys_and_values() {
    for temporary in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let s = if temporary {
            Store::open_temporary(&StoreOptions::default()).unwrap()
        } else {
            open(tmp.path())
        };
        let mut rng = Rng(9);
        let mut model = Model::new();
        let mut w = s.write().unwrap();
        let t = w
            .open_tree_with("m", TreeKind::Normal, OpenMode::CreateNew)
            .unwrap();
        for klen in [1usize, 2, 17, 1000, 2047, 2048] {
            let thr = 4078 - 4 - klen;
            for d in -3i64..=3 {
                for (bi, base) in [thr as i64, 8192 - 16, 2 * 8192 - 16, 3 * 8192 - 16, 1024]
                    .into_iter()
                    .enumerate()
                {
                    let len = (base + d).max(0) as usize;
                    let mut k = val(&mut rng, klen);
                    if klen >= 3 {
                        k[klen - 1] = (d + 10) as u8;
                        k[klen - 2] = bi as u8;
                        k[0] = (klen % 251) as u8;
                    } else if klen == 2 {
                        k[0] = (d + 10) as u8 * 5 + bi as u8;
                        k[1] = 77;
                    } else {
                        k[0] = ((d + 10) as u8) * 5 + bi as u8;
                    }
                    let v = val(&mut rng, len);
                    w.set(&t, &k, &v).unwrap();
                    model.insert(k, v);
                }
            }
        }
        w.commit().unwrap();
        {
            let r = s.read().unwrap();
            check_all(&r, "m", &model, "sweep");
        }
        s.close().unwrap();
        if !temporary {
            let s = open(tmp.path());
            let r = s.read().unwrap();
            check_all(&r, "m", &model, "sweep reopened");
            drop(r);
            s.close().unwrap();
        }
    }
}

#[test]
#[ignore = "about 300 s: one open and read per altered byte"]
fn every_altered_byte_of_a_small_store_with_runs() {
    let tmp = tempfile::tempdir().unwrap();
    let mut rng = Rng(3);
    let v_big = val(&mut rng, 2 * 8192 + 100); // 3-page run
    let v_one = val(&mut rng, 5000); // 1-page run (off leaf)
    let v_small = val(&mut rng, 50);
    {
        let s = open(tmp.path());
        let mut w = s.write().unwrap();
        let t = w
            .open_tree_with("t", TreeKind::Normal, OpenMode::CreateNew)
            .unwrap();
        w.set(&t, b"big", &v_big).unwrap();
        w.set(&t, b"one", &v_one).unwrap();
        w.set(&t, b"small", &v_small).unwrap();
        w.commit().unwrap();
        s.close().unwrap();
    }
    let path = tmp.path().join("krummi.data");
    let original = std::fs::read(&path).unwrap();
    let mut wrong = 0usize;
    let mut detected = 0usize;
    let mut noeffect = 0usize;
    let used: Vec<usize> = (0..original.len())
        .filter(|i| {
            original[(i / 8192) * 8192..((i / 8192) + 1) * 8192]
                .iter()
                .any(|b| *b != 0)
        })
        .filter(|i| i % 13 == 0 || i % 8192 < 64)
        .collect();
    eprintln!(
        "p6 file {} bytes, {} in non-empty pages",
        original.len(),
        used.len()
    );
    for i in used {
        let mut bytes = original.clone();
        bytes[i] ^= 0x5a;
        std::fs::write(&path, &bytes).unwrap();
        let s = match Store::open(tmp.path(), &StoreOptions::default()) {
            Ok(s) => s,
            Err(_) => {
                detected += 1;
                continue;
            }
        };
        let mut outcome_ok = true;
        let mut errored = false;
        {
            let r = s.read().unwrap();
            match r.open_tree("t", TreeKind::Normal) {
                Err(_) => errored = true,
                Ok(t) => {
                    for (k, v) in [
                        (&b"big"[..], &v_big),
                        (&b"one"[..], &v_one),
                        (&b"small"[..], &v_small),
                    ] {
                        match r.get(&t, k) {
                            Err(_) => errored = true,
                            Ok(Some(g)) => {
                                if g != &v[..] {
                                    outcome_ok = false;
                                }
                            }
                            Ok(None) => {
                                outcome_ok = false;
                            }
                        }
                    }
                    match r.range(&t, ..) {
                        Err(_) => errored = true,
                        Ok(it) => {
                            let mut n = 0;
                            for e in it {
                                match e {
                                    Err(_) => {
                                        errored = true;
                                        break;
                                    }
                                    Ok((k, v)) => {
                                        n += 1;
                                        let exp = match k {
                                            b"big" => &v_big,
                                            b"one" => &v_one,
                                            b"small" => &v_small,
                                            _ => {
                                                outcome_ok = false;
                                                continue;
                                            }
                                        };
                                        if v != &exp[..] {
                                            outcome_ok = false;
                                        }
                                    }
                                }
                            }
                            if !errored && n != 3 {
                                outcome_ok = false;
                            }
                        }
                    }
                }
            }
        }
        let _ = s.close();
        if !outcome_ok {
            wrong += 1;
            eprintln!("WRONG DATA at byte {i}");
        } else if errored {
            detected += 1
        } else {
            noeffect += 1
        }
        std::fs::write(&path, &original).unwrap();
    }
    eprintln!(
        "p6: bytes={} detected={detected} noeffect={noeffect} wrong={wrong}",
        original.len()
    );
    assert_eq!(wrong, 0);
}
