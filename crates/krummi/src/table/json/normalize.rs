//! The reader of JSON texts (JS2, JS4, JS8, JS14): one pass that validates a text and writes its normalized form.
//!
//! The standard library validates the UTF-8 (`std::str::from_utf8`); the reader then scans the valid prefix with an
//! explicit stack bounded by 128 levels (I6), so no input can overflow the thread's stack. Each object's names are
//! kept, in their normalized form, on a stack shared by the open objects; they are checked for repeats by sorting
//! them when the object closes, so validation is O(n log n) at worst (I7). When the scan stops at a fault, the
//! repeats already present in the open objects are also faults, at smaller positions, so the reported fault is the
//! smallest of all three (JS8).

use std::fmt::Write as _;

use krummi_pager::{JsonCause, JsonFault};

/// The most objects and arrays a document nests (JS2).
const MAX_DEPTH: usize = 128;

/// Validates `input` as a JSON document (JS2) and returns its normalized text (JS4), or the fault of JS8 with the
/// smallest position.
pub fn normalize(input: &[u8]) -> Result<String, JsonFault> {
    let (text, invalid_utf8) = match std::str::from_utf8(input) {
        Ok(text) => (text, None),
        Err(e) => {
            let valid = e.valid_up_to();
            let text = std::str::from_utf8(&input[..valid])
                .expect("the prefix before an invalid sequence is UTF-8");
            (text, Some(valid))
        }
    };
    let mut reader = Reader {
        text,
        bytes: text.as_bytes(),
        pos: 0,
        out: String::with_capacity(text.len()),
        frames: Vec::new(),
        names: Vec::new(),
    };
    let mut faults = Vec::new();
    match reader.document() {
        Ok(()) => {}
        Err(found) => faults.push(found),
    }
    if let Some(position) = invalid_utf8 {
        // The scan stopped at the end of the valid prefix, so the stack is the one open at the invalid byte.
        faults.push(Found {
            cause: JsonCause::InvalidUtf8,
            position,
            pointer: reader.pointer(reader.frames.len()),
        });
    }
    if faults.is_empty() {
        return Ok(reader.out);
    }
    // Names already read in the open objects precede the fault the scan stopped at.
    for level in 0..reader.frames.len() {
        if let Some(found) = reader.repeat(level) {
            faults.push(found);
        }
    }
    let first = faults
        .into_iter()
        .min_by_key(|f| (f.position, rank(&f.cause)))
        .expect("at least one fault");
    Err(JsonFault {
        cause: first.cause,
        position: first.position as u64,
        pointer: first.pointer.into(),
    })
}

/// The order of the causes at the same position (JS8).
fn rank(cause: &JsonCause) -> u8 {
    match cause {
        JsonCause::InvalidUtf8 => 0,
        JsonCause::Syntax => 1,
        JsonCause::LoneSurrogate => 2,
        JsonCause::RepeatedProperty(_) => 3,
        JsonCause::TooDeep => 4,
        _ => unreachable!("a cause the reader does not produce"),
    }
}

/// A fault the reader found, with its position in bytes and the pointer of the innermost container open there.
struct Found {
    cause: JsonCause,
    position: usize,
    pointer: String,
}

/// An open object or array.
struct Frame {
    object: bool,
    /// For an array, the number of elements started, so the current one is `count - 1`.
    count: usize,
    /// For an object, where its names start in [`Reader::names`], and the range in the output of the current name.
    names_start: usize,
    name: (usize, usize),
}

/// A property name: its normalized text, as a range in the output between its quotes, a hash of that text, and the
/// position of its opening quote in the input. The hash only speeds up sorting; equal hashes still compare the text.
#[derive(Clone, Copy)]
struct Name {
    start: usize,
    end: usize,
    hash: u64,
    quote: usize,
}

impl Name {
    fn text<'a>(&self, out: &'a str) -> &'a [u8] {
        &out.as_bytes()[self.start..self.end]
    }
}

/// FNV-1a, 64 bits.
fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

struct Reader<'a> {
    text: &'a str,
    bytes: &'a [u8],
    pos: usize,
    out: String,
    frames: Vec<Frame>,
    /// The names read so far in the open objects, each object's after its parent's.
    names: Vec<Name>,
}

