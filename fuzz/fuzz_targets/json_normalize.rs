//! The JSON reader over arbitrary bytes (AC-JS11 (9)): it never panics or overflows the stack; an accepted text
//! normalizes to itself, is never longer than its input and is UTF-8.
#![no_main]

use krummi::table::json::normalize;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    match normalize(data) {
        Ok(text) => {
            assert!(
                text.len() <= data.len(),
                "the normalized text is never longer"
            );
            assert_eq!(
                normalize(text.as_bytes()).as_deref(),
                Ok(text.as_str()),
                "normalizing is idempotent"
            );
        }
        Err(fault) => {
            assert!(
                fault.position <= data.len() as u64,
                "the position is inside the text"
            );
        }
    }
});
