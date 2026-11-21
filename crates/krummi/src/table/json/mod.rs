//! JSON columns (7.5, `docs/prd/krummi/tables/tables-json.prd.md`): the value an application gives a JSON column,
//! the reader that validates and normalizes its text (JS2, JS4, JS8), and the view that navigates a normalized
//! document and reads its values as Krummi types (JS6, JS7). Technical starting state: document 13 of the table design.

use std::borrow::Cow;

mod nav;
mod normalize;
mod number;
mod time;

pub use nav::{Elements, JsonKind, JsonValue, Members};
#[doc(hidden)]
pub use normalize::normalize;

/// The document whose normalized text is `text`, for the fuzz targets of the reader and the navigation (AC-JS11): the
/// text is trusted as [`JsonValue`] trusts it, so it should come from [`normalize`].
#[doc(hidden)]
pub fn trusted_document(text: &str) -> JsonValue<'_> {
    JsonValue::new(text)
}

/// The first byte of a stored JSON value in stored form v1 (document 13, *Stored form v1*): form 1, normalized text,
/// with no navigation index (bit 4 clear) and the reserved bits zero. The normalized text follows it.
pub(crate) const FORM_TEXT_V1: u8 = 0x01;

/// The value of a JSON column (JS3, JS5): as given by an application, a text or bytes that a write validates and
/// normalizes; as read back, the document's normalized text (JS4), borrowed from the row or the index entry when it
/// can be (D11).
///
/// Two values are equal when their bytes are: a document read back equals one given with its normalized text.
#[derive(Debug, Clone)]
pub struct Json<'a> {
    bytes: Cow<'a, [u8]>,
    normalized: bool,
}

impl PartialEq for Json<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes
    }
}

impl Eq for Json<'_> {}

impl std::hash::Hash for Json<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.bytes.hash(state);
    }
}

impl<'a> Json<'a> {
    /// A document given as text, validated when it is written (JS3).
    pub fn new(text: impl Into<Cow<'a, str>>) -> Self {
        let bytes = match text.into() {
            Cow::Borrowed(text) => Cow::Borrowed(text.as_bytes()),
            Cow::Owned(text) => Cow::Owned(text.into_bytes()),
        };
        Json {
            bytes,
            normalized: false,
        }
    }

    /// A document given as bytes, which may not be valid UTF-8, validated when it is written (JS3).
    pub fn from_bytes(bytes: impl Into<Cow<'a, [u8]>>) -> Self {
        Json {
            bytes: bytes.into(),
            normalized: false,
        }
    }

    /// A normalized text, as read back or produced by [`normalize`].
    pub(crate) fn normalized(text: Cow<'a, str>) -> Self {
        let mut json = Json::new(text);
        json.normalized = true;
        json
    }

    /// The bytes of the document: as given, or its normalized text when it was read back.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The normalized text of a document read back (JS4, JS5), or `None` for one given by the application and not
    /// yet written.
    pub fn text(&self) -> Option<&str> {
        if self.normalized {
            Some(std::str::from_utf8(&self.bytes).expect("a normalized JSON text is UTF-8"))
        } else {
            None
        }
    }

    /// The document of a value read back, to navigate it (JS6) and read its values (JS7), or `None` for one given by
    /// the application and not yet written.
    pub fn document(&self) -> Option<JsonValue<'_>> {
        self.text().map(JsonValue::new)
    }

    /// Whether this is a normalized text (read back), not a text given by the application.
    pub(crate) fn is_normalized(&self) -> bool {
        self.normalized
    }

    /// The value borrowing this one's bytes, without copying them.
    pub fn reborrow(&self) -> Json<'_> {
        Json {
            bytes: Cow::Borrowed(&self.bytes),
            normalized: self.normalized,
        }
    }

    /// The value with its bytes copied, so that it outlives what it borrowed.
    pub fn into_owned(self) -> Json<'static> {
        Json {
            bytes: Cow::Owned(self.bytes.into_owned()),
            normalized: self.normalized,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::Json;

    #[test]
    fn values_with_the_same_bytes_are_equal_whether_given_or_read_back() {
        let read_back = Json::normalized(Cow::Borrowed("[1]"));
        assert_eq!(read_back, Json::new("[1]"));
        assert_eq!(read_back, Json::from_bytes(&b"[1]"[..]));
        assert_ne!(read_back, Json::new("[ 1 ]"));
    }
}
