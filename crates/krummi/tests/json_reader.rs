//! The JSON reader (JS2, JS4, JS8 and JS14 of `docs/prd/krummi/tables/tables-json.prd.md`), through
//! `krummi::table::json::normalize`: the examples of AC-JS2, the faults of AC-JS3, and the layers of AC-JS11 that
//! test the reader alone: the JSONTestSuite corpus (1), the comparison with `serde_json` (3), the exhaustive small
//! inputs (5), the property tests (6), the limits (7) and, through all of them, the hand-made mutants (8).

#[path = "json_reader/corpus.rs"]
mod corpus;
#[path = "json_reader/differential.rs"]
mod differential;
#[path = "json_reader/exhaustive.rs"]
mod exhaustive;
#[path = "json_reader/generator.rs"]
mod generator;
#[path = "json_reader/limits.rs"]
mod limits;
#[path = "json_reader/properties.rs"]
mod properties;

use krummi::table::json::normalize;
use krummi::{JsonCause, JsonFault};

use generator::{Node, Rng, random_node};

/// The normalized text of a valid document; panics with the fault otherwise.
fn accepted(input: &[u8]) -> String {
    normalize(input)
        .unwrap_or_else(|fault| panic!("{:?} rejected: {fault:?}", String::from_utf8_lossy(input)))
}

/// The fault of an invalid document; panics with the normalized text otherwise.
fn rejected(input: &[u8]) -> JsonFault {
    match normalize(input) {
        Ok(text) => panic!("{:?} accepted as {text:?}", String::from_utf8_lossy(input)),
        Err(fault) => fault,
    }
}

fn fault(cause: JsonCause, position: u64, pointer: &str) -> JsonFault {
    JsonFault {
        cause,
        position,
        pointer: pointer.into(),
    }
}

fn repeated(name: &str) -> JsonCause {
    JsonCause::RepeatedProperty(name.into())
}

// ---------------------------------------------------------------------------------------------------------------
// AC-JS2: the normalized text

