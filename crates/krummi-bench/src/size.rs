//! The size of a closed store (K9).

use std::fs;
use std::io;
use std::path::Path;

/// The sum of the lengths of every file under `folder`, subfolders included. Symbolic links are not followed.
pub fn folder_size(folder: &Path) -> io::Result<u64> {
    let mut total = 0;
    let mut stack = vec![folder.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                stack.push(entry.path());
            } else if kind.is_file() {
                total += entry.metadata()?.len();
            }
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    // AC-K12: the sum of the lengths of every file, subfolders included.
    #[test]
    fn the_size_adds_every_file_of_every_subfolder() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a"), [0u8; 10]).unwrap();
        fs::create_dir_all(dir.path().join("Journals/deep")).unwrap();
        fs::write(dir.path().join("Journals/b"), [0u8; 100]).unwrap();
        fs::write(dir.path().join("Journals/deep/c"), [0u8; 1_000]).unwrap();
        fs::create_dir(dir.path().join("empty")).unwrap();
        assert_eq!(folder_size(dir.path()).unwrap(), 1_110);
    }
}
