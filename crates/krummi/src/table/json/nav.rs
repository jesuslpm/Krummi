//! Navigation of a normalized JSON document (JS6) and the typed reads of its values (JS7).
//!
//! The text is trusted (JS5): it is scanned with no recursion, every value skipped by counting brackets outside
//! strings, so that a damaged text gives wrong values but never overflows the stack, panics on a slice or loops.

use std::borrow::Cow;

use krummi_pager::{Error, Result, ValueRule};

use super::{number, time};
use crate::table::value::{ColumnType, Decimal, check_type};

/// The class of a JSON value (JS6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonKind {
    Object,
    Array,
    String,
    Number,
    Bool,
    Null,
}

/// A JSON value inside a normalized document (JS6): the document itself or any value it contains. Its text is the
/// fragment of the document's normalized text that forms it, borrowed without a copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JsonValue<'a> {
    text: &'a str,
}

/// The properties of an object in stored order: each name, decoded as a string read as text (JS7), and its value.
#[derive(Debug, Clone)]
pub struct Members<'a> {
    /// The text after the opening brace or the last comma read.
    rest: &'a str,
}

/// The elements of an array in order.
#[derive(Debug, Clone)]
pub struct Elements<'a> {
    /// The text after the opening bracket or the last comma read.
    rest: &'a str,
}

impl<'a> JsonValue<'a> {
    /// The value whose normalized text is `text`. The text is trusted: navigating a text that is not normalized may
    /// return wrong values or panic, never overflow the stack or loop (JS5, L2).
    pub(crate) fn new(text: &'a str) -> Self {
        JsonValue { text }
    }

    /// Its class.
    pub fn kind(&self) -> JsonKind {
        match self.text.as_bytes().first() {
            Some(b'{') => JsonKind::Object,
            Some(b'[') => JsonKind::Array,
            Some(b'"') => JsonKind::String,
            Some(b't' | b'f') => JsonKind::Bool,
            Some(b'n') => JsonKind::Null,
            _ => JsonKind::Number,
        }
    }

