//! AC-JS11 (3): the reader against `serde_json` with its default features (a development dependency only). Over every
//! document of the corpus and every generated one, both accept or both reject, except the listed differences; and
//! the value `serde_json` reads from the normalized text is the one it reads from the given text.

use krummi::JsonCause;
use krummi::table::json::normalize;
use serde_json::Value;

use super::corpus;
use super::generator::{Rng, random_node};

/// Why the reader and `serde_json` may disagree on accepting a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Difference {
    /// Krummi refuses repeated property names (JS2); `serde_json` keeps the last.
    Repeated,
    /// Krummi accepts numbers of any size (JS2); `serde_json` refuses those beyond `f64`.
    Number,
    /// Krummi accepts 128 nested levels (JS2); `serde_json`'s recursion limit stops at 127.
    Depth,
}

/// The corpus files on which the two disagree, with the reason.
const CORPUS_DIFFERENCES: &[(&str, Difference)] = &[
    ("i_number_huge_exp.json", Difference::Number),
    ("i_number_neg_int_huge_exp.json", Difference::Number),
    ("i_number_pos_double_huge_exp.json", Difference::Number),
    ("i_number_real_neg_overflow.json", Difference::Number),
    ("i_number_real_pos_overflow.json", Difference::Number),
    ("y_object_duplicated_key.json", Difference::Repeated),
    (
        "y_object_duplicated_key_and_value.json",
        Difference::Repeated,
    ),
];

/// Compares the two readers on one document: `Ok(None)` when they agree, `Ok(Some(..))` for an allowed difference,
/// `Err` otherwise.
fn compare(doc: &[u8]) -> Result<Option<Difference>, String> {
    let ours = normalize(doc);
    let theirs = serde_json::from_slice::<Value>(doc);
    match (ours, theirs) {
        (Ok(text), Ok(value)) => {
            let again: Value = serde_json::from_str(&text)
                .map_err(|e| format!("normalized text {text:?}: {e}"))?;
            if again == value {
                Ok(None)
            } else {
                Err(format!("values differ: {value:?} read as {again:?}"))
            }
        }
        (Err(_), Err(_)) => Ok(None),
        (Err(fault), Ok(_)) if matches!(fault.cause, JsonCause::RepeatedProperty(_)) => {
            Ok(Some(Difference::Repeated))
        }
        (Ok(_), Err(e)) if e.to_string().starts_with("number out of range") => {
            Ok(Some(Difference::Number))
        }
        (Ok(_), Err(e)) if e.to_string().starts_with("recursion limit exceeded") => {
            Ok(Some(Difference::Depth))
        }
        (ours, theirs) => Err(format!("krummi {ours:?}, serde_json {theirs:?}")),
    }
}

#[test]
fn ac_js11_3_the_corpus_agrees_with_serde_json_but_for_the_listed_files() {
    let mut differences = Vec::new();
    for (name, bytes) in corpus::files() {
        let expected = CORPUS_DIFFERENCES
            .iter()
            .find(|(file, _)| *file == name)
            .map(|(_, d)| *d);
        match compare(&bytes) {
            Ok(found) if found == expected => {}
            Ok(found) => {
                differences.push(format!("{name}: expected {expected:?}, found {found:?}"))
            }
            Err(e) => differences.push(format!("{name}: {e}")),
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

#[test]
fn ac_js11_3_generated_documents_agree_with_serde_json() {
    let mut rng = Rng(0x4b_4a53_0003);
    for i in 0..3000 {
        let node = random_node(&mut rng, 1 + (i % 6));
        let mut changed = node.rewritten(&mut rng).into_bytes();
        let at = rng.below(changed.len() as u64) as usize;
        changed[at] = rng.pick(b"{}[]\":,\\/ \tetfnu0-.+Ee1\x00\x1f\x7f\xff\xc3");
        let docs = [
            (node.normalized().into_bytes(), false),
            (node.rewritten(&mut rng).into_bytes(), false),
            (changed, true),
        ];
        for (doc, changed) in docs {
            match compare(&doc) {
                Ok(None | Some(Difference::Number)) => {}
                // A changed byte can make two names equal.
                Ok(Some(Difference::Repeated)) if changed => {}
                other => panic!("{:?}: {other:?}", String::from_utf8_lossy(&doc)),
            }
        }
    }
    // Depth: both accept 127 levels and refuse 129; only Krummi accepts 128.
    for (levels, expected) in [(127, None), (128, Some(Difference::Depth)), (129, None)] {
        for open in ["[", "{\"a\":"] {
            let close = if open == "[" { "]" } else { "}" };
            let doc = format!("{}0{}", open.repeat(levels), close.repeat(levels));
            assert_eq!(
                compare(doc.as_bytes()),
                Ok(expected),
                "{levels} levels of {open}"
            );
        }
    }
}
