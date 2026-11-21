//! The documents of the reader's tests: a small deterministic generator, a test-only tree of JSON values whose numbers
//! are their exact text, the normalized text of a tree written by its own rules (JS4), and the same tree written with
//! random legal whitespace and escapes. Nothing here goes through a JSON parser.

use std::collections::HashSet;
use std::fmt::Write as _;

/// SplitMix64: a small, deterministic generator (the pattern of `table_encoding.rs`).
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    pub fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u64) as usize]
    }

    pub fn chance(&mut self, one_in: u64) -> bool {
        self.below(one_in) == 0
    }
}

/// A JSON value as the tests model it: numbers are their text, members keep their order.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Null,
    Bool(bool),
    Number(String),
    Str(String),
    Array(Vec<Node>),
    Object(Vec<(String, Node)>),
}

/// The characters strings are made of: ASCII, the characters JSON must escape, a slash, and characters of two, three
/// and four bytes of UTF-8, with U+0000, a combining accent and noncharacters among them.
const CHARS: &[char] = &[
    'a',
    'b',
    'z',
    'A',
    '0',
    ' ',
    '~',
    '/',
    '"',
    '\\',
    '\u{0}',
    '\u{1}',
    '\u{8}',
    '\u{9}',
    '\u{a}',
    '\u{c}',
    '\u{d}',
    '\u{1f}',
    '\u{7f}',
    'é',
    '\u{301}',
    'ß',
    '€',
    '\u{fffe}',
    '\u{ffff}',
    '\u{fdd0}',
    '😀',
    '\u{10ffff}',
    '𝄞',
];

/// Number texts of every form of the RFC 8259 grammar, including some that no `f64` or `u64` holds.
const NUMBERS: &[&str] = &[
    "0",
    "-0",
    "1",
    "-1",
    "1.0",
    "1.50e+3",
    "0.1",
    "0.10000000000000000000001",
    "1E400",
    "-1e-400",
    "123456789012345678901234567890",
    "18446744073709551616",
    "9223372036854775807",
    "-9223372036854775808",
    "2.5E-7",
    "0e0",
    "10",
];

pub fn random_number(rng: &mut Rng) -> String {
    if rng.chance(3) {
        return rng.pick(NUMBERS).to_string();
    }
    let mut text = String::new();
    if rng.chance(3) {
        text.push('-');
    }
    if rng.chance(4) {
        text.push('0');
    } else {
        text.push(char::from(b'1' + rng.below(9) as u8));
        for _ in 0..rng.below(6) {
            text.push(char::from(b'0' + rng.below(10) as u8));
        }
    }
    if rng.chance(2) {
        text.push('.');
        for _ in 0..=rng.below(5) {
            text.push(char::from(b'0' + rng.below(10) as u8));
        }
    }
    if rng.chance(3) {
        text.push(rng.pick(&['e', 'E']));
        match rng.below(3) {
            0 => text.push('+'),
            1 => text.push('-'),
            _ => {}
        }
        for _ in 0..=rng.below(3) {
            text.push(char::from(b'0' + rng.below(10) as u8));
        }
    }
    text
}

pub fn random_string(rng: &mut Rng, max_len: u64) -> String {
    (0..rng.below(max_len + 1))
        .map(|_| rng.pick(CHARS))
        .collect()
}

/// A random document of at most `depth` nested containers, with every kind of value, and names unique per object.
pub fn random_node(rng: &mut Rng, depth: u32) -> Node {
    let kinds = if depth == 0 { 4 } else { 6 };
    match rng.below(kinds) {
        0 => [Node::Null, Node::Bool(true), Node::Bool(false)][rng.below(3) as usize].clone(),
        1 => Node::Number(random_number(rng)),
        2 | 3 => Node::Str(random_string(rng, 6)),
        4 => Node::Array(
            (0..rng.below(5))
                .map(|_| random_node(rng, depth - 1))
                .collect(),
        ),
        _ => {
            let mut seen = HashSet::new();
            let mut members = Vec::new();
            for _ in 0..rng.below(5) {
                let name = random_string(rng, 3);
                if seen.insert(name.clone()) {
                    members.push((name, random_node(rng, depth - 1)));
                }
            }
            Node::Object(members)
        }
    }
}

