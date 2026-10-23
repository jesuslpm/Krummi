//! AC-R5 (R2 of the rename-to-krummi PRD): the workspace has exactly the packages `krummi`, `krummi-pager` and
//! `krummi-tx`, each in a folder of the same name.

use std::fs;
use std::path::Path;

#[test]
fn ac_r5_the_workspace_has_only_the_krummi_packages() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut packages: Vec<(String, String)> = fs::read_dir(&crates)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("Cargo.toml").exists())
        .map(|p| {
            let manifest = fs::read_to_string(p.join("Cargo.toml")).unwrap();
            let name = manifest
                .lines()
                .find_map(|l| l.strip_prefix("name = \""))
                .and_then(|l| l.strip_suffix('"'))
                .unwrap()
                .to_owned();
            (p.file_name().unwrap().to_string_lossy().into_owned(), name)
        })
        .collect();
    packages.sort();
    let expected = ["krummi", "krummi-pager", "krummi-tx"].map(|n| (n.to_owned(), n.to_owned()));
    assert_eq!(packages, expected);
    let workspace = fs::read_to_string(crates.join("..").join("Cargo.toml")).unwrap();
    assert!(
        workspace
            .contains(r#"members = ["crates/krummi-pager", "crates/krummi-tx", "crates/krummi"]"#),
        "{workspace}"
    );
}
