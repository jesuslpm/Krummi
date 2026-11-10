//! E9 and E15 of the tables with schema PRD: the text forms of `ulid` and `uuid` (AC8) and their generators (AC12,
//! and the generator part of AC-N4 of the tables PRD).

use std::cmp::Ordering;
use std::collections::HashSet;
use std::io;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

use krummi::ids::testing::{Generator, Kind};
use krummi::ids::{self, Ulid, Uuid};
use krummi::{Error, ErrorKind, Result, ValueRule};
use krummi_pager::testing::{self as faults, Action, Op};

const KINDS: [Kind; 3] = [Kind::Ulid, Kind::UuidV7, Kind::SequentialUuid];
/// The last millisecond that fits in 48 bits (E15).
const MAX_MS: u64 = (1 << 48) - 1;

/// A value of `kind` from its process-wide generator.
fn generate(kind: Kind) -> Result<[u8; 16]> {
    match kind {
        Kind::Ulid => ids::ulid().map(|u| u.0),
        Kind::UuidV7 => ids::uuid_v7().map(|u| u.0),
        Kind::SequentialUuid => ids::sequential_uuid().map(|u| u.0),
    }
}

/// The order of the column `kind`'s values go to: their bytes, except the sequential `uuid`, which goes to a column
/// with the order of SQL Server. `SqlGuid` compares the bytes of `Guid.ToByteArray` in the groups 10-15, 8-9, 6-7,
/// 4-5 and 0-3, and `ToByteArray` of the RFC 9562 bytes r0..r15 is r3 r2 r1 r0 r5 r4 r7 r6 r8..r15 (document 3 of
/// the table design, *UUID and ULID*).
fn column_order(kind: Kind, a: &[u8; 16], b: &[u8; 16]) -> Ordering {
    match kind {
        Kind::SequentialUuid => sql_server_key(a).cmp(&sql_server_key(b)),
        _ => a.cmp(b),
    }
}

fn sql_server_key(r: &[u8; 16]) -> [u8; 16] {
    let d = [
        r[3], r[2], r[1], r[0], r[5], r[4], r[7], r[6], r[8], r[9], r[10], r[11], r[12], r[13],
        r[14], r[15],
    ];
    let mut key = [0; 16];
    let mut i = 0;
    for group in [10..16, 8..10, 6..8, 4..6, 0..4] {
        for j in group {
            key[i] = d[j];
            i += 1;
        }
    }
    key
}

/// The millisecond of a value of `kind` (the first 6 bytes, or bytes 10-15 of the sequential `uuid`).
fn millisecond(kind: Kind, value: &[u8; 16]) -> u64 {
    let bytes = match kind {
        Kind::SequentialUuid => &value[10..16],
        _ => &value[..6],
    };
    bytes.iter().fold(0, |n, &b| (n << 8) | u64::from(b))
}

