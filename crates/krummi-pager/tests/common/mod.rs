//! Helpers shared by the integration tests of the Pager.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use krummi_pager::format::{self, HEADER_PAGES, Header, PAGE_SIZE};
use krummi_pager::{Error, OpenOptions, Pager};

pub fn open(folder: &Path) -> Pager {
    Pager::open(folder, &OpenOptions::default())
        .unwrap_or_else(|e| panic!("opening {}: {e}", folder.display()))
}

pub fn open_err(folder: &Path) -> Error {
    match Pager::open(folder, &OpenOptions::default()) {
        Ok(_) => panic!("opening {} should fail", folder.display()),
        Err(e) => e,
    }
}

/// Every file under `dir`, by relative path with `/` separators; folders appear with a trailing `/`.
pub fn folder_snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                out.insert(format!("{name}/"), Vec::new());
                walk(base, &path, out);
            } else {
                out.insert(name, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

pub fn file_bytes(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap()
}

/// Flips the lowest bit of the byte at `offset`.
pub fn damage_byte(path: &Path, offset: usize) {
    let mut bytes = fs::read(path).unwrap();
    bytes[offset] ^= 0x01;
    fs::write(path, bytes).unwrap();
}

/// Rewrites both header copies of a closed store with a valid header of another format version.
pub fn rewrite_format_version(data: &Path, version: u32) {
    let mut bytes = fs::read(data).unwrap();
    let header = Header {
        format_version: version,
        ..Header::new_store()
    };
    format::write_header_pages(&mut bytes[..HEADER_PAGES as usize * PAGE_SIZE], &header);
    fs::write(data, bytes).unwrap();
}

const HOLD_ENV: &str = "MUNINN_TEST_HOLD_STORE";
const READY_ENV: &str = "MUNINN_TEST_READY_FILE";

/// Starts a child process that opens the store in `folder` and holds it until killed. Returns once the store
/// is open.
pub fn child_open_and_hold(folder: &Path) -> Child {
    let ready = folder.with_extension(format!("ready-{}", std::process::id()));
    let _ = fs::remove_file(&ready);
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "child_helper_open_and_hold",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(HOLD_ENV, folder)
        .env(READY_ENV, &ready)
        .spawn()
        .unwrap();
    let start = Instant::now();
    while !ready.exists() {
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "the child did not open the store"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = fs::remove_file(&ready);
    child
}

/// Body of the `child_helper_open_and_hold` test: does nothing unless started by [`child_open_and_hold`].
pub fn run_child_helper() {
    let (Some(folder), Some(ready)) = (std::env::var_os(HOLD_ENV), std::env::var_os(READY_ENV))
    else {
        return;
    };
    let pager = open(&PathBuf::from(folder));
    fs::write(ready, b"ready").unwrap();
    std::thread::sleep(Duration::from_secs(120));
    drop(pager);
}