    /// Its normalized text, a valid normalized JSON text itself (a string's includes its quotes).
    pub fn text(&self) -> &'a str {
        self.text
    }

    /// The number of properties of an object or elements of an array; `None` for another class.
    pub fn len(&self) -> Option<usize> {
        match self.kind() {
            JsonKind::Object => self.members().map(Iterator::count),
            JsonKind::Array => self.elements().map(Iterator::count),
            _ => None,
        }
    }

    /// Whether an object has no properties or an array no elements; `None` for another class.
    pub fn is_empty(&self) -> Option<bool> {
        self.len().map(|len| len == 0)
    }

    /// The properties of an object; `None` for another class.
    pub fn members(&self) -> Option<Members<'a>> {
        (self.kind() == JsonKind::Object).then(|| Members {
            rest: &self.text[1..],
        })
    }

    /// The value of the property with this name, compared with the decoded names (JS2); `None` when it is absent or
    /// this is not an object.
    pub fn get(&self, name: &str) -> Option<JsonValue<'a>> {
        self.members()?
            .find(|(member, _)| member == name)
            .map(|(_, value)| value)
    }

    /// The elements of an array; `None` for another class.
    pub fn elements(&self) -> Option<Elements<'a>> {
        (self.kind() == JsonKind::Array).then(|| Elements {
            rest: &self.text[1..],
        })
    }

    /// The element at `index`, from 0; `None` when it is absent or this is not an array.
    pub fn element(&self, index: usize) -> Option<JsonValue<'a>> {
        self.elements()?.nth(index)
    }

    /// The value of an RFC 6901 pointer relative to this value (JS6): `Ok(None)` when it is absent, and invalid value
    /// with the rule malformed JSON pointer for a malformed pointer, checked before navigating.
    pub fn pointer(&self, pointer: &str) -> Result<Option<JsonValue<'a>>> {
        let Some(steps) = pointer.strip_prefix('/') else {
            return if pointer.is_empty() {
                Ok(Some(*self))
            } else {
                Err(read_error(ValueRule::MalformedJsonPointer))
            };
        };
        // Every `~` is followed by `0` or `1`.
        if pointer
            .split('~')
            .skip(1)
            .any(|after| !after.starts_with(['0', '1']))
        {
            return Err(read_error(ValueRule::MalformedJsonPointer));
        }
        let mut value = *self;
        for step in steps.split('/') {
            let name = if step.contains('~') {
                Cow::Owned(step.replace("~1", "/").replace("~0", "~"))
            } else {
                Cow::Borrowed(step)
            };
            let next = match value.kind() {
                JsonKind::Object => value.get(&name),
                JsonKind::Array => array_index(&name).and_then(|index| value.element(index)),
                _ => None,
            };
            match next {
                Some(next) => value = next,
                None => return Ok(None),
            }
        }
        Ok(Some(value))
    }

    /// A string as text, without its quotes: borrowed when it has no escapes, decoded into a copy when it has (JS7).
    pub fn as_text(&self) -> Option<Cow<'a, str>> {
        let inner = self.text.strip_prefix('"')?.strip_suffix('"')?;
        Some(if inner.contains('\\') {
            Cow::Owned(unescape(inner))
        } else {
            Cow::Borrowed(inner)
        })
    }

    /// `true` or `false`.
    pub fn as_bool(&self) -> Option<bool> {
        match self.text {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    }

    /// A number whose exact value is an integer that `i64` holds, in any notation (JS7).
    pub fn as_i64(&self) -> Option<i64> {
        self.number().and_then(number::as_i64)
    }

    /// A number whose exact value is an integer that `u64` holds, in any notation (JS7).
    pub fn as_u64(&self) -> Option<u64> {
        self.number().and_then(number::as_u64)
    }

    /// A number rounded to the nearest `f64`, `-0` as `+0`; `None` outside the finite range (JS7).
    pub fn as_f64(&self) -> Option<f64> {
        self.number().and_then(number::as_f64)
    }

    /// A number rounded to `scale` as E8 rounds, `None` when it then has more than `precision` digits (JS7); invalid
    /// value with the rule invalid read scale when `precision` or `scale` is outside 1 ≤ p ≤ 38, 0 ≤ s ≤ p, checked
    /// before the value.
    pub fn as_decimal(&self, precision: u8, scale: u8) -> Result<Option<Decimal>> {
        // The limits of N4, those of a `decimal` column.
        if check_type(&ColumnType::Decimal { precision, scale }).is_err() {
            return Err(read_error(ValueRule::InvalidReadScale));
        }
        Ok(self
            .number()
            .and_then(|text| number::as_decimal(text, precision, scale)))
    }

    /// A string `YYYY-MM-DD` in the domain of E8, as days since 1970-01-01 (JS7).
    pub fn as_date(&self) -> Option<i32> {
        time::date(&self.as_text()?)
    }

    /// An RFC 3339 instant with an offset, converted to UTC and rounded to the microsecond, in the domain of E8, as
    /// nanoseconds since the Unix epoch (JS7).
    pub fn as_timestamp(&self) -> Option<i128> {
        time::timestamp(&self.as_text()?)
    }

    /// The text of a number.
    fn number(&self) -> Option<&'a str> {
        (self.kind() == JsonKind::Number).then_some(self.text)
    }
}

impl<'a> Iterator for Members<'a> {
    type Item = (Cow<'a, str>, JsonValue<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        // Each call consumes text or ends the iteration, so it terminates on any text.
        let rest = std::mem::take(&mut self.rest);
        let bytes = rest.as_bytes();
        if bytes.first() != Some(&b'"') {
            return None;
        }
        let name_end = string_end(bytes, 0);
        let name = JsonValue::new(&rest[..name_end]).as_text()?;
        if bytes.get(name_end) != Some(&b':') {
            return None;
        }
        let end = value_end(bytes, name_end + 1);
        let value = JsonValue::new(&rest[name_end + 1..end]);
        if bytes.get(end) == Some(&b',') {
            self.rest = &rest[end + 1..];
        }
        Some((name, value))
    }
}

impl<'a> Iterator for Elements<'a> {
    type Item = JsonValue<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        // Each call consumes text or ends the iteration, so it terminates on any text.
        let rest = std::mem::take(&mut self.rest);
        let bytes = rest.as_bytes();
        if matches!(bytes.first(), None | Some(b']')) {
            return None;
        }
        let end = value_end(bytes, 0);
        if bytes.get(end) == Some(&b',') {
            self.rest = &rest[end + 1..];
        }
        Some(JsonValue::new(&rest[..end]))
    }
}