#[test]
fn ac_js2_examples_are_read_as_their_normalized_text() {
    let cases: [(&str, &str); 3] = [
        (
            r#" { "b" : 1.50 , "a" : [ true , "é\/" ] } "#,
            r#"{"b":1.50,"a":[true,"é/"]}"#,
        ),
        (r#""A\u0000\u001F\t\"""#, r#""A\u0000\u001f\t\"""#),
        ("\"😀\"", "\"😀\""),
    ];
    for (input, expected) in cases {
        assert_eq!(accepted(input.as_bytes()), expected, "{input:?}");
    }
}

#[test]
fn ac_js2_an_indented_document_of_about_4_kb_gives_its_compact_text() {
    // Members of every kind around a large array, generated with a fixed seed until the compact text reaches 4 KB.
    let mut rng = Rng(0x4b_4a53_0002);
    let mut members = vec![("id".to_string(), Node::Number("42".into()))];
    let mut lines = Vec::new();
    let compact = loop {
        lines.push(random_node(&mut rng, 3));
        let mut doc = members.clone();
        doc.push(("lines".into(), Node::Array(lines.clone())));
        doc.push(("tag".into(), Node::Str("é\"\\/\n".into())));
        let doc = Node::Object(doc);
        if doc.normalized().len() >= 4096 {
            break doc;
        }
        if lines.len() % 7 == 0 {
            members.push((format!("m{}", lines.len()), random_node(&mut rng, 2)));
        }
    };
    let indented = compact.indented();
    let expected = compact.normalized();
    assert!(
        indented.len() > expected.len() + 500,
        "the indented text has whitespace to remove"
    );
    let text = accepted(indented.as_bytes());
    assert_eq!(text, expected);
    assert_eq!(accepted(expected.as_bytes()), expected);
}

// ---------------------------------------------------------------------------------------------------------------
// AC-JS3: the faults, with their cause, position and pointer (JS8)

#[test]
fn ac_js3_every_text_of_the_table_fails_with_its_cause_position_and_pointer() {
    use JsonCause::*;
    let deep_open = "[".repeat(129);
    let deep = format!("{deep_open}{}", "]".repeat(129));
    let deep_pointer = "/0".repeat(127);
    let cases: Vec<(&[u8], JsonFault)> = vec![
        (b"", fault(Syntax, 0, "")),
        (b"   ", fault(Syntax, 3, "")),
        (b"1 2", fault(Syntax, 2, "")),
        (b"\xEF\xBB\xBF1", fault(Syntax, 0, "")),
        (b"NaN", fault(Syntax, 0, "")),
        (b"+1", fault(Syntax, 0, "")),
        (b".5", fault(Syntax, 0, "")),
        (b"'a'", fault(Syntax, 0, "")),
        (b"01", fault(Syntax, 1, "")),
        (b"1.", fault(Syntax, 2, "")),
        (b"[1,]", fault(Syntax, 3, "")),
        (br#"{"a":1,}"#, fault(Syntax, 7, "")),
        (br#"{"a":1}/*c*/"#, fault(Syntax, 7, "")),
        (b"[\"a\t\"]", fault(Syntax, 3, "")),
        (br#""\x""#, fault(Syntax, 2, "")),
        (br#""\u12""#, fault(Syntax, 5, "")),
        (br#""\ud800""#, fault(LoneSurrogate, 1, "")),
        (br#""\udc00""#, fault(LoneSurrogate, 1, "")),
        (br#""\ud800A""#, fault(LoneSurrogate, 1, "")),
        (b"\xFF", fault(InvalidUtf8, 0, "")),
        (b"[\"\xE2\x28\"]", fault(InvalidUtf8, 2, "")),
        (b"\"\xE2\x82", fault(InvalidUtf8, 1, "")),
        (deep.as_bytes(), fault(TooDeep, 128, &deep_pointer)),
        (br#"{"a":1,"a":2}"#, fault(repeated("a"), 7, "")),
        (
            br#"{"a":1,"b":{"a":2,"a":3}}"#,
            fault(repeated("a"), 18, "/b"),
        ),
        (
            br#"{"x":{"a/~":[{"k":1,"k":2}]}}"#,
            fault(repeated("k"), 20, "/x/a~1~0/0"),
        ),
        (br#"{"\ud800":1,"\ud800":2}"#, fault(LoneSurrogate, 2, "")),
    ];
    for (input, expected) in cases {
        assert_eq!(
            rejected(input),
            expected,
            "{:?}",
            String::from_utf8_lossy(input)
        );
    }
}

#[test]
fn several_faults_report_the_smallest_position_then_the_order_of_the_causes() {
    use JsonCause::*;
    let cases: Vec<(&[u8], JsonFault)> = vec![
        // A repeated name in an open object comes before a later fault of any cause.
        (br#"{"a":1,"a":2"#, fault(repeated("a"), 7, "")),
        (br#"{"a":1,"a" 2}"#, fault(repeated("a"), 7, "")),
        (br#"{"a":1,"a":"\ud800"}"#, fault(repeated("a"), 7, "")),
        (b"{\"a\":1,\"a\":[\xFF]}", fault(repeated("a"), 7, "")),
        (br#"{"a":1,"a":[[[1,]]]}"#, fault(repeated("a"), 7, "")),
        // The outer object's repeat comes before the inner one's, found first.
        (
            br#"{"a":1,"a":2,"b":{"c":1,"c":2}}"#,
            fault(repeated("a"), 7, ""),
        ),
        (
            br#"{"a":1,"b":{"c":1,"c":2},"a":3}"#,
            fault(repeated("c"), 18, "/b"),
        ),
        // A name with a fault does not repeat: the fault is reported instead.
        (br#"{"a":1,"a"#, fault(Syntax, 9, "")),
        (br#"{"\ud800":1,"a":2,"a":3}"#, fault(LoneSurrogate, 2, "")),
        // Repeats are found after decoding, never by Unicode normalization.
        (br#"{"a":1,"\u0061":2}"#, fault(repeated("a"), 7, "")),
        (br#"{"\/":1,"/":2}"#, fault(repeated("/"), 8, "")),
        (
            r#"{"\ud83d\ude00":1,"😀":2}"#.as_bytes(),
            fault(repeated("😀"), 18, ""),
        ),
        // The first of several repeats, by the position of the later name.
        (
            br#"{"b":0,"a":1,"c":2,"a":3,"b":4}"#,
            fault(repeated("a"), 19, ""),
        ),
        // Invalid UTF-8 at the same position as the end of an unfinished text is reported as such.
        (b"[\"\\ud8\xFF", fault(InvalidUtf8, 6, "")),
        (b"1\xFF", fault(InvalidUtf8, 1, "")),
        (b"{\"a\":[\xFF]}", fault(InvalidUtf8, 6, "/a")),
        // A high surrogate followed by anything but a low one is lone, before what follows.
        (br#""\ud800\u12G4""#, fault(LoneSurrogate, 1, "")),
        (br#""\ud800\x""#, fault(LoneSurrogate, 1, "")),
        (br#""\ud800"#, fault(LoneSurrogate, 1, "")),
        (b"\"\\ud800\xFF\"", fault(LoneSurrogate, 1, "")),
        (br#""\udc00\ud800""#, fault(LoneSurrogate, 1, "")),
        (br#""a\u0041\udc00""#, fault(LoneSurrogate, 8, "")),
        // The pointer is the innermost container open at the fault.
        (br#"{"a":[1,{"b":[true,]}]}"#, fault(Syntax, 19, "/a/1/b")),
        (br#"[[1],[2]x]"#, fault(Syntax, 8, "")),
        (br#"{"~0/":{"":[x]}}"#, fault(Syntax, 12, "/~00~1/")),
        (b"[1}", fault(Syntax, 2, "")),
    ];
    for (input, expected) in cases {
        assert_eq!(
            rejected(input),
            expected,
            "{:?}",
            String::from_utf8_lossy(input)
        );
    }
}

#[test]
fn syntax_faults_are_at_the_first_character_that_no_text_can_continue() {
    let cases: [(&str, u64); 26] = [
        ("tru", 3),
        ("trux", 3),
        ("nul", 3),
        ("falsy", 4),
        ("-", 1),
        ("-a", 1),
        ("1e", 2),
        ("1e+", 3),
        ("1.e5", 2),
        ("-01", 2),
        ("0x10", 1),
        ("1.5.", 3),
        ("Infinity", 0),
        ("-Infinity", 1),
        ("[", 1),
        ("{", 1),
        ("{\"a\"", 4),
        ("\"abc", 4),
        ("{\"a\" 1}", 5),
        ("{1:1}", 1),
        ("[1 2]", 3),
        ("\"\\", 2),
        ("{\"a\":}", 5),
        ("\u{a0}1", 0),
        ("[\"\u{1f}\"]", 2),
        ("1\u{feff}", 1),
    ];
    for (input, position) in cases {
        assert_eq!(
            rejected(input.as_bytes()),
            fault(JsonCause::Syntax, position, ""),
            "{input:?}"
        );
    }
}

#[test]
fn values_at_the_root_and_whitespace_are_accepted() {
    let cases: [(&str, &str); 11] = [
        ("42", "42"),
        (" \"x\" ", "\"x\""),
        ("true", "true"),
        ("\t\n\rnull\r\n\t ", "null"),
        ("false", "false"),
        ("-0", "-0"),
        ("1.50e+3", "1.50e+3"),
        ("[ ]", "[]"),
        ("{ }", "{}"),
        ("\"\\u0000\"", "\"\\u0000\""),
        ("\"\\uFFFF\\uffFe\"", "\"\u{ffff}\u{fffe}\""),
    ];
    for (input, expected) in cases {
        assert_eq!(accepted(input.as_bytes()), expected, "{input:?}");
    }
    // A combining accent and the precomposed character are different names.
    assert_eq!(
        accepted("{\"é\":1,\"e\u{301}\":2}".as_bytes()),
        "{\"é\":1,\"e\u{301}\":2}"
    );
}

#[test]
fn the_first_repeat_is_found_among_many_repeats_of_each_name() {
    let mut rng = Rng(0x4b_4a53_0008);
    for round in 0..20 {
        // Up to 3,000 members over a few names, so that each name repeats many times in an unsorted order.
        let distinct = 2 + round * 3;
        let mut text = String::from("{");
        let mut seen = std::collections::HashSet::new();
        let mut first_repeat = None;
        for i in 0..3000 {
            if i > 0 {
                text.push(',');
            }
            let name = format!("n{}", rng.below(distinct));
            if !seen.insert(name.clone()) && first_repeat.is_none() {
                first_repeat = Some((name.clone(), text.len() as u64));
            }
            text.push_str(&format!("\"{name}\":{i}"));
        }
        text.push('}');
        let (name, position) = first_repeat.unwrap();
        assert_eq!(
            rejected(text.as_bytes()),
            fault(repeated(&name), position, ""),
            "round {round}"
        );
    }
}
