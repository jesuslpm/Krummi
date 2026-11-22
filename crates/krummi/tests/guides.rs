//! Every usage guide with Rust examples is registered in the doctest module of `src/lib.rs`, so its examples run.

use std::fs;
use std::path::Path;

#[test]
fn every_guide_with_rust_examples_runs_as_a_doctest() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib = fs::read_to_string(crate_dir.join("src/lib.rs")).expect("lib.rs");
    let registered: Vec<&str> = lib
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("guide!("))
        .collect();
    let guides = crate_dir.join("../../docs/guides");
    let mut missing = Vec::new();
    for entry in fs::read_dir(&guides).expect("docs/guides") {
        let path = entry.expect("guide entry").path();
        if path.extension().is_none_or(|e| e != "md") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("guide");
        let name = path.file_name().unwrap().to_str().unwrap();
        let entry = format!("\"{name}\");");
        if text.contains("```rust") && !registered.iter().any(|line| line.ends_with(&entry)) {
            missing.push(name.to_owned());
        }
    }
    assert!(
        missing.is_empty(),
        "guides with Rust examples missing from `mod guides` in src/lib.rs: {missing:?}"
    );
}
