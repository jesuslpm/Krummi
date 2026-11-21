//! AC-JS11 (1): Nicolas Seriot's JSONTestSuite (`tests/data/json_test_suite`, see its README). Every `y_` file is
//! accepted and every `n_` file rejected, except the `y_` files that break a stricter rule of Krummi, listed by name
//! with their rule and error; every `i_` file has the outcome pinned here. A difference not listed fails the test.

use std::collections::BTreeMap;
use std::path::PathBuf;

use krummi::JsonCause;
use krummi::table::json::normalize;

/// What the reader does with a file: accepts it, or rejects it with this cause.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Accept,
    Reject(JsonCause),
}

/// The `y_` files Krummi rejects by a rule stricter than RFC 8259's grammar: JS2 refuses repeated property names.
const Y_EXCEPTIONS: &[(&str, &str)] = &[
    ("y_object_duplicated_key.json", "a"),
    ("y_object_duplicated_key_and_value.json", "a"),
];

/// The outcome of every implementation-defined `i_` file (JS2 and JS8): numbers of any size are accepted; every other
/// file breaks a rule of JS2, with the cause of its first fault.
fn i_outcomes() -> Vec<(&'static str, Outcome)> {
    use JsonCause::*;
    use Outcome::*;
    vec![
        ("i_number_double_huge_neg_exp.json", Accept),
        ("i_number_huge_exp.json", Accept),
        ("i_number_neg_int_huge_exp.json", Accept),
        ("i_number_pos_double_huge_exp.json", Accept),
        ("i_number_real_neg_overflow.json", Accept),
        ("i_number_real_pos_overflow.json", Accept),
        ("i_number_real_underflow.json", Accept),
        ("i_number_too_big_neg_int.json", Accept),
        ("i_number_too_big_pos_int.json", Accept),
        ("i_number_very_big_negative_int.json", Accept),
        (
            "i_object_key_lone_2nd_surrogate.json",
            Reject(LoneSurrogate),
        ),
        (
            "i_string_1st_surrogate_but_2nd_missing.json",
            Reject(LoneSurrogate),
        ),
        (
            "i_string_1st_valid_surrogate_2nd_invalid.json",
            Reject(LoneSurrogate),
        ),
        ("i_string_UTF-16LE_with_BOM.json", Reject(InvalidUtf8)),
        ("i_string_UTF-8_invalid_sequence.json", Reject(InvalidUtf8)),
        ("i_string_UTF8_surrogate_U+D800.json", Reject(InvalidUtf8)),
        (
            "i_string_incomplete_surrogate_and_escape_valid.json",
            Reject(LoneSurrogate),
        ),
        (
            "i_string_incomplete_surrogate_pair.json",
            Reject(LoneSurrogate),
        ),
        (
            "i_string_incomplete_surrogates_escape_valid.json",
            Reject(LoneSurrogate),
        ),
        (
            "i_string_invalid_lonely_surrogate.json",
            Reject(LoneSurrogate),
        ),
        ("i_string_invalid_surrogate.json", Reject(LoneSurrogate)),
        ("i_string_invalid_utf-8.json", Reject(InvalidUtf8)),
        (
            "i_string_inverted_surrogates_U+1D11E.json",
            Reject(LoneSurrogate),
        ),
        ("i_string_iso_latin_1.json", Reject(InvalidUtf8)),
        ("i_string_lone_second_surrogate.json", Reject(LoneSurrogate)),
        (
            "i_string_lone_utf8_continuation_byte.json",
            Reject(InvalidUtf8),
        ),
        ("i_string_not_in_unicode_range.json", Reject(InvalidUtf8)),
        (
            "i_string_overlong_sequence_2_bytes.json",
            Reject(InvalidUtf8),
        ),
        (
            "i_string_overlong_sequence_6_bytes.json",
            Reject(InvalidUtf8),
        ),
        (
            "i_string_overlong_sequence_6_bytes_null.json",
            Reject(InvalidUtf8),
        ),
        ("i_string_truncated-utf-8.json", Reject(InvalidUtf8)),
        // A NUL byte is valid UTF-8 but no JSON token: the text fails at its first character.
        ("i_string_utf16BE_no_BOM.json", Reject(Syntax)),
        ("i_string_utf16LE_no_BOM.json", Reject(Syntax)),
        ("i_structure_500_nested_arrays.json", Reject(TooDeep)),
        ("i_structure_UTF-8_BOM_empty_object.json", Reject(Syntax)),
    ]
}

/// Every file of the corpus, by name.
pub fn files() -> BTreeMap<String, Vec<u8>> {
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/json_test_suite/test_parsing");
    let mut files = BTreeMap::new();
    for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        files.insert(name, std::fs::read(&path).unwrap());
    }
    files
}

/// The outcome every file must have.
fn expected(name: &str, i: &BTreeMap<&str, Outcome>) -> Outcome {
    if let Some((_, repeated)) = Y_EXCEPTIONS.iter().find(|(file, _)| *file == name) {
        return Outcome::Reject(JsonCause::RepeatedProperty((*repeated).into()));
    }
    match &name[..2] {
        "y_" => Outcome::Accept,
        "i_" => i
            .get(name)
            .unwrap_or_else(|| panic!("{name}: no pinned outcome"))
            .clone(),
        "n_" => Outcome::Reject(JsonCause::Syntax), // any cause; compared by acceptance below
        _ => panic!("{name}: not a file of the corpus"),
    }
}

#[test]
fn ac_js11_1_the_json_test_suite_has_its_pinned_outcomes() {
    let files = files();
    assert_eq!(files.len(), 318, "the vendored corpus");
    let i: BTreeMap<&str, Outcome> = i_outcomes().into_iter().collect();
    for name in i
        .keys()
        .copied()
        .chain(Y_EXCEPTIONS.iter().map(|(name, _)| *name))
    {
        assert!(
            files.contains_key(name),
            "{name}: listed but not in the corpus"
        );
    }
    let mut differences = Vec::new();
    for (name, bytes) in &files {
        let expected = expected(name, &i);
        let actual = normalize(bytes);
        let same = match (&expected, &actual) {
            (Outcome::Accept, Ok(_)) => true,
            (Outcome::Reject(_), Err(_)) if name.starts_with("n_") => true,
            (Outcome::Reject(cause), Err(fault)) => fault.cause == *cause,
            _ => false,
        };
        if !same {
            differences.push(format!("{name}: expected {expected:?}, got {actual:?}"));
        }
        match actual {
            Ok(text) => {
                // Accepted texts are normalized: idempotent and never longer than the input (JS4).
                assert_eq!(
                    normalize(text.as_bytes()).as_deref(),
                    Ok(text.as_str()),
                    "{name}"
                );
                assert!(text.len() <= bytes.len(), "{name}");
            }
            Err(fault) => assert!(fault.position <= bytes.len() as u64, "{name}: {fault:?}"),
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}
