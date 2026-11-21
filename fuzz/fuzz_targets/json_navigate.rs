//! Navigation and typed reads over valid documents (AC-JS11 (9)): every value of a document normalized from arbitrary
//! bytes is visited, by iteration, by lookup and by pointer, and read as every type, without a panic.
#![no_main]

use krummi::table::json::{JsonKind, JsonValue, normalize, trusted_document};
use libfuzzer_sys::fuzz_target;

/// Escapes a property name as one step of an RFC 6901 pointer.
fn step(name: &str) -> String {
    format!("/{}", name.replace('~', "~0").replace('/', "~1"))
}

fn read_all(value: JsonValue<'_>) {
    let _ = value.as_text();
    let _ = value.as_bool();
    let _ = value.as_i64();
    let _ = value.as_u64();
    let _ = value.as_f64();
    let _ = value.as_date();
    let _ = value.as_timestamp();
    for (precision, scale) in [(1, 0), (10, 2), (38, 0), (38, 38)] {
        value.as_decimal(precision, scale).expect("valid limits");
    }
    assert!(value.as_decimal(0, 0).is_err() && value.as_decimal(39, 0).is_err());
}

fuzz_target!(|data: &[u8]| {
    let Ok(text) = normalize(data) else { return };
    let document = trusted_document(&text);
    // Each pending value with its pointer from the root; iteration keeps the visit off the stack.
    let mut pending = vec![(document, String::new())];
    while let Some((value, pointer)) = pending.pop() {
        assert_eq!(
            document.pointer(&pointer).expect("a well-formed pointer"),
            Some(value)
        );
        read_all(value);
        match value.kind() {
            JsonKind::Object => {
                let members: Vec<_> = value.members().expect("an object").collect();
                assert_eq!(value.len(), Some(members.len()));
                for (name, member) in members {
                    // Names are unique, so a lookup finds this member.
                    assert_eq!(value.get(&name), Some(member));
                    pending.push((member, pointer.clone() + &step(&name)));
                }
                assert_eq!(value.elements(), None);
            }
            JsonKind::Array => {
                let elements: Vec<_> = value.elements().expect("an array").collect();
                assert_eq!(value.len(), Some(elements.len()));
                for (index, element) in elements.into_iter().enumerate() {
                    assert_eq!(value.element(index), Some(element));
                    pending.push((element, format!("{pointer}/{index}")));
                }
                assert_eq!(value.element(value.len().unwrap_or(0)), None);
            }
            _ => assert_eq!(value.len(), None),
        }
    }
});
