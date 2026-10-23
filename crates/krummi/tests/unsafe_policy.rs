//! D12 / AC21: unsafe code is allowed only in the Pager crate.
//!
//! Two checks: every workspace member except `krummi-pager` inherits the workspace lints, which forbid unsafe
//! code; and no Rust source outside the Pager crate uses the `unsafe` keyword.

use std::fs;
use std::path::{Path, PathBuf};

const PAGER_CRATE: &str = "krummi-pager";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Removes comments and string, char and byte literals, so that only code tokens remain.
fn strip_comments_and_literals(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0;
            while i < chars.len() {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            out.push(' ');
        } else if c == '"' {
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            out.push(' ');
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// Returns `path:line` for every use of the `unsafe` keyword in the Rust files under `dir`.
fn unsafe_uses(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in fs::read_dir(&current).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let code = strip_comments_and_literals(&fs::read_to_string(&path).unwrap());
                for (n, line) in code.lines().enumerate() {
                    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
                    let mut rest = line;
                    while let Some(pos) = rest.find("unsafe") {
                        let before = rest[..pos].chars().last();
                        let after = rest[pos + 6..].chars().next();
                        if !before.is_some_and(is_ident) && !after.is_some_and(is_ident) {
                            found.push(format!("{}:{}", path.display(), n + 1));
                            break;
                        }
                        rest = &rest[pos + 6..];
                    }
                }
            }
        }
    }
    found
}

fn member_dirs() -> Vec<PathBuf> {
    let crates = workspace_root().join("crates");
    let mut dirs: Vec<PathBuf> = fs::read_dir(crates)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("Cargo.toml").exists())
        .collect();
    dirs.sort();
    dirs
}

#[test]
fn the_check_detects_unsafe_code_outside_the_pager() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/unsafe_outside_pager");
    let found = unsafe_uses(&fixture);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].ends_with("lib.rs:3"), "{found:?}");
}

#[test]
fn no_crate_but_the_pager_uses_unsafe_code() {
    let mut found = Vec::new();
    for dir in member_dirs() {
        if dir.file_name().unwrap() == PAGER_CRATE {
            continue;
        }
        // Test fixtures are data for this test, not code of the engine.
        for sub in ["src", "benches", "examples"] {
            if dir.join(sub).exists() {
                found.extend(unsafe_uses(&dir.join(sub)));
            }
        }
        let tests = dir.join("tests");
        if tests.exists() {
            for entry in fs::read_dir(&tests).unwrap() {
                let path = entry.unwrap().path();
                if path.is_file() {
                    let code = strip_comments_and_literals(&fs::read_to_string(&path).unwrap());
                    assert!(
                        !code.contains("unsafe {") && !code.contains("unsafe fn"),
                        "unsafe code in {}",
                        path.display()
                    );
                }
            }
        }
    }
    assert!(found.is_empty(), "unsafe code outside the Pager: {found:?}");
}

#[test]
fn every_crate_but_the_pager_forbids_unsafe_code() {
    let workspace = fs::read_to_string(workspace_root().join("Cargo.toml")).unwrap();
    assert!(
        workspace.contains("[workspace.lints.rust]\nunsafe_code = \"forbid\""),
        "the workspace must forbid unsafe code"
    );
    for dir in member_dirs() {
        let manifest = fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        let is_pager = dir.file_name().unwrap() == PAGER_CRATE;
        let inherits = manifest.contains("[lints]\nworkspace = true");
        assert_eq!(
            inherits,
            !is_pager,
            "{}: only the Pager may opt out of the workspace lints",
            dir.display()
        );
    }
}
