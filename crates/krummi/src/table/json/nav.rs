//! Navigation of a normalized JSON document (JS6) and the typed reads of its values (JS7).

use std::borrow::Cow;

use krummi_pager::Result;

use crate::table::value::Decimal;

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
    rest: &'a str,
}

/// The elements of an array in order.
#[derive(Debug, Clone)]
pub struct Elements<'a> {
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
        todo!("T3: navigation")
    }

    /// Its normalized text, a valid normalized JSON text itself (a string's includes its quotes).
    pub fn text(&self) -> &'a str {
        self.text
    }

    /// The number of properties of an object or elements of an array; `None` for another class.
    pub fn len(&self) -> Option<usize> {
        todo!("T3: navigation")
    }

    /// Whether an object has no properties or an array no elements; `None` for another class.
    pub fn is_empty(&self) -> Option<bool> {
        self.len().map(|len| len == 0)
    }

    /// The properties of an object; `None` for another class.
    pub fn members(&self) -> Option<Members<'a>> {
        let _ = Members { rest: self.text };
        todo!("T3: navigation")
    }

    /// The value of the property with this name, compared with the decoded names (JS2); `None` when it is absent or
    /// this is not an object.
    pub fn get(&self, name: &str) -> Option<JsonValue<'a>> {
        let _ = name;
        todo!("T3: navigation")
    }

    /// The elements of an array; `None` for another class.
    pub fn elements(&self) -> Option<Elements<'a>> {
        let _ = Elements { rest: self.text };
        todo!("T3: navigation")
    }

    /// The element at `index`, from 0; `None` when it is absent or this is not an array.
    pub fn element(&self, index: usize) -> Option<JsonValue<'a>> {
        let _ = index;
        todo!("T3: navigation")
    }

    /// The value of an RFC 6901 pointer relative to this value (JS6): `Ok(None)` when it is absent, and invalid value
    /// with the rule malformed JSON pointer for a malformed pointer, checked before navigating.
    pub fn pointer(&self, pointer: &str) -> Result<Option<JsonValue<'a>>> {
        let _ = pointer;
        todo!("T3: navigation")
    }

    /// A string as text, without its quotes: borrowed when it has no escapes, decoded into a copy when it has (JS7).
    pub fn as_text(&self) -> Option<Cow<'a, str>> {
        todo!("T3: typed reads")
    }

    /// `true` or `false`.
    pub fn as_bool(&self) -> Option<bool> {
        todo!("T3: typed reads")
    }

    /// A number whose exact value is an integer that `i64` holds, in any notation (JS7).
    pub fn as_i64(&self) -> Option<i64> {
        todo!("T3: typed reads")
    }

    /// A number whose exact value is an integer that `u64` holds, in any notation (JS7).
    pub fn as_u64(&self) -> Option<u64> {
        todo!("T3: typed reads")
    }

    /// A number rounded to the nearest `f64`, `-0` as `+0`; `None` outside the finite range (JS7).
    pub fn as_f64(&self) -> Option<f64> {
        todo!("T3: typed reads")
    }

    /// A number rounded to `scale` as E8 rounds, `None` when it then has more than `precision` digits (JS7); invalid
    /// value with the rule invalid read scale when `precision` or `scale` is outside 1 ≤ p ≤ 38, 0 ≤ s ≤ p, checked
    /// before the value.
    pub fn as_decimal(&self, precision: u8, scale: u8) -> Result<Option<Decimal>> {
        let _ = (precision, scale);
        todo!("T3: typed reads")
    }

    /// A string `YYYY-MM-DD` in the domain of E8, as days since 1970-01-01 (JS7).
    pub fn as_date(&self) -> Option<i32> {
        todo!("T3: typed reads")
    }

    /// An RFC 3339 instant with an offset, converted to UTC and rounded to the microsecond, in the domain of E8, as
    /// nanoseconds since the Unix epoch (JS7).
    pub fn as_timestamp(&self) -> Option<i128> {
        todo!("T3: typed reads")
    }
}

impl<'a> Iterator for Members<'a> {
    type Item = (Cow<'a, str>, JsonValue<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        let _ = self.rest;
        todo!("T3: navigation")
    }
}

impl<'a> Iterator for Elements<'a> {
    type Item = JsonValue<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let _ = self.rest;
        todo!("T3: navigation")
    }
}
