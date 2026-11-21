//! The reader of JSON texts (JS2, JS4, JS8, JS14): one pass that validates a text and writes its normalized form.

use krummi_pager::JsonFault;

/// Validates `input` as a JSON document (JS2) and returns its normalized text (JS4), or the fault of JS8 with the
/// smallest position.
pub fn normalize(input: &[u8]) -> Result<String, JsonFault> {
    let _ = input;
    todo!("T2: the JSON reader")
}
