//! K14 / AC-K14 of the benchmark PRD: the benchmark is not part of the engine, so none of its packages, nor the
//! other engines it measures, reach Krummi's library.

use std::path::Path;
use std::process::Command;

#[test]
fn the_library_depends_on_no_package_of_the_benchmark() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let output = Command::new(cargo)
        .args([
            "tree",
            "--offline",
            "--edges",
            "normal,build",
            "--prefix",
            "none",
            "--format",
            "{p}",
            "--manifest-path",
        ])
        .arg(&manifest)
        .args(["-p", "krummi"])
        .output()
        .expect("cargo tree runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8(output.stdout).unwrap();
    let names: Vec<&str> = tree
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    assert!(names.contains(&"krummi"), "{tree}");
    for forbidden in [
        "krummi-bench",
        "heed",
        "lmdb-master-sys",
        "redb",
        "canopydb",
        "fjall",
        "lsm-tree",
    ] {
        assert!(
            !names.contains(&forbidden),
            "{forbidden} reaches the library:\n{tree}"
        );
    }
}