/// The invalid value of a read (JS6, JS7): with no table, column or other data.
fn read_error(rule: ValueRule) -> Error {
    Error::InvalidValue {
        folder: None,
        table: None,
        column: None,
        sequence: None,
        rule,
        json: None,
    }
}

/// The array index of a pointer step: `0` or decimal digits without a leading zero (JS6); `None` for any other
/// step and for an index no array can reach.
fn array_index(step: &str) -> Option<usize> {
    let digits = !step.is_empty() && step.bytes().all(|b| b.is_ascii_digit());
    if digits && (step == "0" || !step.starts_with('0')) {
        step.parse().ok()
    } else {
        None
    }
}

/// The position after the value that starts at `start`, found without recursion: a string ends at its closing quote,
/// an object or an array at the bracket that closes it, counting the brackets outside strings, and any other value at
/// the first `,`, `:`, `}` or `]`. A text that ends first gives its length. Every position returned is a character
/// boundary: the text's end, or that of an ASCII byte.
fn value_end(bytes: &[u8], start: usize) -> usize {
    let mut depth = 0usize;
    let mut i = start;
    while let Some(&byte) = bytes.get(i) {
        match byte {
            b'"' => {
                i = string_end(bytes, i);
                if depth == 0 {
                    return i;
                }
                continue;
            }
            b'{' | b'[' => depth += 1,
            b'}' | b']' if depth > 1 => depth -= 1,
            b'}' | b']' => return if depth == 1 { i + 1 } else { i },
            b',' | b':' if depth == 0 => return i,
            _ => {}
        }
        i += 1;
    }
    bytes.len()
}

/// The position after the string whose opening quote is at `start`: after its closing quote, skipping each escaped
/// byte, or the text's length when it is not closed.
fn string_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start + 1;
    while let Some(&byte) = bytes.get(i) {
        match byte {
            b'"' => return i + 1,
            b'\\' => i += 2,
            _ => i += 1,
        }
    }
    bytes.len()
}

