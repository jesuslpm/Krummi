//! AC-JS1 (JS1, JS2, JS4): the corpus through a table, read back, closed and reopened, and in a temporary store; and
//! AC-JS4 (JS5): the column null against the document `null`, and a `null` property against a missing one.

use krummi::table::json::{JsonKind, normalize};
use krummi::table::value::Value;

use super::generator::{Node, Rng, random_node};
use super::*;

/// The numbers of AC-JS1, as their texts.
const NUMBERS: [&str; 10] = [
    "-0",
    "0",
    "1E400",
    "-1e-400",
    "123456789012345678901234567890",
    "18446744073709551616",
    "0.1",
    "0.10000000000000000000001",
    "1.0",
    "1",
];

/// `levels` nested containers, `true` an object (with the member `k`) and `false` an array, the innermost empty.
fn nested(kinds: &[bool]) -> Node {
    let mut node = if kinds[kinds.len() - 1] {
        Node::Object(Vec::new())
    } else {
        Node::Array(Vec::new())
    };
    for &object in kinds[..kinds.len() - 1].iter().rev() {
        node = if object {
            Node::Object(vec![("k".to_owned(), node)])
        } else {
            Node::Array(vec![node])
        };
    }
    node
}

/// The corpus of AC-JS1: each document's model and the text given for it.
fn corpus() -> Vec<(Node, String)> {
    let mut rng = Rng(0x4b_4a53_0001);
    let mut models = vec![
        Node::Null,
        Node::Bool(true),
        Node::Bool(false),
        Node::Str(String::new()),
        // Characters of one to four bytes of UTF-8.
        Node::Str("a é € 😀".to_owned()),
        Node::Array(Vec::new()),
        Node::Object(Vec::new()),
        Node::Array(
            NUMBERS
                .iter()
                .map(|n| Node::Number((*n).to_owned()))
                .collect(),
        ),
        // An object of thousands of properties.
        Node::Object(
            (0..3_000)
                .map(|i| (format!("p{i}"), Node::Number(i.to_string())))
                .collect(),
        ),
    ];
    models.extend(NUMBERS.iter().map(|n| Node::Number((*n).to_owned())));
    // 128 levels: only objects, only arrays, alternating from each, and ten random mixes with a fixed seed.
    let mut ways = vec![
        vec![true; 128],
        vec![false; 128],
        (0..128).map(|i| i % 2 == 0).collect::<Vec<_>>(),
        (0..128).map(|i| i % 2 == 1).collect(),
    ];
    for _ in 0..10 {
        ways.push((0..128).map(|_| rng.chance(2)).collect());
    }
    models.extend(ways.iter().map(|kinds| nested(kinds)));
    models.extend((0..40).map(|_| random_node(&mut rng, 4)));
    let mut corpus: Vec<(Node, String)> = models
        .into_iter()
        .map(|node| {
            let given = node.rewritten(&mut rng);
            (node, given)
        })
        .collect();
    // Every escape, and characters of one to four bytes given escaped.
    corpus.push((
        Node::Str("\"\\/\u{8}\u{c}\n\r\t\u{0}\u{1f}Aé€😀".to_owned()),
        r#""\"\\\/\b\f\n\r\t\u0000\u001FAé€😀""#.to_owned(),
    ));
    corpus
}

/// Asserts that the row of key `i` of `t` in `store` reads the normalized text of the model of `corpus[i]`, which
/// represents the model with its member order and number texts, and normalizes to itself.
fn check_corpus(store: &Store, corpus: &[(Node, String)]) {
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    for (i, (node, given)) in corpus.iter().enumerate() {
        let row = read
            .get_row(&table, &[Value::I64(i as i64)])
            .unwrap()
            .unwrap();
        let json = row.json(1).unwrap();
        let text = json.text().unwrap();
        assert_eq!(text, node.normalized(), "{given}");
        assert_model(node, json.document().unwrap());
        assert_eq!(normalize(text.as_bytes()).as_deref(), Ok(text), "{given}");
        assert_eq!(text_of(&row.get("doc").unwrap()), text);
    }
}

#[test]
fn ac_js1_the_corpus_round_trips_through_a_table() {
    let corpus = corpus();
    stores(|store, _| {
        insert_corpus(store, &corpus);
        check_corpus(store, &corpus);
    });
}

fn insert_corpus(store: &Store, corpus: &[(Node, String)]) {
    let mut write = store.write().unwrap();
    let table = create(&mut write, "t", &docs());
    for (i, (_, given)) in corpus.iter().enumerate() {
        write
            .insert_row(&table, &[("id", Value::I64(i as i64)), ("doc", doc(given))])
            .unwrap();
    }
    write.commit().unwrap();
}

/// The corpus written, the store closed and reopened, reads the same.
#[test]
fn ac_js1_the_corpus_reads_the_same_after_closing_and_reopening() {
    let corpus = corpus();
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    insert_corpus(&store, &corpus);
    store.close().unwrap();
    let store = open(tmp.path());
    check_corpus(&store, &corpus);
    store.close().unwrap();
}

/// AC-JS4 (JS5): a null column reads null, the document `null` a value of class `null`; in `{"a":null}`, `a` is a
/// `null` value and `b` is not there.
#[test]
fn ac_js4_the_column_null_is_not_the_document_null() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &docs());
        let rows = [
            [("id", Value::I64(1)), ("doc", Value::Null)],
            [("id", Value::I64(2)), ("doc", doc(" null "))],
            [("id", Value::I64(3)), ("doc", doc(r#"{"a":null}"#))],
        ];
        for row in &rows {
            write.insert_row(&table, row).unwrap();
        }
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let row = |id: i64| read.get_row(&table, &[Value::I64(id)]).unwrap().unwrap();
        let null = row(1);
        assert_eq!(null.get("doc"), Some(Value::Null));
        assert!(null.json(1).is_none());
        let document = row(2);
        let json = document.json(1).unwrap();
        assert_eq!(json.text(), Some("null"));
        assert_eq!(json.document().unwrap().kind(), JsonKind::Null);
        let object = row(3);
        let json = object.json(1).unwrap();
        let value = json.document().unwrap();
        assert_eq!(value.get("a").map(|a| a.kind()), Some(JsonKind::Null));
        assert_eq!(value.pointer("/a").unwrap().map(|a| a.text()), Some("null"));
        assert!(value.get("b").is_none());
        assert!(value.pointer("/b").unwrap().is_none());
    });
}