/// Sorting `values` with their column's order leaves them in generation order, with no two equal.
fn assert_in_generation_order(kind: Kind, values: &[[u8; 16]]) {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| column_order(kind, a, b));
    assert!(
        sorted == values,
        "{kind:?}: sorting changes the generation order"
    );
    for pair in values.windows(2) {
        assert_eq!(
            column_order(kind, &pair[0], &pair[1]),
            Ordering::Less,
            "{kind:?}: {:02x?} then {:02x?}",
            pair[0],
            pair[1]
        );
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn assert_invalid_value(err: &Error, expected: ValueRule) {
    assert_eq!(err.kind(), ErrorKind::InvalidValue, "{err}");
    let Error::InvalidValue {
        folder,
        table,
        column,
        sequence,
        rule,
    } = err
    else {
        unreachable!()
    };
    assert_eq!(*rule, expected);
    assert_eq!(
        (folder, table, column, sequence),
        (&None, &None, &None, &None)
    );
}

/// A deterministic stand-in for the bits the operating system returns (not the engine's: tests only).
struct TestBits(u64);

impl TestBits {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

// --- E9: text forms (AC8) ---

/// Values of each generator, converted to text and read back, give the same value, in the one form of E9.
#[test]
fn generated_values_round_trip_through_their_text_form() {
    for _ in 0..1000 {
        let ulid = ids::ulid().unwrap();
        let text = ulid.to_string();
        assert_eq!(text.len(), 26);
        assert!(text.as_bytes()[0] <= b'7', "{text}");
        assert!(
            text.bytes()
                .all(|c| b"0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(&c)),
            "{text}"
        );
        assert_eq!(text.parse::<Ulid>().unwrap(), ulid);

        for uuid in [ids::uuid_v7().unwrap(), ids::sequential_uuid().unwrap()] {
            let text = uuid.to_string();
            assert_eq!(text.len(), 36);
            assert_eq!(text, text.to_lowercase());
            let groups: Vec<usize> = text.split('-').map(str::len).collect();
            assert_eq!(groups, [8, 4, 4, 4, 12], "{text}");
            assert_eq!(text.replace('-', ""), hex(&uuid.0), "RFC 9562 byte order");
            assert_eq!(text.parse::<Uuid>().unwrap(), uuid);
        }
    }
}

/// The text forms of known values: Crockford Base32 of the 128 bits (the first character holds the top 3), and the
/// RFC 9562 text of the bytes in order.
#[test]
fn known_values_have_the_text_of_e9() {
    let cases: [([u8; 16], &str); 3] = [
        ([0; 16], "00000000000000000000000000"),
        ([0xff; 16], "7ZZZZZZZZZZZZZZZZZZZZZZZZZ"),
        (
            [
                0x01, 0x8b, 0xcf, 0xe5, 0x68, 0x00, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef,
                0x01, 0x23,
            ],
            "01HF7YAT0004HMASW9NF6YY093",
        ),
    ];
    for (bytes, text) in cases {
        assert_eq!(Ulid(bytes).to_string(), text);
        assert_eq!(text.parse::<Ulid>().unwrap(), Ulid(bytes));
    }
    let bytes = [
        0x01, 0x8b, 0xcf, 0xe5, 0x68, 0x00, 0x78, 0xd3, 0xbf, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xfe,
    ];
    let text = "018bcfe5-6800-78d3-bfff-fffffffffffe";
    assert_eq!(Uuid(bytes).to_string(), text);
    assert_eq!(text.parse::<Uuid>().unwrap(), Uuid(bytes));
    assert_eq!(
        "ffffffff-ffff-ffff-ffff-ffffffffffff"
            .parse::<Uuid>()
            .unwrap(),
        Uuid([0xff; 16])
    );
}

/// Every text that is not the one form of E9 fails with an invalid value, rule malformed text, with no table,
/// column or sequence.
#[test]
fn unsupported_text_forms_fail_with_invalid_value() {
    let ulids = [
        "",
        "01hf7yat0004hmasw9nf6yy093",           // lowercase
        "01HF7YAT0004HMASW9NF6YY09k",           // one lowercase character
        "01HF7YAT0004HMASW9NF6YY09",            // 25 characters
        "01HF7YAT0004HMASW9NF6YY093K",          // 27 characters
        "81HF7YAT0004HMASW9NF6YY093",           // first character above 7
        "Z1HF7YAT0004HMASW9NF6YY093",           // first character above 7
        "01HF7YAT0004HMASW9NF6YY09I",           // I is not in the alphabet
        "01HF7YAT0004HMASW9NF6YY09L",           // L is not in the alphabet
        "01HF7YAT0004HMASW9NF6YY09O",           // O is not in the alphabet
        "01HF7YAT0004HMASW9NF6YY09U",           // U is not in the alphabet
        "01HF7YAT0004HMASW9NF6YY09-",           // punctuation
        " 01HF7YAT0004HMASW9NF6YY093",          // leading space
        "01HF7YAT0004HMASW9NF6YY093 ",          // trailing space
        "01HF7YAT00048MAHW9NF6YY0\u{e9}",       // non-ASCII, 26 bytes
        "018bcfe5-6800-78d3-bfff-fffffffffffe", // a uuid
    ];
    for text in ulids {
        let err = text.parse::<Ulid>().unwrap_err();
        assert_invalid_value(&err, ValueRule::MalformedText);
    }
    let uuids = [
        "",
        "018BCFE5-6800-78D3-BFFF-FFFFFFFFFFFE",   // uppercase
        "018bcfe5-6800-78d3-bfff-fffffffffffE",   // one uppercase digit
        "{018bcfe5-6800-78d3-bfff-fffffffffffe}", // braces
        "(018bcfe5-6800-78d3-bfff-fffffffffffe)", // parentheses
        "018bcfe5680078d3bffffffffffffffe",       // no hyphens
        "018bcfe56800-78d3-bfff-fffffffffffe",    // a hyphen missing
        "018bcfe-56800-78d3-bfff-fffffffffffe",   // a hyphen moved
        "018bcfe5-6800-78d3-bfff-fffffffffffe0",  // 37 characters
        "018bcfe5-6800-78d3-bfff-ffffffffffff-",  // a hyphen at the end
        "018bcfe5-6800-78d3-bfff-fffffffffff",    // 35 characters
        "018bcfe5-6800-78d3-bfff-fffffffffffg",   // not hexadecimal
        "018bcfe5_6800_78d3_bfff_fffffffffffe",   // other separators
        "urn:uuid:018bcfe5-6800-78d3-bfff-fffffffffffe",
        " 18bcfe5-6800-78d3-bfff-fffffffffffe",     // space
        "+18bcfe5-6800-78d3-bfff-fffffffffffe",     // sign
        "018bcfe5-6800-78d3-bfff-ffffffffff\u{e9}", // non-ASCII, 36 bytes
        "01HF7YAT0004HMASW9NF6YY093",               // a ulid
    ];
    for text in uuids {
        let err = text.parse::<Uuid>().unwrap_err();
        assert_invalid_value(&err, ValueRule::MalformedText);
    }
}

// --- E15: generators (AC12, AC-N4) ---

/// 100,000 values of each generator from one thread, with a test clock that stops, goes back and advances, and test
/// random bits near exhaustion inside a millisecond: sorted with their column's order, they stay in generation order;
/// as many share a millisecond as the increment allows and the rest go to successive milliseconds (AC-N4).
#[test]
fn values_from_one_thread_sort_in_generation_order() {
    const BASE: u64 = 1_700_000_000_000;
    for kind in KINDS {
        let random_bits = if kind == Kind::UuidV7 { 74 } else { 80 };
        let mask = (1u128 << random_bits) - 1;
        let mut generator = Generator::new(kind);
        let mut bits = TestBits(7);
        let mut draws = 0;
        let mut overflows = 0;
        let mut went_back = 0;
        let mut values = Vec::with_capacity(100_000);
        for i in 0..100_000u64 {
            // The clock stops for 20 calls at each millisecond; in every 7th block of 500 calls it goes back 20 ms,
            // and in every 11th it jumps ahead 30 ms (and then back).
            let block = i / 500;
            let mut clock = BASE + i / 20;
            if block % 7 == 3 {
                clock -= 20;
            }
            if block % 11 == 5 {
                clock += 30;
            }
            let previous = values.last().map(|v| millisecond(kind, v));
            if previous.is_some_and(|p| clock < p) {
                went_back += 1;
            }
            let value = generator
                .next(clock, &mut |buf: &mut [u8]| {
                    draws += 1;
                    // One draw in three is a few increments from exhaustion.
                    let n = if draws % 3 == 0 {
                        mask - u128::from(bits.next() % 4)
                    } else {
                        (u128::from(bits.next()) << 64) | u128::from(bits.next())
                    };
                    buf.copy_from_slice(&n.to_be_bytes()[6..]);
                    Ok(())
                })
                .unwrap();
            let ms = millisecond(kind, &value);
            if previous.is_some_and(|p| clock <= p && ms == p + 1) {
                overflows += 1;
            }
            values.push(value);
        }
        assert!(draws > 1_000, "{kind:?}: {draws} draws");
        assert!(overflows > 100, "{kind:?}: {overflows} overflows");
        assert!(went_back > 1_000, "{kind:?}: {went_back} calls behind");
        assert_in_generation_order(kind, &values);
    }
}

/// Several threads at once: all values distinct, each thread's increasing, and a value generated after the others
/// finished is greater than all of them.
#[test]
fn values_from_several_threads_are_distinct_and_increasing() {
    const THREADS: usize = 8;
    const PER_THREAD: usize = 5_000;
    for kind in KINDS {
        let barrier = Arc::new(Barrier::new(THREADS));
        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    (0..PER_THREAD)
                        .map(|_| generate(kind).unwrap())
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        let mut all = HashSet::new();
        for handle in handles {
            let values = handle.join().unwrap();
            assert_in_generation_order(kind, &values);
            all.extend(values);
        }
        assert_eq!(all.len(), THREADS * PER_THREAD, "{kind:?}: repeated values");
        let after = generate(kind).unwrap();
        assert!(
            all.iter()
                .all(|v| column_order(kind, v, &after) == Ordering::Less),
            "{kind:?}"
        );
    }
}

/// A uuid version 7 keeps its version and variant bits (RFC 9562 §5.7).
#[test]
fn uuid_v7_values_carry_version_7_and_the_rfc_variant() {
    for _ in 0..1000 {
        let uuid = ids::uuid_v7().unwrap();
        assert_eq!(uuid.0[6] >> 4, 7);
        assert_eq!(uuid.0[8] >> 6, 0b10);
    }
}

/// A failure of the operating system's generator makes the function fail with an input/output error; the generator
/// works again afterwards.
#[test]
fn a_failure_of_the_operating_system_generator_is_an_io_error() {
    for kind in KINDS {
        faults::reset();
        // The generator draws only at a new millisecond: the fault waits for this thread's next draw.
        faults::inject(
            Op::Random,
            faults::count(Op::Random),
            Action::Fail(io::ErrorKind::Other),
        );
        let mut failed = None;
        for _ in 0..10_000 {
            match generate(kind) {
                Ok(_) => thread::sleep(Duration::from_millis(1)),
                Err(e) => {
                    failed = Some(e);
                    break;
                }
            }
        }
        faults::reset();
        let err = failed.expect("the generator drew random bits");
        assert_eq!(err.kind(), ErrorKind::Io, "{kind:?}: {err}");
        generate(kind).unwrap();
    }
}

/// A test random source that fails: the call fails with that error and changes nothing, so the next call gives the
/// value the failed one would have given.
#[test]
fn a_failed_draw_changes_nothing() {
    for kind in KINDS {
        let mut generator = Generator::new(kind);
        let mut fail = |_: &mut [u8]| -> Result<()> {
            Err(Error::Io {
                folder: Default::default(),
                source: io::Error::other("test"),
            })
        };
        let mut ones = |buf: &mut [u8]| -> Result<()> {
            buf.fill(0xff);
            Ok(())
        };
        let first = generator.next(10, &mut ones).unwrap();
        // Exhausted at 10: the next call needs a draw for millisecond 11.
        let err = generator.next(10, &mut fail).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Io);
        let err = generator.next(12, &mut fail).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Io);
        let mut reference = Generator::new(kind);
        reference.next(10, &mut ones).unwrap();
        assert_eq!(
            generator.next(10, &mut ones).unwrap(),
            reference.next(10, &mut ones).unwrap()
        );
        assert_eq!(millisecond(kind, &first), 10);
    }
}

/// With the clock at the last 48-bit millisecond and the increment exhausted, the function fails with an invalid
/// value, rule millisecond out of range, and draws nothing; a clock past 48 bits fails the same way.
#[test]
fn the_last_millisecond_with_the_increment_exhausted_fails() {
    for kind in KINDS {
        let mut generator = Generator::new(kind);
        let mut draws = 0;
        let mut ones = |buf: &mut [u8]| -> Result<()> {
            draws += 1;
            buf.fill(0xff);
            Ok(())
        };
        let last = generator.next(MAX_MS, &mut ones).unwrap();
        assert_eq!(millisecond(kind, &last), MAX_MS);
        for clock in [MAX_MS, MAX_MS - 1, 0] {
            let err = generator.next(clock, &mut ones).unwrap_err();
            assert_invalid_value(&err, ValueRule::MillisecondOutOfRange);
        }

        let mut fresh = Generator::new(kind);
        for clock in [MAX_MS + 1, u64::MAX] {
            let err = fresh.next(clock, &mut ones).unwrap_err();
            assert_invalid_value(&err, ValueRule::MillisecondOutOfRange);
        }
        assert_eq!(draws, 1);
    }
}

/// The vectors stored in the repository (tests/data/generators/vectors.txt, written once by tools/guid-vectors from
/// the layout of the user's .NET generator and ordered with `SqlGuid`): each generator gives exactly their bytes with
/// the test clock and bits, draws exactly when they say, fails where they say, and the sequential `uuid` values of
/// each scenario sort with the SQL Server order in the vectors' order.
#[test]
fn generators_give_the_stored_vectors() {
    let text = include_str!("data/generators/vectors.txt");
    let mut scenario: Option<(Kind, String, Generator, Vec<[u8; 16]>)> = None;
    let mut checked = [0usize; 3];
    let finish = |scenario: Option<(Kind, String, Generator, Vec<[u8; 16]>)>| {
        if let Some((kind, name, _, values)) = scenario {
            assert!(!values.is_empty(), "{name}");
            let mut sorted = values.clone();
            sorted.sort_by(|a, b| column_order(kind, a, b));
            assert!(
                sorted == values,
                "{kind:?} {name}: not in the vectors' order"
            );
        }
    };
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        match fields.as_slice() {
            [] => {}
            [first, ..] if first.starts_with('#') => {}
            ["scenario", kind, name] => {
                finish(scenario.take());
                let kind = match *kind {
                    "ulid" => Kind::Ulid,
                    "uuid-v7" => Kind::UuidV7,
                    "sequential-uuid" => Kind::SequentialUuid,
                    other => panic!("unknown generator {other}"),
                };
                scenario = Some((kind, name.to_string(), Generator::new(kind), Vec::new()));
            }
            ["step", clock, random, expected] => {
                let (kind, name, generator, values) = scenario.as_mut().expect("a scenario");
                let clock: u64 = clock.parse().unwrap();
                let random = (*random != "-").then(|| unhex(random));
                let mut drew = false;
                let result = generator.next(clock, &mut |buf: &mut [u8]| {
                    drew = true;
                    let bits = random
                        .as_ref()
                        .unwrap_or_else(|| panic!("{name}: a draw at {clock}"));
                    buf.copy_from_slice(bits);
                    Ok(())
                });
                assert_eq!(drew, random.is_some(), "{name}: draw at {clock}");
                if *expected == "invalid-value" {
                    assert_invalid_value(&result.unwrap_err(), ValueRule::MillisecondOutOfRange);
                } else {
                    let value = result.unwrap();
                    assert_eq!(hex(&value), *expected, "{kind:?} {name} at {clock}");
                    if *kind == Kind::UuidV7 {
                        assert_eq!((value[6] >> 4, value[8] >> 6), (7, 0b10));
                    }
                    values.push(value);
                }
                checked[*kind as usize] += 1;
            }
            _ => panic!("malformed line: {line}"),
        }
    }
    finish(scenario);
    assert!(checked.iter().all(|&n| n >= 30), "{checked:?}");
}