/// The decoded content of a string with escapes: those of the normalized text (JS4) and, for a text that is not
/// normalized, the rest of RFC 8259's, a lone surrogate as U+FFFD.
fn unescape(inner: &str) -> String {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let decoded = match chars.next() {
            None => break,
            Some('b') => '\u{8}',
            Some('f') => '\u{c}',
            Some('n') => '\n',
            Some('r') => '\r',
            Some('t') => '\t',
            Some('u') => {
                let code = chars
                    .by_ref()
                    .take(4)
                    .try_fold(0u32, |code, c| Some(code * 16 + c.to_digit(16)?));
                code.and_then(char::from_u32).unwrap_or('\u{fffd}')
            }
            Some(other) => other,
        };
        out.push(decoded);
    }
    out
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use krummi_pager::{Error, ValueRule};

    use super::{JsonKind, JsonValue};

    /// The document of AC-JS5.
    const DOC: &str = r#"{"a/b":{"~":[10,20]},"x":[1,2,3],"s":"t","o":{},"e":[]}"#;

    fn at<'a>(doc: &JsonValue<'a>, pointer: &str) -> Option<&'a str> {
        doc.pointer(pointer).unwrap().map(|v| v.text())
    }

    fn assert_malformed(result: krummi_pager::Result<Option<JsonValue<'_>>>) {
        let err = result.unwrap_err();
        assert!(
            matches!(
                err,
                Error::InvalidValue {
                    folder: None,
                    table: None,
                    column: None,
                    sequence: None,
                    rule: ValueRule::MalformedJsonPointer,
                    json: None,
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn ac_js5_properties_come_back_in_order_with_name_and_value() {
        let doc = JsonValue::new(DOC);
        let members: Vec<(String, &str)> = doc
            .members()
            .unwrap()
            .map(|(name, value)| (name.into_owned(), value.text()))
            .collect();
        assert_eq!(
            members,
            [
                ("a/b".to_owned(), r#"{"~":[10,20]}"#),
                ("x".to_owned(), "[1,2,3]"),
                ("s".to_owned(), r#""t""#),
                ("o".to_owned(), "{}"),
                ("e".to_owned(), "[]"),
            ]
        );
    }

    #[test]
    fn ac_js5_pointers() {
        let doc = JsonValue::new(DOC);
        assert_eq!(at(&doc, "/a~1b/~0/1"), Some("20"));
        assert_eq!(at(&doc, ""), Some(DOC));
        assert_eq!(at(&doc, "/x/0"), Some("1"));
        for absent in ["/x/3", "/x/-", "/x/01", "/x/+1", "/s/0", "/nada"] {
            assert_eq!(at(&doc, absent), None, "{absent}");
        }
    }

    #[test]
    fn ac_js5_texts_and_counts() {
        let doc = JsonValue::new(DOC);
        let x = doc.get("x").unwrap();
        let s = doc.get("s").unwrap();
        let o = doc.get("o").unwrap();
        let e = doc.get("e").unwrap();
        assert_eq!(x.text(), "[1,2,3]");
        assert_eq!(s.text(), r#""t""#);
        assert_eq!(x.len(), Some(3));
        assert_eq!(o.len(), Some(0));
        assert_eq!(o.is_empty(), Some(true));
        assert_eq!(o.members().unwrap().count(), 0);
        assert_eq!(e.len(), Some(0));
        assert_eq!(e.is_empty(), Some(true));
        assert_eq!(e.elements().unwrap().count(), 0);
        assert_eq!(e.element(0), None);
        assert_eq!(doc.len(), Some(5));
        assert_eq!(doc.is_empty(), Some(false));
        let elements: Vec<&str> = x.elements().unwrap().map(|v| v.text()).collect();
        assert_eq!(elements, ["1", "2", "3"]);
        assert_eq!(x.element(2).map(|v| v.text()), Some("3"));
        assert_eq!(x.element(3), None);
    }

    #[test]
    fn ac_js5_other_class_gives_none() {
        let doc = JsonValue::new(DOC);
        let x = doc.get("x").unwrap();
        let o = doc.get("o").unwrap();
        let s = doc.get("s").unwrap();
        assert!(x.members().is_none());
        assert_eq!(x.get("0"), None);
        for v in [o, s] {
            assert!(v.elements().is_none());
            assert_eq!(v.element(0), None);
        }
        assert!(s.members().is_none());
        assert_eq!(s.get("t"), None);
        assert_eq!(s.len(), None);
        assert_eq!(s.is_empty(), None);
        for scalar in ["1", "true", "null", r#""x""#] {
            let v = JsonValue::new(scalar);
            assert_eq!(v.len(), None);
            assert!(v.members().is_none());
            assert!(v.elements().is_none());
        }
    }

    #[test]
    fn ac_js5_malformed_pointers_fail() {
        let doc = JsonValue::new(DOC);
        for pointer in ["a", "/~2", "/a~", "/s/0/~2"] {
            assert_malformed(doc.pointer(pointer));
        }
        // Checked before navigating, also when an earlier step is already absent (JS6).
        assert_malformed(doc.pointer("/nada/~"));
        assert_malformed(doc.pointer("/x/9/~x"));
        // And on a scalar.
        assert_malformed(JsonValue::new("1").pointer("x"));
    }

    #[test]
    fn ac_js4_null_property_and_missing_one() {
        let doc = JsonValue::new(r#"{"a":null}"#);
        let a = doc.get("a").unwrap();
        assert_eq!(a.kind(), JsonKind::Null);
        assert_eq!(a.text(), "null");
        assert_eq!(doc.get("b"), None);
        assert_eq!(at(&doc, "/a"), Some("null"));
        assert_eq!(at(&doc, "/b"), None);
        assert_eq!(JsonValue::new("null").kind(), JsonKind::Null);
    }

    #[test]
    fn kind_of_every_class() {
        for (text, kind) in [
            ("{}", JsonKind::Object),
            (r#"{"a":1}"#, JsonKind::Object),
            ("[]", JsonKind::Array),
            (r#""""#, JsonKind::String),
            ("0", JsonKind::Number),
            ("-1.5e3", JsonKind::Number),
            ("true", JsonKind::Bool),
            ("false", JsonKind::Bool),
            ("null", JsonKind::Null),
        ] {
            assert_eq!(JsonValue::new(text).kind(), kind, "{text}");
        }
    }

    #[test]
    fn pointer_steps_follow_rfc_6901() {
        let array: Vec<String> = (0..12).map(|i| i.to_string()).collect();
        let array = format!("[{}]", array.join(","));
        let array = JsonValue::new(&array);
        assert_eq!(at(&array, "/10"), Some("10"));
        assert_eq!(at(&array, "/0"), Some("0"));
        for absent in [
            "/010",
            "/00",
            "/-",
            "/+1",
            "/-1",
            "/ 1",
            "/1 ",
            "/",
            "/12",
            "/1e1",
            "/99999999999999999999999",
        ] {
            assert_eq!(at(&array, absent), None, "{absent}");
        }
        // On an object every step is a name, digits too; the empty name and names with `/` and `~`.
        let object = JsonValue::new(r#"{"0":"zero","":"empty","a/b":1,"m~n":2,"~1":3,"01":4}"#);
        assert_eq!(at(&object, "/0"), Some(r#""zero""#));
        assert_eq!(at(&object, "/"), Some(r#""empty""#));
        assert_eq!(at(&object, "/a~1b"), Some("1"));
        assert_eq!(at(&object, "/m~0n"), Some("2"));
        assert_eq!(at(&object, "/~01"), Some("3"));
        assert_eq!(at(&object, "/01"), Some("4"));
        // A step on a scalar is absent.
        for scalar in ["1", "true", "null", r#""ab""#] {
            assert_eq!(at(&JsonValue::new(scalar), "/0"), None);
            assert_eq!(at(&JsonValue::new(scalar), ""), Some(scalar));
        }
        // Relative to the value it is asked of.
        let doc = JsonValue::new(DOC);
        let inner = doc.get("a/b").unwrap();
        assert_eq!(at(&inner, "/~0/0"), Some("10"));
    }

    #[test]
    fn names_are_decoded_and_copied_only_with_escapes() {
        let doc = JsonValue::new(
            "{\"plain\":1,\"q\\\"\\\\\\n\\u0000\\u001f\":2,\"\u{e9}\":3,\"e\u{301}\":4,\"😀\":5}",
        );
        let names: Vec<Cow<'_, str>> = doc.members().unwrap().map(|(name, _)| name).collect();
        assert!(matches!(names[0], Cow::Borrowed("plain")));
        assert_eq!(names[1], "q\"\\\n\u{0}\u{1f}");
        assert!(matches!(names[1], Cow::Owned(_)));
        assert!(matches!(names[2], Cow::Borrowed("\u{e9}")));
        assert_eq!(doc.get("q\"\\\n\u{0}\u{1f}").map(|v| v.text()), Some("2"));
        // No Unicode normalization: the precomposed and the combining forms are different names (JS2).
        assert_eq!(doc.get("\u{e9}").map(|v| v.text()), Some("3"));
        assert_eq!(doc.get("e\u{301}").map(|v| v.text()), Some("4"));
        assert_eq!(doc.get("😀").map(|v| v.text()), Some("5"));
        assert_eq!(doc.get("q"), None);
        // A borrowed name points into the document.
        let Cow::Borrowed(plain) = &names[0] else {
            unreachable!()
        };
        assert!(
            doc.text()
                .as_bytes()
                .as_ptr_range()
                .contains(&plain.as_ptr())
        );
    }

    #[test]
    fn skipping_respects_strings_and_escapes() {
        let doc = JsonValue::new(r#"{"a":"]}\"{[,:","b":["\\",{"c":"\"]"}],"d":2}"#);
        assert_eq!(doc.len(), Some(3));
        assert_eq!(doc.get("a").map(|v| v.text()), Some(r#""]}\"{[,:""#));
        assert_eq!(
            doc.get("b").map(|v| v.text()),
            Some(r#"["\\",{"c":"\"]"}]"#)
        );
        assert_eq!(doc.get("d").map(|v| v.text()), Some("2"));
        assert_eq!(at(&doc, "/b/1/c"), Some(r#""\"]""#));
        assert_eq!(at(&doc, "/b/0"), Some(r#""\\""#));
    }

    #[test]
    fn text_and_bool_reads() {
        let s = JsonValue::new(r#""abc""#);
        let text = s.as_text().unwrap();
        assert!(matches!(text, Cow::Borrowed("abc")));
        assert!(std::ptr::eq(text.as_ptr(), s.text()[1..].as_ptr()));
        let escaped = JsonValue::new(r#""a\nb\"\\\b\f\r\t\u0000\u001f/""#)
            .as_text()
            .unwrap();
        assert_eq!(escaped, "a\nb\"\\\u{8}\u{c}\r\t\u{0}\u{1f}/");
        assert!(matches!(escaped, Cow::Owned(_)));
        assert_eq!(JsonValue::new(r#""""#).as_text().as_deref(), Some(""));
        assert_eq!(JsonValue::new(r#""😀é""#).as_text().as_deref(), Some("😀é"));
        assert_eq!(JsonValue::new("true").as_bool(), Some(true));
        assert_eq!(JsonValue::new("false").as_bool(), Some(false));
        // Another class, and `null`, give nothing (JS7).
        for other in ["1", "null", "{}", "[]", r#""true""#] {
            assert_eq!(JsonValue::new(other).as_bool(), None, "{other}");
        }
        for other in ["1", "null", "true", "{}", r#"["a"]"#] {
            assert_eq!(JsonValue::new(other).as_text(), None, "{other}");
        }
        for other in [r#""1""#, "null", "true", "{}", "[1]"] {
            let v = JsonValue::new(other);
            assert_eq!(v.as_i64(), None, "{other}");
            assert_eq!(v.as_u64(), None, "{other}");
            assert_eq!(v.as_f64(), None, "{other}");
            assert_eq!(v.as_decimal(10, 2).unwrap(), None, "{other}");
        }
    }

    /// Navigation terminates without overflowing the stack on 100,000 levels, which a normalized document never has
    /// (at most 128, JS2) but a damaged row may (JS5): skipping a value is iterative.
    #[test]
    fn deep_nesting_terminates_without_overflowing_the_stack() {
        const LEVELS: usize = 100_000;
        let arrays = format!("{}{}", "[".repeat(LEVELS), "]".repeat(LEVELS));
        let doc = JsonValue::new(&arrays);
        assert_eq!(doc.kind(), JsonKind::Array);
        assert_eq!(doc.len(), Some(1));
        assert_eq!(doc.element(0).unwrap().text().len(), arrays.len() - 2);
        assert_eq!(at(&doc, "/0/0/0").map(str::len), Some(arrays.len() - 6));
        assert_eq!(at(&doc, "/1"), None);

        let objects = format!("{}1{}", r#"{"a":"#.repeat(LEVELS), "}".repeat(LEVELS));
        let doc = JsonValue::new(&objects);
        assert_eq!(doc.len(), Some(1));
        assert_eq!(doc.get("a").unwrap().kind(), JsonKind::Object);
        assert_eq!(doc.get("b"), None);

        // Unclosed, as damage may leave it.
        let unclosed = "[".repeat(LEVELS);
        let doc = JsonValue::new(&unclosed);
        assert_eq!(doc.len(), Some(1));
        assert_eq!(at(&doc, "/0/0").map(str::len), Some(LEVELS - 2));
        let unclosed = format!("[\"{}", "\\".repeat(LEVELS));
        assert_eq!(JsonValue::new(&unclosed).len(), Some(1));
    }

    /// Navigation and the typed reads terminate on any text, normalized or not (JS5); this one also never panics.
    #[test]
    fn navigation_of_damaged_texts_terminates() {
        let alphabet: Vec<char> = r#"{}[]",:\a0-1.eE+Tz~/ é"#.chars().collect();
        let mut rng = Rng(0x5eed_0002);
        for _ in 0..20_000 {
            let len = rng.below(24) as usize;
            let text: String = (0..len).map(|_| rng.pick(&alphabet)).collect();
            let v = JsonValue::new(&text);
            let _ = v.kind();
            let _ = v.len();
            let _ = v.members().map(|m| m.count());
            let _ = v.elements().map(|e| e.count());
            let _ = v.get("a");
            let _ = v.element(1);
            let _ = v.pointer("/a/0/");
            let _ = v.as_text();
            let _ = v.as_bool();
            let _ = v.as_i64();
            let _ = v.as_u64();
            let _ = v.as_f64();
            let _ = v.as_decimal(38, 4);
            let _ = v.as_date();
            let _ = v.as_timestamp();
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // AC-JS11 (2): a reference model written directly as normalized text, never through a JSON parser.

    /// SplitMix64, as in `tests/table_encoding.rs`.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }

        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }

        fn pick<T: Copy>(&mut self, items: &[T]) -> T {
            items[self.below(items.len() as u64) as usize]
        }

        fn chance(&mut self, one_in: u64) -> bool {
            self.below(one_in) == 0
        }
    }

    /// The reference model of AC-JS11 (2): numbers by their text, properties in order.
    #[derive(Debug)]
    enum Model {
        Null,
        Bool(bool),
        Number(String),
        String(String),
        Array(Vec<Model>),
        Object(Vec<(String, Model)>),
    }

    const STRING_CHARS: &[char] = &[
        'a', 'z', 'K', '0', ' ', '"', '\\', '/', '~', ':', ',', '{', '}', '[', ']', '\u{0}',
        '\u{8}', '\u{9}', '\u{a}', '\u{c}', '\u{d}', '\u{1f}', '\u{7f}', '\u{e9}', '\u{301}', '€',
        '😀', '\u{fffe}',
    ];

    fn random_string(rng: &mut Rng) -> String {
        (0..rng.below(8)).map(|_| rng.pick(STRING_CHARS)).collect()
    }

    fn random_digits(rng: &mut Rng, max: u64) -> String {
        (0..=rng.below(max))
            .map(|_| rng.pick(&['0', '1', '5', '9']))
            .collect()
    }

    fn random_number(rng: &mut Rng) -> String {
        let mut text = String::new();
        if rng.chance(2) {
            text.push('-');
        }
        if rng.chance(3) {
            text.push('0');
        } else {
            text.push(rng.pick(&['1', '2', '7', '9']));
            if rng.chance(2) {
                text.push_str(&random_digits(rng, 30));
            }
        }
        if rng.chance(2) {
            text.push('.');
            text.push_str(&random_digits(rng, 30));
        }
        if rng.chance(2) {
            text.push(rng.pick(&['e', 'E']));
            if let Some(sign) = [None, Some('+'), Some('-')][rng.below(3) as usize] {
                text.push(sign);
            }
            text.push_str(&random_digits(rng, 5));
        }
        text
    }

    fn random_model(rng: &mut Rng, depth: u32) -> Model {
        let kinds = if depth == 0 { 4 } else { 6 };
        match rng.below(kinds) {
            0 => Model::Null,
            1 => Model::Bool(rng.chance(2)),
            2 => Model::Number(random_number(rng)),
            3 => Model::String(random_string(rng)),
            4 => Model::Array(
                (0..rng.below(6))
                    .map(|_| random_model(rng, depth - 1))
                    .collect(),
            ),
            _ => {
                let mut members: Vec<(String, Model)> = Vec::new();
                for _ in 0..rng.below(6) {
                    let name = random_string(rng);
                    if members.iter().all(|(n, _)| *n != name) {
                        members.push((name, random_model(rng, depth - 1)));
                    }
                }
                Model::Object(members)
            }
        }
    }

    /// A string in the escape form of JS4.
    fn write_string(text: &str, out: &mut String) {
        out.push('"');
        for c in text.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\u{8}' => out.push_str("\\b"),
                '\u{c}' => out.push_str("\\f"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if c < ' ' => out.push_str(&format!("\\u{:04x}", u32::from(c))),
                c => out.push(c),
            }
        }
        out.push('"');
    }

    /// The model's normalized text (JS4).
    fn write_model(model: &Model, out: &mut String) {
        match model {
            Model::Null => out.push_str("null"),
            Model::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Model::Number(text) => out.push_str(text),
            Model::String(text) => write_string(text, out),
            Model::Array(elements) => {
                out.push('[');
                for (i, element) in elements.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_model(element, out);
                }
                out.push(']');
            }
            Model::Object(members) => {
                out.push('{');
                for (i, (name, value)) in members.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_string(name, out);
                    out.push(':');
                    write_model(value, out);
                }
                out.push('}');
            }
        }
    }

    fn pointer_token(name: &str) -> String {
        name.replace('~', "~0").replace('/', "~1")
    }

    /// Navigating `value`, reached from `root` by `pointer`, gives back `model`.
    fn check(model: &Model, value: JsonValue<'_>, root: JsonValue<'_>, pointer: &str) {
        let mut expected = String::new();
        write_model(model, &mut expected);
        assert_eq!(value.text(), expected);
        assert_eq!(
            root.pointer(pointer).unwrap().map(|v| v.text()),
            Some(value.text()),
            "{pointer}"
        );
        match model {
            Model::Null => assert_eq!(value.kind(), JsonKind::Null),
            Model::Bool(b) => {
                assert_eq!(value.kind(), JsonKind::Bool);
                assert_eq!(value.as_bool(), Some(*b));
            }
            Model::Number(text) => {
                assert_eq!(value.kind(), JsonKind::Number);
                assert_eq!(value.text(), text);
            }
            Model::String(text) => {
                assert_eq!(value.kind(), JsonKind::String);
                assert_eq!(value.as_text().as_deref(), Some(text.as_str()));
            }
            Model::Array(elements) => {
                assert_eq!(value.kind(), JsonKind::Array);
                assert_eq!(value.len(), Some(elements.len()));
                assert_eq!(value.elements().unwrap().count(), elements.len());
                for (i, (element, got)) in
                    elements.iter().zip(value.elements().unwrap()).enumerate()
                {
                    assert_eq!(value.element(i), Some(got));
                    check(element, got, root, &format!("{pointer}/{i}"));
                }
                assert_eq!(value.element(elements.len()), None);
            }
            Model::Object(members) => {
                assert_eq!(value.kind(), JsonKind::Object);
                assert_eq!(value.len(), Some(members.len()));
                assert_eq!(value.members().unwrap().count(), members.len());
                for ((name, member), (got_name, got)) in
                    members.iter().zip(value.members().unwrap())
                {
                    assert_eq!(got_name, *name);
                    assert_eq!(value.get(name), Some(got));
                    let pointer = format!("{pointer}/{}", pointer_token(name));
                    check(member, got, root, &pointer);
                }
            }
        }
    }

    #[test]
    fn ac_js11_2_navigation_gives_back_the_reference_model() {
        let mut rng = Rng(0x0715_7a5e);
        for _ in 0..2_000 {
            let model = random_model(&mut rng, 5);
            let mut text = String::new();
            write_model(&model, &mut text);
            let doc = JsonValue::new(&text);
            check(&model, doc, doc, "");
        }
    }

    #[test]
    fn ac_js11_4_numbers_beyond_f64_and_u64_keep_their_texts() {
        let numbers = [
            "1e400",
            "-1e-400",
            "123456789012345678901234567890",
            "18446744073709551616",
            "0.10000000000000000000001",
            "0.1",
            "1.0",
            "1",
        ];
        let text = format!("[{}]", numbers.join(","));
        let doc = JsonValue::new(&text);
        let read: Vec<JsonValue<'_>> = doc.elements().unwrap().collect();
        let texts: Vec<&str> = read.iter().map(|v| v.text()).collect();
        assert_eq!(texts, numbers);
        for (i, a) in texts.iter().enumerate() {
            for b in &texts[i + 1..] {
                assert_ne!(a, b);
            }
        }
        let [huge, tiny, big, past_u64, long, tenth, one_point_zero, one] = read[..] else {
            unreachable!()
        };
        for v in [huge, tiny, big, past_u64, long, tenth] {
            assert_eq!(v.as_i64(), None, "{}", v.text());
            assert_eq!(v.as_u64(), None, "{}", v.text());
        }
        assert_eq!(huge.as_f64(), None);
        assert_eq!(huge.as_decimal(38, 0).unwrap(), None);
        assert_eq!(tiny.as_f64().map(f64::to_bits), Some(0));
        assert_eq!(big.as_f64(), Some(1.234_567_890_123_456_8e29));
        assert_eq!(
            big.as_decimal(38, 0).unwrap().map(|d| d.mantissa),
            Some(123_456_789_012_345_678_901_234_567_890)
        );
        assert_eq!(big.as_decimal(29, 0).unwrap(), None);
        assert_eq!(past_u64.as_f64(), Some(18_446_744_073_709_551_616.0));
        assert_eq!(long.as_f64(), Some(0.1));
        assert_eq!(
            long.as_decimal(23, 23).unwrap().map(|d| d.mantissa),
            Some(10_000_000_000_000_000_000_001)
        );
        assert_eq!(
            tenth.as_decimal(23, 23).unwrap().map(|d| d.mantissa),
            Some(10_000_000_000_000_000_000_000)
        );
        assert_eq!(one_point_zero.as_i64(), Some(1));
        assert_eq!(one.as_i64(), Some(1));
        assert_eq!(one_point_zero.as_u64(), Some(1));
    }
}
