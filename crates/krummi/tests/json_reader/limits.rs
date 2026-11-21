//! AC-JS11 (7): 128 nested levels are accepted and 129 refused for every way of nesting, deep texts never overflow
//! the stack, and the check of repeated names stays O(n log n) on an object of a million properties.

use std::time::{Duration, Instant};

use krummi::table::json::normalize;
use krummi::{JsonCause, JsonFault};

use super::generator::Rng;

/// A document of `kinds.len()` nested containers (`true` an object, `false` an array), each the only value of its
/// parent, the innermost empty; with the position of the innermost container's opening and the pointer of its parent.
fn nested(kinds: &[bool]) -> (String, u64, String) {
    let mut text = String::new();
    let mut steps = Vec::new();
    let mut innermost = 0;
    for (level, &object) in kinds.iter().enumerate() {
        innermost = text.len() as u64;
        let last = level + 1 == kinds.len();
        match (object, last) {
            (true, false) => {
                text.push_str("{\"k/\":");
                steps.push("/k~1");
            }
            (false, false) => {
                text.push('[');
                steps.push("/0");
            }
            (true, true) => text.push_str("{}"),
            (false, true) => text.push_str("[]"),
        }
    }
    for &object in kinds[..kinds.len() - 1].iter().rev() {
        text.push(if object { '}' } else { ']' });
    }
    // The parent of the innermost container is reached by the steps of the levels above the parent.
    (
        text,
        innermost,
        steps[..kinds.len().saturating_sub(2)].concat(),
    )
}

/// Every way of nesting of AC-JS1: only objects, only arrays, alternating from each, and ten seeded random mixes.
fn ways(levels: usize) -> Vec<Vec<bool>> {
    let mut ways = vec![
        vec![true; levels],
        vec![false; levels],
        (0..levels).map(|i| i % 2 == 0).collect(),
        (0..levels).map(|i| i % 2 == 1).collect(),
    ];
    let mut rng = Rng(0x4b_4a53_0007);
    for _ in 0..10 {
        ways.push((0..levels).map(|_| rng.chance(2)).collect());
    }
    ways
}

#[test]
fn ac_js11_7_128_levels_are_accepted_and_129_refused() {
    for kinds in ways(128) {
        let (text, _, _) = nested(&kinds);
        assert_eq!(
            normalize(text.as_bytes()).as_deref(),
            Ok(text.as_str()),
            "{text}"
        );
    }
    for kinds in ways(129) {
        let (text, position, pointer) = nested(&kinds);
        let expected = JsonFault {
            cause: JsonCause::TooDeep,
            position,
            pointer: pointer.into(),
        };
        assert_eq!(normalize(text.as_bytes()), Err(expected), "{text}");
    }
}

#[test]
fn deep_texts_never_overflow_the_stack() {
    for open in ["[", "{\"a\":", "[{\"\":"] {
        let text = open.repeat(1_000_000);
        let fault = normalize(text.as_bytes()).unwrap_err();
        assert_eq!(fault.cause, JsonCause::TooDeep);
        assert!(fault.position < 1000);
    }
}

/// An object of `members` distinct properties, followed by a repeat of the first when `repeat`.
fn wide_object(members: usize, repeat: bool) -> Vec<u8> {
    use std::fmt::Write as _;
    let mut text = String::with_capacity(members * 12);
    text.push('{');
    // Distinct names in an order that is not sorted: an odd multiplier is a bijection of the 32-bit integers.
    for i in 0..members as u32 {
        write!(text, "\"{:x}\":0,", i.wrapping_mul(0x9e37_79b9)).unwrap();
    }
    if repeat {
        text.push_str("\"0\":0,");
    }
    text.pop();
    text.push('}');
    text.into_bytes()
}

/// The median time of five runs of each document, run in turns so that both see the same load.
fn medians_of_five(docs: [&[u8]; 2], run: impl Fn(&[u8])) -> [Duration; 2] {
    let mut times = [Vec::new(), Vec::new()];
    for _ in 0..5 {
        for (doc, times) in docs.iter().zip(&mut times) {
            let start = Instant::now();
            run(doc);
            times.push(start.elapsed());
        }
    }
    times.map(|mut times| {
        times.sort();
        times[2]
    })
}

#[test]
fn ac_js11_7_a_million_properties_take_at_most_20_times_a_hundred_thousand() {
    for repeat in [false, true] {
        let small = wide_object(100_000, repeat);
        let large = wide_object(1_000_000, repeat);
        let run = |doc: &[u8]| match normalize(doc) {
            Ok(_) => assert!(!repeat),
            Err(fault) => {
                assert!(repeat, "{fault:?}");
                assert_eq!(fault.cause, JsonCause::RepeatedProperty("0".into()));
                assert_eq!(fault.position, doc.len() as u64 - 6);
            }
        };
        let [small_time, large_time] = medians_of_five([&small, &large], run);
        let ratio = large_time.as_secs_f64() / small_time.as_secs_f64();
        assert!(
            ratio <= 20.0,
            "repeat {repeat}: {large_time:?} for 10^6 members, {small_time:?} for 10^5 ({ratio:.1} times)"
        );
    }
}
