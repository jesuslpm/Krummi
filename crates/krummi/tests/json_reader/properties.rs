//! AC-JS11 (6): property tests with a fixed seed. Random documents of every kind, written with random legal whitespace
//! and escapes, give the normalized text of their model; normalizing is idempotent and never lengthens a text; and a
//! random byte changed in a valid document gives an error or another valid document, never a panic.

use krummi::JsonCause;
use krummi::table::json::normalize;

use super::generator::{Rng, random_node};

#[test]
fn ac_js11_6_rewritten_documents_give_the_normalized_text_of_their_model() {
    let mut rng = Rng(0x4b_4a53_0006);
    for i in 0..4000 {
        let node = random_node(&mut rng, 1 + (i % 8));
        let expected = node.normalized();
        assert_eq!(
            normalize(expected.as_bytes()).as_deref(),
            Ok(expected.as_str()),
            "idempotent"
        );
        for _ in 0..3 {
            let input = node.rewritten(&mut rng);
            let text = normalize(input.as_bytes()).unwrap_or_else(|f| panic!("{input:?}: {f:?}"));
            assert_eq!(text, expected, "{input:?}");
            assert!(text.len() <= input.len(), "{input:?}");
        }
    }
}

#[test]
fn ac_js11_6_a_changed_byte_gives_an_error_or_a_valid_document() {
    let mut rng = Rng(0x4b_4a53_0066);
    let mut outcomes = [0usize; 2];
    for i in 0..4000 {
        let node = random_node(&mut rng, 1 + (i % 6));
        let mut bytes = node.rewritten(&mut rng).into_bytes();
        for _ in 0..1 + rng.below(3) {
            let at = rng.below(bytes.len() as u64) as usize;
            bytes[at] = if rng.chance(2) {
                rng.pick(b"{}[]\":,\\/ \tetfnu0-.+Ee1\x00\x1f\x7f")
            } else {
                rng.next() as u8
            };
        }
        match normalize(&bytes) {
            Ok(text) => {
                outcomes[0] += 1;
                assert_eq!(
                    normalize(text.as_bytes()).as_deref(),
                    Ok(text.as_str()),
                    "{bytes:?}"
                );
                assert!(text.len() <= bytes.len());
            }
            Err(fault) => {
                outcomes[1] += 1;
                assert!(fault.position <= bytes.len() as u64, "{bytes:?}: {fault:?}");
                // A fault found at its first offending byte stays there when the text is cut right after that byte
                // (a character cut in two is then invalid UTF-8 at the same position).
                if matches!(
                    fault.cause,
                    JsonCause::Syntax | JsonCause::InvalidUtf8 | JsonCause::TooDeep
                ) {
                    let cut = &bytes[..(fault.position as usize + 1).min(bytes.len())];
                    let again =
                        normalize(cut).expect_err("a text cut after its fault is not valid");
                    assert_eq!(
                        again.position, fault.position,
                        "{bytes:?}: {fault:?} then {again:?}"
                    );
                    assert_eq!(
                        again.pointer, fault.pointer,
                        "{bytes:?}: {fault:?} then {again:?}"
                    );
                }
            }
        }
    }
    assert!(outcomes[0] > 100 && outcomes[1] > 1000, "{outcomes:?}");
}
