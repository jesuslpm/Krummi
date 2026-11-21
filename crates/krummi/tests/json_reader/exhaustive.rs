//! AC-JS11 (5): every text up to a fixed length over the characters of numbers against an independent implementation
//! of RFC 8259's number grammar, and every `\uXXXX` escape alone and in pairs.

use krummi::table::json::normalize;
use krummi::{JsonCause, JsonFault};

use super::generator::normalized_string;

/// RFC 8259's `number = [ minus ] int [ frac ] [ exp ]`, written by splitting the text at its parts.
fn is_number(text: &str) -> bool {
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    let (mantissa, exponent) = match unsigned.find(['e', 'E']) {
        Some(at) => (&unsigned[..at], Some(&unsigned[at + 1..])),
        None => (unsigned, None),
    };
    let (int, frac) = match mantissa.split_once('.') {
        Some((int, frac)) => (int, Some(frac)),
        None => (mantissa, None),
    };
    let digits = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
    let int_ok = int == "0" || (digits(int) && !int.starts_with('0'));
    let frac_ok = frac.is_none_or(digits);
    let exp_ok = exponent.is_none_or(|e| digits(e.strip_prefix(['+', '-']).unwrap_or(e)));
    int_ok && frac_ok && exp_ok
}

/// Whether some number starts with `text`: every unfinished number is finished by one more digit.
fn starts_a_number(text: &str) -> bool {
    is_number(text) || is_number(&format!("{text}0"))
}

/// The syntax fault of a text over the number alphabet that is not a number: at the first character that no number
/// can continue, or at the end when the text ends before completing one.
fn number_fault_position(text: &str) -> u64 {
    (0..=text.len())
        .take_while(|&k| starts_a_number(&text[..k]))
        .last()
        .unwrap() as u64
}

#[test]
fn ac_js11_5_every_short_text_over_the_number_alphabet_follows_the_grammar() {
    const ALPHABET: &[u8] = b"0123456789.eE+-";
    const MAX_LEN: usize = 5;
    let mut text = Vec::new();
    let mut counts = [0usize; 2];
    // Every text of each length, as a counter in base 15.
    for len in 0..=MAX_LEN {
        let mut digits = vec![0usize; len];
        loop {
            text.clear();
            text.extend(digits.iter().map(|&d| ALPHABET[d]));
            let s = std::str::from_utf8(&text).unwrap();
            let in_array = format!("[{s}]");
            if is_number(s) {
                counts[0] += 1;
                assert_eq!(normalize(&text).as_deref(), Ok(s), "{s:?}");
                assert_eq!(
                    normalize(in_array.as_bytes()).as_deref(),
                    Ok(in_array.as_str()),
                    "{in_array:?}"
                );
            } else {
                counts[1] += 1;
                let at = number_fault_position(s);
                let syntax = |position| JsonFault {
                    cause: JsonCause::Syntax,
                    position,
                    pointer: "".into(),
                };
                assert_eq!(normalize(&text), Err(syntax(at)), "{s:?}");
                if !s.is_empty() {
                    // In an array, the `]` that follows an unfinished number is the fault.
                    assert_eq!(
                        normalize(in_array.as_bytes()),
                        Err(syntax(at + 1)),
                        "{in_array:?}"
                    );
                }
            }
            // The next text of this length.
            let mut i = 0;
            while i < len && digits[i] == ALPHABET.len() - 1 {
                digits[i] = 0;
                i += 1;
            }
            if i == len {
                break;
            }
            digits[i] += 1;
        }
    }
    assert!(counts[0] > 1000 && counts[1] > 100_000, "{counts:?}");
}

#[test]
fn the_independent_number_grammar_agrees_with_rfc_8259_examples() {
    for yes in [
        "0", "-0", "1", "10", "1.5", "0.5e1", "1E+2", "1e-2", "-9.99E99",
    ] {
        assert!(is_number(yes), "{yes}");
    }
    for no in [
        "", "-", "01", "1.", ".5", "+1", "1e", "1e+", "--1", "1.2.3", "1e5e", "0x1", "1-",
    ] {
        assert!(!is_number(no), "{no}");
    }
}

fn lone_at(position: u64) -> JsonFault {
    JsonFault {
        cause: JsonCause::LoneSurrogate,
        position,
        pointer: "".into(),
    }
}

/// The normalized text of a string of one character.
fn string_of(c: char) -> String {
    let mut out = String::new();
    normalized_string(&c.to_string(), &mut out);
    out
}

#[test]
fn ac_js11_5_every_unicode_escape_alone() {
    for unit in 0..=0xffffu32 {
        for doc in [format!("\"\\u{unit:04x}\""), format!("\"\\u{unit:04X}\"")] {
            let expected = match char::from_u32(unit) {
                Some(c) => Ok(string_of(c)),
                None => Err(lone_at(1)),
            };
            assert_eq!(normalize(doc.as_bytes()), expected, "{doc}");
        }
    }
}

#[test]
fn ac_js11_5_every_surrogate_pair_and_every_reversed_pair() {
    let mut doc = String::new();
    for high in 0xd800..=0xdbffu32 {
        for low in 0xdc00..=0xdfffu32 {
            doc.clear();
            doc.push_str(&format!("\"\\u{high:04x}\\u{low:04X}\""));
            let c = char::decode_utf16([high as u16, low as u16])
                .next()
                .unwrap()
                .unwrap();
            assert_eq!(normalize(doc.as_bytes()), Ok(format!("\"{c}\"")), "{doc}");
            doc.clear();
            doc.push_str(&format!("\"\\u{low:04x}\\u{high:04x}\""));
            assert_eq!(normalize(doc.as_bytes()), Err(lone_at(1)), "{doc}");
        }
    }
}

#[test]
fn ac_js11_5_a_surrogate_next_to_anything_but_its_pair_is_lone() {
    let followers = [
        "", "A", "\\n", "\\u0041", "\\u00e9", "\\uffff", "\\ud800", "\\udbff", "\\u", "\\ud",
        "\\udc0", "\\udc0g",
    ];
    for high in (0xd800..=0xdbffu32).step_by(3).chain([0xdbff]) {
        for follower in followers {
            let doc = format!("\"\\u{high:04x}{follower}\"");
            assert_eq!(normalize(doc.as_bytes()), Err(lone_at(1)), "{doc}");
        }
    }
    for low in (0xdc00..=0xdfffu32).step_by(3).chain([0xdfff]) {
        for before in ["", "a", "\\u0041", "😀"] {
            let doc = format!("\"{before}\\u{low:04x}\\udc00\"");
            assert_eq!(
                normalize(doc.as_bytes()),
                Err(lone_at(1 + before.len() as u64)),
                "{doc}"
            );
        }
    }
}