impl Reader<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn fault(&self, cause: JsonCause, position: usize) -> Found {
        Found {
            cause,
            position,
            pointer: self.pointer(self.frames.len()),
        }
    }

    fn syntax(&self) -> Found {
        self.fault(JsonCause::Syntax, self.pos)
    }

    fn whitespace(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.peek() {
            self.pos += 1;
        }
    }

    /// Reads the whole text: one value with optional whitespace around it.
    fn document(&mut self) -> Result<(), Found> {
        self.whitespace();
        'value: loop {
            // A value starts here.
            match self.peek() {
                Some(b'{') => {
                    self.open(true)?;
                    self.whitespace();
                    if self.peek() == Some(b'}') {
                        self.close()?;
                    } else {
                        self.member()?;
                        continue 'value;
                    }
                }
                Some(b'[') => {
                    self.open(false)?;
                    self.whitespace();
                    if self.peek() == Some(b']') {
                        self.close()?;
                    } else {
                        self.frames.last_mut().expect("the array just opened").count = 1;
                        continue 'value;
                    }
                }
                Some(b'"') => {
                    self.string()?;
                }
                Some(b't') => self.literal("true")?,
                Some(b'f') => self.literal("false")?,
                Some(b'n') => self.literal("null")?,
                Some(b'-' | b'0'..=b'9') => self.number()?,
                _ => return Err(self.syntax()),
            }
            // A value ended here: what follows it in its container.
            loop {
                self.whitespace();
                let next = self.peek();
                let Some(frame) = self.frames.last_mut() else {
                    return if next.is_none() {
                        Ok(())
                    } else {
                        Err(self.syntax())
                    };
                };
                match (next, frame.object) {
                    (Some(b','), false) => {
                        frame.count += 1;
                        self.pos += 1;
                        self.out.push(',');
                        self.whitespace();
                        continue 'value;
                    }
                    (Some(b','), true) => {
                        self.pos += 1;
                        self.out.push(',');
                        self.whitespace();
                        self.member()?;
                        continue 'value;
                    }
                    (Some(b']'), false) | (Some(b'}'), true) => self.close()?,
                    _ => return Err(self.syntax()),
                }
            }
        }
    }

    /// Opens an object or an array at its `{` or `[`.
    fn open(&mut self, object: bool) -> Result<(), Found> {
        if self.frames.len() >= MAX_DEPTH {
            return Err(self.fault(JsonCause::TooDeep, self.pos));
        }
        self.frames.push(Frame {
            object,
            count: 0,
            names_start: self.names.len(),
            name: (0, 0),
        });
        self.out.push(if object { '{' } else { '[' });
        self.pos += 1;
        Ok(())
    }

    /// Closes the innermost container at its `}` or `]`, once its names have no repeat.
    fn close(&mut self) -> Result<(), Found> {
        let level = self.frames.len() - 1;
        if let Some(found) = self.repeat(level) {
            return Err(found);
        }
        let frame = self.frames.pop().expect("an open container");
        self.names.truncate(frame.names_start);
        self.out.push(if frame.object { '}' } else { ']' });
        self.pos += 1;
        Ok(())
    }

    /// Reads a member's name and its colon, up to its value.
    fn member(&mut self) -> Result<(), Found> {
        if self.peek() != Some(b'"') {
            return Err(self.syntax());
        }
        let quote = self.pos;
        let start = self.out.len() + 1;
        self.string()?;
        let end = self.out.len() - 1;
        let name = Name {
            start,
            end,
            hash: hash(&self.out.as_bytes()[start..end]),
            quote,
        };
        self.names.push(name);
        self.frames.last_mut().expect("an open object").name = (name.start, name.end);
        self.whitespace();
        if self.peek() != Some(b':') {
            return Err(self.syntax());
        }
        self.pos += 1;
        self.out.push(':');
        self.whitespace();
        Ok(())
    }

    /// The first repeated name of the object at `level` (none for an array): the one whose opening quote comes first
    /// among the names equal to an earlier one. Sorts the object's names, which only a closing or a fault reads.
    fn repeat(&mut self, level: usize) -> Option<Found> {
        let frame = &self.frames[level];
        if !frame.object {
            return None;
        }
        let end = self.frames[level + 1..]
            .iter()
            .find(|f| f.object)
            .map_or(self.names.len(), |f| f.names_start);
        let names = &mut self.names[frame.names_start..end];
        let out = &self.out;
        names.sort_unstable_by(|a, b| {
            (a.hash.cmp(&b.hash))
                .then_with(|| a.text(out).cmp(b.text(out)))
                .then(a.quote.cmp(&b.quote))
        });
        let mut first: Option<Name> = None;
        for pair in names.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if a.hash == b.hash
                && a.text(out) == b.text(out)
                && first.is_none_or(|f| b.quote < f.quote)
            {
                first = Some(b);
            }
        }
        let name = first?;
        Some(Found {
            cause: JsonCause::RepeatedProperty(decode(&self.out[name.start..name.end]).into()),
            position: name.quote,
            pointer: self.pointer(level + 1),
        })
    }

    /// The RFC 6901 pointer of the container at `levels - 1`, `""` for the root or none: the steps of the containers
    /// above it to their current child.
    fn pointer(&self, levels: usize) -> String {
        let mut pointer = String::new();
        for frame in &self.frames[..levels.saturating_sub(1)] {
            pointer.push('/');
            if frame.object {
                for c in decode(&self.out[frame.name.0..frame.name.1]).chars() {
                    match c {
                        '~' => pointer.push_str("~0"),
                        '/' => pointer.push_str("~1"),
                        _ => pointer.push(c),
                    }
                }
            } else {
                write!(pointer, "{}", frame.count - 1).expect("writing to a string");
            }
        }
        pointer
    }

    /// Reads `true`, `false` or `null`.
    fn literal(&mut self, word: &str) -> Result<(), Found> {
        for &expected in word.as_bytes() {
            if self.peek() != Some(expected) {
                return Err(self.syntax());
            }
            self.pos += 1;
        }
        self.out.push_str(word);
        Ok(())
    }

    fn digits(&mut self) {
        while let Some(b'0'..=b'9') = self.peek() {
            self.pos += 1;
        }
    }

    /// One or more digits.
    fn some_digits(&mut self) -> Result<(), Found> {
        if !matches!(self.peek(), Some(b'0'..=b'9')) {
            return Err(self.syntax());
        }
        self.digits();
        Ok(())
    }

    /// Reads a number of RFC 8259's grammar, `-? (0 | [1-9][0-9]*) (.[0-9]+)? ([eE][+-]?[0-9]+)?`, and copies its text.
    fn number(&mut self) -> Result<(), Found> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => self.digits(),
            _ => return Err(self.syntax()),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            self.some_digits()?;
        }
        if let Some(b'e' | b'E') = self.peek() {
            self.pos += 1;
            if let Some(b'+' | b'-') = self.peek() {
                self.pos += 1;
            }
            self.some_digits()?;
        }
        self.out.push_str(&self.text[start..self.pos]);
        Ok(())
    }

    /// Four hexadecimal digits at the reader's position, as a UTF-16 code unit.
    fn hex4(&mut self) -> Result<u32, Found> {
        let mut unit = 0;
        for _ in 0..4 {
            let digit = self
                .peek()
                .and_then(|b| char::from(b).to_digit(16))
                .ok_or_else(|| self.syntax())?;
            unit = unit * 16 + digit;
            self.pos += 1;
        }
        Ok(unit)
    }

    /// The low surrogate of an escape `\uDC00` to `\uDFFF` at `at`, if there is one.
    fn low_surrogate(&self, at: usize) -> Option<u32> {
        let escape = self.bytes.get(at..at + 6)?;
        if &escape[..2] != b"\\u" {
            return None;
        }
        let unit = escape[2..]
            .iter()
            .try_fold(0, |unit, &b| Some(unit * 16 + char::from(b).to_digit(16)?))?;
        (0xdc00..=0xdfff).contains(&unit).then_some(unit)
    }

    /// Reads a string at its opening quote and writes its normalized text (JS4 (4)).
    fn string(&mut self) -> Result<(), Found> {
        self.pos += 1;
        self.out.push('"');
        loop {
            let run = self.pos;
            while let Some(b) = self.peek() {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.pos += 1;
            }
            self.out.push_str(&self.text[run..self.pos]);
            match self.peek() {
                Some(b'"') => {
                    self.pos += 1;
                    self.out.push('"');
                    return Ok(());
                }
                Some(b'\\') => self.escape()?,
                // The end of the text, or a control character, which a string cannot hold unescaped.
                _ => return Err(self.syntax()),
            }
        }
    }

    /// Reads an escape at its `\` and writes its character as the normalized text does.
    fn escape(&mut self) -> Result<(), Found> {
        let backslash = self.pos;
        self.pos += 1;
        let c = match self.peek() {
            Some(b'"') => '"',
            Some(b'\\') => '\\',
            Some(b'/') => '/',
            Some(b'b') => '\u{8}',
            Some(b'f') => '\u{c}',
            Some(b'n') => '\n',
            Some(b'r') => '\r',
            Some(b't') => '\t',
            Some(b'u') => {
                self.pos += 1;
                let unit = self.hex4()?;
                let c = match unit {
                    0xd800..=0xdbff => {
                        let Some(low) = self.low_surrogate(self.pos) else {
                            return Err(self.fault(JsonCause::LoneSurrogate, backslash));
                        };
                        self.pos += 6;
                        char::from_u32(0x10000 + ((unit - 0xd800) << 10) + (low - 0xdc00))
                    }
                    0xdc00..=0xdfff => return Err(self.fault(JsonCause::LoneSurrogate, backslash)),
                    _ => char::from_u32(unit),
                };
                push_escaped(&mut self.out, c.expect("a scalar value"));
                return Ok(());
            }
            _ => return Err(self.syntax()),
        };
        self.pos += 1;
        push_escaped(&mut self.out, c);
        Ok(())
    }
}