/// A string as JS4 (4) writes it: raw UTF-8 except `"`, `\` and U+0000 to U+001F.
pub fn normalized_string(text: &str, out: &mut String) {
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
            '\u{0}'..='\u{1f}' => write!(out, "\\u{:04x}", c as u32).unwrap(),
            _ => out.push(c),
        }
    }
    out.push('"');
}

impl Node {
    /// The normalized text of this value (JS4), written by the test's own rules.
    pub fn normalized(&self) -> String {
        let mut out = String::new();
        self.write_normalized(&mut out);
        out
    }

    fn write_normalized(&self, out: &mut String) {
        match self {
            Node::Null => out.push_str("null"),
            Node::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Node::Number(text) => out.push_str(text),
            Node::Str(text) => normalized_string(text, out),
            Node::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write_normalized(out);
                }
                out.push(']');
            }
            Node::Object(members) => {
                out.push('{');
                for (i, (name, value)) in members.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    normalized_string(name, out);
                    out.push(':');
                    value.write_normalized(out);
                }
                out.push('}');
            }
        }
    }

    /// This value indented by two spaces per level, one member or element per line.
    pub fn indented(&self) -> String {
        let mut out = String::new();
        self.write_indented(&mut out, 0);
        out.push('\n');
        out
    }

    fn write_indented(&self, out: &mut String, level: usize) {
        let pad = |out: &mut String, level: usize| {
            out.push('\n');
            out.push_str(&"  ".repeat(level));
        };
        match self {
            Node::Array(items) if !items.is_empty() => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    pad(out, level + 1);
                    item.write_indented(out, level + 1);
                }
                pad(out, level);
                out.push(']');
            }
            Node::Object(members) if !members.is_empty() => {
                out.push('{');
                for (i, (name, value)) in members.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    pad(out, level + 1);
                    normalized_string(name, out);
                    out.push_str(": ");
                    value.write_indented(out, level + 1);
                }
                pad(out, level);
                out.push('}');
            }
            _ => self.write_normalized(out),
        }
    }

    /// This value written with random legal whitespace around every token and random legal escapes in every string.
    pub fn rewritten(&self, rng: &mut Rng) -> String {
        let mut out = String::new();
        space(rng, &mut out);
        self.write_random(rng, &mut out);
        space(rng, &mut out);
        out
    }

    fn write_random(&self, rng: &mut Rng, out: &mut String) {
        match self {
            Node::Str(text) => random_string_text(rng, text, out),
            Node::Array(items) => {
                out.push('[');
                space(rng, out);
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                        space(rng, out);
                    }
                    item.write_random(rng, out);
                    space(rng, out);
                }
                out.push(']');
            }
            Node::Object(members) => {
                out.push('{');
                space(rng, out);
                for (i, (name, value)) in members.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                        space(rng, out);
                    }
                    random_string_text(rng, name, out);
                    space(rng, out);
                    out.push(':');
                    space(rng, out);
                    value.write_random(rng, out);
                    space(rng, out);
                }
                out.push('}');
            }
            _ => self.write_normalized(out),
        }
    }
}

/// Up to three of the four whitespace characters of RFC 8259, at random.
fn space(rng: &mut Rng, out: &mut String) {
    for _ in 0..rng.below(4) {
        out.push(rng.pick(&[' ', '\t', '\n', '\r']));
    }
}

/// A string with each character written raw when JSON allows it, as its short escape, or as `\uXXXX` (a surrogate
/// pair beyond the BMP) in lower or upper case, at random.
fn random_string_text(rng: &mut Rng, text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        let short = match c {
            '"' => Some("\\\""),
            '\\' => Some("\\\\"),
            '/' => Some("\\/"),
            '\u{8}' => Some("\\b"),
            '\u{c}' => Some("\\f"),
            '\n' => Some("\\n"),
            '\r' => Some("\\r"),
            '\t' => Some("\\t"),
            _ => None,
        };
        let raw_allowed = !matches!(c, '"' | '\\' | '\u{0}'..='\u{1f}');
        match (rng.below(3), short) {
            (0, _) if raw_allowed => out.push(c),
            (1, Some(short)) => out.push_str(short),
            _ => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    if rng.chance(2) {
                        write!(out, "\\u{unit:04x}").unwrap();
                    } else {
                        write!(out, "\\u{unit:04X}").unwrap();
                    }
                }
            }
        }
    }
    out.push('"');
}