/// Writes a character of a string as the normalized text does (JS4 (4)): as it is in UTF-8, except `"`, `\` and the
/// controls U+0000 to U+001F.
fn push_escaped(out: &mut String, c: char) {
    match c {
        '"' => out.push_str("\\\""),
        '\\' => out.push_str("\\\\"),
        '\u{8}' => out.push_str("\\b"),
        '\u{c}' => out.push_str("\\f"),
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        '\u{0}'..='\u{1f}' => write!(out, "\\u{:04x}", c as u32).expect("writing to a string"),
        _ => out.push(c),
    }
}

/// The characters of a normalized string's text between its quotes, with its escapes decoded.
fn decode(normalized: &str) -> String {
    let mut decoded = String::with_capacity(normalized.len());
    let mut chars = normalized.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            decoded.push(c);
            continue;
        }
        let c = match chars.next() {
            Some('b') => '\u{8}',
            Some('f') => '\u{c}',
            Some('n') => '\n',
            Some('r') => '\r',
            Some('t') => '\t',
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                char::from_u32(u32::from_str_radix(&hex, 16).expect("a normalized escape"))
                    .expect("a control")
            }
            Some(c) => c,
            None => unreachable!("a normalized string does not end in a backslash"),
        };
        decoded.push(c);
    }
    decoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_reverses_the_escapes_of_the_normalized_text() {
        let all: String = (0u32..0x80)
            .filter_map(char::from_u32)
            .chain(['é', '😀'])
            .collect();
        let mut normalized = String::new();
        for c in all.chars() {
            push_escaped(&mut normalized, c);
        }
        assert_eq!(decode(&normalized), all);
    }

    #[test]
    fn the_escape_table_is_that_of_js4() {
        let mut out = String::new();
        for c in [
            '"', '\\', '/', '\u{8}', '\u{c}', '\n', '\r', '\t', '\u{0}', '\u{1f}', '\u{7f}', 'é',
        ] {
            push_escaped(&mut out, c);
        }
        assert_eq!(out, "\\\"\\\\/\\b\\f\\n\\r\\t\\u0000\\u001f\u{7f}é");
    }

    #[test]
    fn pointers_escape_tilde_and_slash() {
        let fault = normalize(br#"{"a~/b":{"":[0,{"~1":[x]}]}}"#).unwrap_err();
        assert_eq!(&*fault.pointer, "/a~0~1b//1/~01");
    }
}
